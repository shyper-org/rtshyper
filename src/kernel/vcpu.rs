use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use spin::{Lazy, Mutex};

use crate::arch::{ContextFrame, ContextFrameTrait, InterruptContext, InterruptContextTriat, VmContext};
use crate::config::VmConfigEntry;
use crate::kernel::{current_cpu, interrupt_vm_inject, vm_if_set_state};

#[cfg(any(feature = "memory-reservation"))]
use super::bwres::membwres::MemoryBandwidth;
use super::{CpuState, Vm};
#[cfg(any(feature = "memory-reservation"))]
use crate::arch::PmuTimerEvent;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VcpuState {
    Inv = 0,
    Runnable = 1,
    Running = 2,
    Blocked = 3,
}

#[derive(Clone)]
#[repr(transparent)]
pub struct Vcpu(pub Arc<VcpuInner>);

impl PartialEq for Vcpu {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Vcpu {}

#[allow(dead_code)]
pub struct WeakVcpu(Weak<VcpuInner>);

#[allow(dead_code)]
impl WeakVcpu {
    pub fn upgrade(&self) -> Option<Vcpu> {
        self.0.upgrade().map(Vcpu)
    }
}

pub struct VcpuInner {
    inner_const: VcpuConst,
    pub inner_mut: Mutex<VcpuInnerMut>,
    #[cfg(any(feature = "memory-reservation"))]
    reservation: MemoryBandwidth,
    #[cfg(any(feature = "memory-reservation"))]
    pmu_event: Option<Arc<PmuTimerEvent>>,
}

struct VcpuConst {
    id: usize,      // vcpu_id
    vm: Weak<Vm>,   // weak pointer to related Vm
    phys_id: usize, // related physical CPU id
}

impl Vcpu {
    #[allow(unused_variables)]
    pub(super) fn new(vm: Weak<Vm>, vcpu_id: usize, phys_id: usize, config: &VmConfigEntry) -> Self {
        let inner_const = VcpuConst {
            id: vcpu_id,
            vm,
            phys_id,
        };
        #[cfg(any(feature = "memory-reservation"))]
        let inner = Arc::new_cyclic(|weak| VcpuInner {
            inner_const,
            reservation: MemoryBandwidth::new(
                // each vcpu allocates bandwidth equally
                config.memory.budget / config.cpu_num() as u32,
                config.memory.period,
            ),
            pmu_event: if config.memory.is_limited() {
                debug!("vcpu {vcpu_id} memory is limited");
                Some(Arc::new(PmuTimerEvent(WeakVcpu(weak.clone()))))
            } else {
                None
            },
            inner_mut: Mutex::new(VcpuInnerMut::new()),
        });
        #[cfg(not(feature = "memory-reservation"))]
        let inner = Arc::new(VcpuInner {
            inner_const,
            inner_mut: Mutex::new(VcpuInnerMut::new()),
        });
        Self(inner)
    }

    #[cfg(any(feature = "memory-reservation"))]
    pub(super) fn pmu_event(&self) -> Option<Arc<PmuTimerEvent>> {
        self.0.pmu_event.clone()
    }

    pub fn init(&self, config: &VmConfigEntry) {
        self.init_boot_info(config);
        self.reset_context();
    }

    pub fn init_boot_info(&self, config: &VmConfigEntry) {
        use crate::kernel::VmType;
        let arg = match config.os_type {
            VmType::VmTOs => config.device_tree_load_ipa(),
            _ => {
                let arg = &config.memory_region()[0];
                arg.ipa_start + arg.length
            }
        };
        let mut inner = self.0.inner_mut.lock();
        inner.vcpu_ctx.set_argument(arg);
        inner.vcpu_ctx.set_exception_pc(config.kernel_entry_point());
    }

    // pub fn shutdown(&self) {
    //     use crate::board::{PlatOperation, Platform};
    //     info!(
    //         "Core {} (vm {} vcpu {}) shutdown ok",
    //         current_cpu().id,
    //         active_vm().unwrap().id(),
    //         active_vcpu_id()
    //     );
    //     Platform::cpu_shutdown();
    // }

    pub fn context_vm_store(&self) {
        #[cfg(any(feature = "memory-reservation"))]
        if self.0.pmu_event.is_some() {
            crate::arch::vcpu_stop_pmu(self);
        }

        #[cfg(feature = "vtimer")]
        self.vm().unwrap().update_vtimer();
        self.save_cpu_ctx();

        let mut inner = self.0.inner_mut.lock();
        inner.vm_ctx.ext_regs_store();
        drop(inner);
        self.intc_save_context();
    }

    pub fn context_vm_restore(&self) {
        #[cfg(any(feature = "memory-reservation"))]
        if self.0.pmu_event.is_some() {
            crate::arch::vcpu_start_pmu(self);
        }

        #[cfg(feature = "vtimer")]
        let vtimer_offset = self.vm().unwrap().update_vtimer_offset();
        self.restore_cpu_ctx();

        let mut inner = self.0.inner_mut.lock();
        #[cfg(feature = "vtimer")]
        inner.vm_ctx.generic_timer.set_offset(vtimer_offset as u64);
        inner.vm_ctx.ext_regs_restore();
        drop(inner);
        self.intc_restore_context();

        self.inject_int_inlist();
    }

    pub fn intc_restore_context(&self) {
        let inner = self.0.inner_mut.lock();
        inner.intc_ctx.restore_state();
    }

    pub fn intc_save_context(&self) {
        let mut inner = self.0.inner_mut.lock();
        inner.intc_ctx.save_state();
    }

    fn save_cpu_ctx(&self) {
        if let Some(ctx) = unsafe { current_cpu().current_ctx().as_ref() } {
            let mut inner = self.0.inner_mut.lock();
            inner.vcpu_ctx.clone_from(ctx);
        } else {
            error!("save_cpu_ctx: cpu{} ctx is NULL", current_cpu().id);
        }
    }

