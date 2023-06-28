use core::mem::size_of;
use core::sync::atomic::{AtomicU32, AtomicBool, Ordering};

use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

use crate::arch::{ContextFrame, ContextFrameTrait, cpu_interrupt_unmask, GicContext, VmContext, VM_IPA_SIZE};
use crate::config::VmConfigEntry;
use crate::kernel::{current_cpu, interrupt_vm_inject, vm_if_set_state, active_vm_id};
use crate::util::memcpy_safe;

use super::{CpuState, Vm, WeakVm};

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

impl core::cmp::PartialEq for Vcpu {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

pub struct VcpuInner {
    inner_const: VcpuConst,
    reservation: MemoryBandwidth,
    pub inner_mut: Mutex<VcpuInnerMut>,
}

struct MemoryBandwidth {
    budget: u32,
    period: u64,
    remaining_budget: AtomicU32,
    replenish: AtomicBool,
}

impl MemoryBandwidth {
    fn new(budget: u32, period: u64) -> Self {
        Self {
            budget,
            period,
            remaining_budget: AtomicU32::new(budget),
            replenish: AtomicBool::new(false),
        }
    }
}

struct VcpuConst {
    id: usize,      // vcpu_id
    vm: WeakVm,     // weak pointer to related Vm
    phys_id: usize, // related physical CPU id
}

#[allow(dead_code)]
impl Vcpu {
    pub(super) fn new(vm: WeakVm, vcpu_id: usize, phys_id: usize, config: &VmConfigEntry) -> Self {
        Self(Arc::new(VcpuInner {
            inner_const: VcpuConst {
                id: vcpu_id,
                vm,
                phys_id,
            },
            reservation: MemoryBandwidth::new(config.memory_budget(), config.memory_replenishment_period()),
            inner_mut: Mutex::new(VcpuInnerMut::new()),
        }))
    }

    pub(super) fn init(&self, config: &VmConfigEntry) {
        crate::arch::vcpu_arch_init(config, self);
        self.reset_context();
    }

    // pub fn shutdown(&self) {
    //     use crate::board::{PlatOperation, Platform};
    //     println!(
    //         "Core {} (vm {} vcpu {}) shutdown ok",
    //         current_cpu().id,
    //         active_vm_id(),
    //         active_vcpu_id()
    //     );
    //     Platform::cpu_shutdown();
    // }

    pub fn context_vm_store(&self) {
        self.vm().unwrap().update_vtimer();
        self.save_cpu_ctx();

        #[cfg(any(feature = "memory-reservation"))]
        crate::arch::vcpu_stop_pmu(self);

        let mut inner = self.0.inner_mut.lock();
        inner.vm_ctx.ext_regs_store();
        inner.vm_ctx.fpsimd_save_context();
        inner.vm_ctx.gic_save_state();
    }

    pub fn context_vm_restore(&self) {
        // println!("context_vm_restore");
        let vtimer_offset = self.vm().unwrap().update_vtimer_offset();
        self.restore_cpu_ctx();

        #[cfg(any(feature = "memory-reservation"))]
        crate::arch::vcpu_start_pmu(self);

        let mut inner = self.0.inner_mut.lock();
        inner.vm_ctx.generic_timer.set_offset(vtimer_offset as u64);
        // restore vm's VFP and SIMD
        inner.vm_ctx.fpsimd_restore_context();
        inner.vm_ctx.gic_restore_state();
        inner.vm_ctx.ext_regs_restore();
        drop(inner);

        self.inject_int_inlist();
    }

    pub fn gic_restore_context(&self) {
        let inner = self.0.inner_mut.lock();
        inner.vm_ctx.gic_restore_state();
    }

    pub fn gic_save_context(&self) {
        let mut inner = self.0.inner_mut.lock();
        inner.vm_ctx.gic_save_state();
    }

    pub fn save_cpu_ctx(&self) {
        let inner = self.0.inner_mut.lock();
        match current_cpu().current_ctx() {
            None => {
                println!("save_cpu_ctx: cpu{} ctx is NULL", current_cpu().id);
            }
            Some(ctx) => {
                memcpy_safe(
                    &(inner.vcpu_ctx) as *const _ as *const u8,
                    ctx as *const u8,
                    size_of::<ContextFrame>(),
                );
            }
        }
    }

    fn restore_cpu_ctx(&self) {
        let inner = self.0.inner_mut.lock();
        match current_cpu().current_ctx() {
            None => {
                println!("restore_cpu_ctx: cpu{} ctx is NULL", current_cpu().id);
            }
            Some(ctx) => {
                memcpy_safe(
                    ctx as *const u8,
                    &(inner.vcpu_ctx) as *const _ as *const u8,
                    size_of::<ContextFrame>(),
                );
            }
        }
    }

    pub fn set_gich_ctlr(&self, ctlr: u32) {
        let mut inner = self.0.inner_mut.lock();
        inner.vm_ctx.gic_state.ctlr = ctlr;
    }

    pub fn set_hcr(&self, hcr: u64) {
        let mut inner = self.0.inner_mut.lock();
        inner.vm_ctx.hcr_el2 = hcr;
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
    pub fn vm(&self) -> Option<Vm> {
        self.0.inner_const.vm.get_vm()
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

    pub fn reset_context(&self) {
        self.arch_ctx_reset();
        let mut inner = self.0.inner_mut.lock();
        inner.gic_ctx_reset();
        // if self.vm().vm_type() == VmType::VmTBma {
        //     println!("vm {} bma ctx restore", self.vm_id());
        //     self.reset_vm_ctx();
        //     self.context_ext_regs_store(); // what the fuck ?? why store here ???
        // }
    }

    fn arch_ctx_reset(&self) {
        let mut inner = self.0.inner_mut.lock();
        inner.vm_ctx.sctlr_el1 = 0x30C50830;
        inner.vm_ctx.vtcr_el2 = 0x80013540 + ((64 - VM_IPA_SIZE) & ((1 << 6) - 1));
        let mut vmpidr = 0;
        vmpidr |= 1 << 31;

        #[cfg(feature = "tx2")]
        if self.vm_id() == 0 {
            // A57 is cluster #1 for L4T
            vmpidr |= 0x100;
        }

        vmpidr |= self.id();
        inner.vm_ctx.vmpidr_el2 = vmpidr as u64;
    }

    pub fn context_ext_regs_store(&self) {
        let mut inner = self.0.inner_mut.lock();
        inner.context_ext_regs_store();
    }

    pub fn vcpu_ctx_addr(&self) -> usize {
        let inner = self.0.inner_mut.lock();
        inner.vcpu_ctx_addr()
    }

    pub fn set_elr(&self, elr: usize) {
        let mut inner = self.0.inner_mut.lock();
        inner.set_elr(elr);
    }

    pub fn elr(&self) -> usize {
        let inner = self.0.inner_mut.lock();
        inner.vcpu_ctx.exception_pc()
    }

    pub fn set_gpr(&self, idx: usize, val: usize) {
        let mut inner = self.0.inner_mut.lock();
        inner.set_gpr(idx, val);
    }

    pub fn push_int(&self, int: usize) {
        let mut inner = self.0.inner_mut.lock();
        if !inner.int_list.contains(&int) {
            inner.int_list.push(int);
        }
    }

    fn inject_int_inlist(&self) {
        match self.vm() {
            None => {}
            Some(vm) => {
                let mut inner = self.0.inner_mut.lock();
                let int_list = core::mem::take(&mut inner.int_list);
                drop(inner);
                for int in int_list {
                    // println!("schedule: inject int {} for vm {}", int, vm.id());
                    interrupt_vm_inject(&vm, self, int);
                }
            }
        }
    }

    pub fn remaining_budget(&self) -> u32 {
        self.0.reservation.remaining_budget.load(Ordering::Relaxed)
    }

    pub(super) fn period(&self) -> u64 {
        self.0.reservation.period
    }

    pub fn update_remaining_budget(&self, remaining_budget: u32) {
        self.0
            .reservation
            .remaining_budget
            .store(remaining_budget, Ordering::Relaxed);
    }

    pub fn reset_remaining_budget(&self) {
        let reservation = &self.0.reservation;
        reservation.remaining_budget.store(0, Ordering::Relaxed);
        reservation.replenish.store(true, Ordering::Relaxed);
    }

    pub fn supply_budget(&self) {
        let reservation = &self.0.reservation;
        if reservation.replenish.load(Ordering::Relaxed) {
            reservation
                .remaining_budget
                .store(reservation.budget, Ordering::Relaxed);
            reservation.replenish.store(false, Ordering::Relaxed);
        }
    }
}

pub struct VcpuInnerMut {
    pub state: VcpuState,
    pub int_list: Vec<usize>,
    pub vcpu_ctx: ContextFrame,
    pub vm_ctx: VmContext,
    pub gic_ctx: GicContext,
}

impl VcpuInnerMut {
    fn new() -> Self {
        Self {
            state: VcpuState::Inv,
            int_list: vec![],
            vcpu_ctx: ContextFrame::default(),
            vm_ctx: VmContext::default(),
            gic_ctx: GicContext::default(),
        }
    }

    fn vcpu_ctx_addr(&self) -> usize {
        &(self.vcpu_ctx) as *const _ as usize
    }

    fn gic_ctx_reset(&mut self) {
        use crate::arch::gic_lrs;
        for i in 0..gic_lrs() {
            self.vm_ctx.gic_state.lr[i] = 0;
        }
        self.vm_ctx.gic_state.hcr |= 1 << 2;
    }

    fn context_ext_regs_store(&mut self) {
        self.vm_ctx.ext_regs_store();
    }

    fn set_elr(&mut self, elr: usize) {
        self.vcpu_ctx.set_exception_pc(elr);
    }

    fn set_gpr(&mut self, idx: usize, val: usize) {
        self.vcpu_ctx.set_gpr(idx, val);
    }
}

#[allow(dead_code)]
pub fn vcpu_idle(_vcpu: Vcpu) -> ! {
    cpu_interrupt_unmask();
    loop {
        use crate::arch::ArchTrait;
        crate::arch::Arch::wait_for_interrupt();
    }
}

// WARNING: No Auto `drop` in this function
pub fn vcpu_run(announce: bool) {
    {
        let vcpu = current_cpu().active_vcpu.clone().unwrap();
        let vm = vcpu.vm().unwrap();

        vm_if_set_state(active_vm_id(), super::VmState::Active);

        if announce {
            crate::device::virtio_net_announce(vm);
        }
        // if the cpu is already running (a vcpu in scheduling queue),
        // just return, no `context_vm_entry` required
        if current_cpu().cpu_state == CpuState::Run {
            return;
        }
        current_cpu().cpu_state = CpuState::Run;
        // tlb_invalidate_guest_all();
        // for i in 0..vm.mem_region_num() {
        //     unsafe {
        //         cache_invalidate_d(vm.pa_start(i), vm.pa_length(i));
        //     }
        // }
    }
    extern "C" {
        fn context_vm_entry(ctx: usize) -> !;
    }
    unsafe {
        context_vm_entry(current_cpu().current_ctx().unwrap());
    }
}