    fn restore_cpu_ctx(&self) {
        if let Some(ctx) = unsafe { current_cpu().current_ctx().as_mut() } {
            let inner = self.0.inner_mut.lock();
            ctx.clone_from(&inner.vcpu_ctx);
        } else {
            error!("save_cpu_ctx: cpu{} ctx is NULL", current_cpu().id);
        }
    }

    pub fn state(&self) -> VcpuState {
        let inner = self.0.inner_mut.lock();
        inner.state
    }

    pub(super) fn set_state(&self, state: VcpuState) {
        let mut inner = self.0.inner_mut.lock();
        inner.state = state;
    }

    #[inline]
    pub fn id(&self) -> usize {
        self.0.inner_const.id
    }

    #[inline]
    pub fn vm(&self) -> Option<Arc<Vm>> {
        self.0.inner_const.vm.upgrade()
    }

    #[inline]
    pub fn phys_id(&self) -> usize {
        self.0.inner_const.phys_id
    }

    pub fn vm_id(&self) -> usize {
        self.vm().unwrap().id()
    }

    pub fn vm_pt_dir(&self) -> usize {
        self.vm().unwrap().pt_dir()
    }

    fn reset_context(&self) {
        let mut inner = self.0.inner_mut.lock();

        let mut vmpidr = 1 << 31;

        #[cfg(feature = "tx2")]
        if self.vm_id() == 0 {
            // A57 is cluster #1 for L4T
            vmpidr |= 0x100;
        }

        vmpidr |= self.id();
        inner.vm_ctx.vmpidr_el2 = vmpidr as u64;
        // if self.vm().vm_type() == VmType::VmTBma {
        //     info!("vm {} bma ctx restore", self.vm_id());
        //     self.reset_vm_ctx();
        //     self.context_ext_regs_store(); // what the fuck ?? why store here ???
        // }
    }

    pub fn set_exception_pc(&self, elr: usize) {
        let mut inner = self.0.inner_mut.lock();
        inner.vcpu_ctx.set_exception_pc(elr);
    }

    pub fn set_gpr(&self, idx: usize, val: usize) {
        let mut inner = self.0.inner_mut.lock();
        inner.vcpu_ctx.set_gpr(idx, val);
    }

    pub fn push_int(&self, int: usize) {
        let mut inner = self.0.inner_mut.lock();
        if !inner.int_list.contains(&int) {
            inner.int_list.push(int);
        }
    }

    fn inject_int_inlist(&self) {
        if let Some(vm) = self.vm() {
            let mut inner = self.0.inner_mut.lock();
            let int_list = core::mem::take(&mut inner.int_list);
            drop(inner);
            trace!("schedule: inject int {:?} for vm {}", int_list, vm.id());
            for int in int_list {
                interrupt_vm_inject(&vm, self, int);
            }
        }
    }
}

#[cfg(any(feature = "memory-reservation"))]
impl Vcpu {
    pub fn remaining_budget(&self) -> u32 {
        self.0.reservation.remaining_budget()
    }

    pub fn period(&self) -> u64 {
        self.0.reservation.period()
    }

    pub fn update_remaining_budget(&self, remaining_budget: u32) {
        self.0.reservation.update_remaining_budget(remaining_budget);
    }

    pub fn reset_remaining_budget(&self) {
        self.0.reservation.reset_remaining_budget();
    }

    pub fn supply_budget(&self) {
        self.0.reservation.supply_budget();
    }

    #[cfg(any(feature = "dynamic-budget"))]
    pub fn budget_try_rescue(&self) -> bool {
        self.0.reservation.budget_try_rescue()
    }
}

pub struct VcpuInnerMut {
    state: VcpuState,
    int_list: Vec<usize>,
    // regs: ArchVcpuRegs
    vcpu_ctx: ContextFrame,
    pub vm_ctx: VmContext,
    pub intc_ctx: InterruptContext,
}

impl VcpuInnerMut {
    fn new() -> Self {
        Self {
            state: VcpuState::Inv,
            int_list: vec![],
            vcpu_ctx: ContextFrame::default(),
            vm_ctx: VmContext::new(),
            intc_ctx: InterruptContext::default(),
        }
    }
}

// WARNING: No Auto `drop` in this function
pub fn vcpu_run(announce: bool) {
    let vcpu = current_cpu().active_vcpu.clone().unwrap();
    let vm = vcpu.vm().unwrap();

    vm_if_set_state(vm.id(), super::VmState::Active);

    if announce {
        crate::device::virtio_net_announce(vm);
    }
    // if the cpu is already running (a vcpu in scheduling queue), just return
    if current_cpu().cpu_state == CpuState::Run {
        return;
    }
    current_cpu().cpu_state = CpuState::Run;
    // tlb_invalidate_guest_all();
}

fn idle_thread() -> ! {
    loop {
        use crate::arch::ArchTrait;
        crate::arch::Arch::wait_for_interrupt();
    }
}

struct IdleThread {
    ctx: ContextFrame,
}

static IDLE_THREAD: Lazy<IdleThread> = Lazy::new(|| {
    let mut ctx = ContextFrame::new_privileged();
    ctx.set_exception_pc(idle_thread as usize);
    IdleThread { ctx }
});

pub(super) fn run_idle_thread() {
    if let Some(ctx) = unsafe { current_cpu().current_ctx().as_mut() } {
        trace!("Core {} idle", current_cpu().id);
        current_cpu().cpu_state = CpuState::Idle;
        ctx.clone_from(&IDLE_THREAD.ctx);
    } else {
        error!("run_idle_thread: cpu{} ctx is NULL", current_cpu().id);
    }
}
