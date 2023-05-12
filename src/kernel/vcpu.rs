use alloc::sync::Arc;
use alloc::vec::Vec;
use core::mem::size_of;
use spin::Mutex;

use crate::arch::{ContextFrame, ContextFrameTrait, cpu_interrupt_unmask, GicContext, VmContext, VM_IPA_SIZE};
use crate::board::{PlatOperation, Platform};
use crate::kernel::{current_cpu, interrupt_vm_inject, vm_if_set_state};
use crate::kernel::{active_vcpu_id, active_vm_id};
use crate::util::memcpy_safe;

use super::{CpuState, Vm, WeakVm};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VcpuState {
    Inv = 0,
    Runnable = 1,
    Running = 2,
}

#[derive(Clone)]
pub struct Vcpu {
    pub inner: Arc<Mutex<VcpuInner>>,
}

#[allow(dead_code)]
impl Vcpu {
    pub fn new(vm: &Vm, vcpu_id: usize) -> Self {
        let this = Self {
            inner: Arc::new(Mutex::new(VcpuInner::new(vm.get_weak(), vcpu_id))),
        };
        crate::arch::vcpu_arch_init(vm, &this);
        this.reset_context();
        this
    }

    pub fn shutdown(&self) {
        println!(
            "Core {} (vm {} vcpu {}) shutdown ok",
            current_cpu().id,
            active_vm_id(),
            active_vcpu_id()
        );
        Platform::cpu_shutdown();
    }

    pub fn context_vm_store(&self) {
        self.vm().unwrap().update_vtimer();
        self.save_cpu_ctx();

        let mut inner = self.inner.lock();
        inner.vm_ctx.ext_regs_store();
        inner.vm_ctx.fpsimd_save_context();
        inner.vm_ctx.gic_save_state();
    }

    pub fn context_vm_restore(&self) {
        // println!("context_vm_restore");
        let vtimer_offset = self.vm().unwrap().update_vtimer_offset();
        self.restore_cpu_ctx();

        let mut inner = self.inner.lock();
        inner.vm_ctx.generic_timer.set_offset(vtimer_offset as u64);
        // restore vm's VFP and SIMD
        inner.vm_ctx.fpsimd_restore_context();
        inner.vm_ctx.gic_restore_state();
        inner.vm_ctx.ext_regs_restore();
        drop(inner);

        self.inject_int_inlist();
    }

    pub fn gic_restore_context(&self) {
        let inner = self.inner.lock();
        inner.vm_ctx.gic_restore_state();
    }

    pub fn gic_save_context(&self) {
        let mut inner = self.inner.lock();
        inner.vm_ctx.gic_save_state();
    }

    pub fn save_cpu_ctx(&self) {
        let inner = self.inner.lock();
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
        let inner = self.inner.lock();
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

    pub fn set_phys_id(&self, phys_id: usize) {
        let mut inner = self.inner.lock();
        println!("set vcpu {} phys id {}", inner.id, phys_id);
        inner.phys_id = phys_id;
    }

    pub fn set_gich_ctlr(&self, ctlr: u32) {
        let mut inner = self.inner.lock();
        inner.vm_ctx.gic_state.ctlr = ctlr;
    }

    pub fn set_hcr(&self, hcr: u64) {
        let mut inner = self.inner.lock();
        inner.vm_ctx.hcr_el2 = hcr;
    }

    pub fn state(&self) -> VcpuState {
        let inner = self.inner.lock();
        inner.state
    }

    pub fn set_state(&self, state: VcpuState) {
        let mut inner = self.inner.lock();
        inner.state = state;
    }

    pub fn id(&self) -> usize {
        let inner = self.inner.lock();
        inner.id
    }

    pub fn vm(&self) -> Option<Vm> {
        let inner = self.inner.lock();
        inner.vm.get_vm()
    }

    pub fn phys_id(&self) -> usize {
        let inner = self.inner.lock();
        inner.phys_id
    }

    pub fn vm_id(&self) -> usize {
        self.vm().unwrap().id()
    }

    pub fn vm_pt_dir(&self) -> usize {
        self.vm().unwrap().pt_dir()
    }

    pub fn reset_context(&self) {
        let mut inner = self.inner.lock();
        inner.reset_context();
    }

    pub fn context_ext_regs_store(&self) {
        let mut inner = self.inner.lock();
        inner.context_ext_regs_store();
    }

    pub fn vcpu_ctx_addr(&self) -> usize {
        let inner = self.inner.lock();
        inner.vcpu_ctx_addr()
    }

    pub fn set_elr(&self, elr: usize) {
        let mut inner = self.inner.lock();
        inner.set_elr(elr);
    }

    pub fn elr(&self) -> usize {
        let inner = self.inner.lock();
        inner.vcpu_ctx.exception_pc()
    }

    pub fn set_gpr(&self, idx: usize, val: usize) {
        let mut inner = self.inner.lock();
        inner.set_gpr(idx, val);
    }

    pub fn push_int(&self, int: usize) {
        let mut inner = self.inner.lock();
        if !inner.int_list.contains(&int) {
            inner.int_list.push(int);
        }
    }

    fn inject_int_inlist(&self) {
        match self.vm() {
            None => {}
            Some(vm) => {
                let mut inner = self.inner.lock();
                let int_list = inner.int_list.clone();
                inner.int_list.clear();
                drop(inner);
                for int in int_list {
                    // println!("schedule: inject int {} for vm {}", int, vm.id());
                    interrupt_vm_inject(&vm, self, int);
                }
            }
        }
    }
}

pub struct VcpuInner {
    pub id: usize,
    pub phys_id: usize,
    pub state: VcpuState,
    pub vm: WeakVm,
    pub int_list: Vec<usize>,
    pub vcpu_ctx: ContextFrame,
    pub vm_ctx: VmContext,
    pub gic_ctx: GicContext,
}

impl VcpuInner {
    pub fn new(vm: WeakVm, id: usize) -> Self {
        Self {
            id,
            phys_id: 0,
            state: VcpuState::Inv,
            vm,
            int_list: vec![],
            vcpu_ctx: ContextFrame::default(),
            vm_ctx: VmContext::default(),
            gic_ctx: GicContext::default(),
        }
    }

    fn vcpu_ctx_addr(&self) -> usize {
        &(self.vcpu_ctx) as *const _ as usize
    }

    #[cfg(feature = "tx2")]
    fn vm_id(&self) -> usize {
        self.vm.get_vm().unwrap().id()
    }

    fn arch_ctx_reset(&mut self) {
        self.vm_ctx.sctlr_el1 = 0x30C50830;
        self.vm_ctx.pmcr_el0 = 0;
        self.vm_ctx.vtcr_el2 = 0x80013540 + ((64 - VM_IPA_SIZE) & ((1 << 6) - 1));
        let mut vmpidr = 0;
        vmpidr |= 1 << 31;

        #[cfg(feature = "tx2")]
        if self.vm_id() == 0 {
            // A57 is cluster #1 for L4T
            vmpidr |= 0x100;
        }

        vmpidr |= self.id;
        self.vm_ctx.vmpidr_el2 = vmpidr as u64;
    }

    fn reset_context(&mut self) {
        self.arch_ctx_reset();
        self.gic_ctx_reset();
        // use crate::kernel::vm_if_get_type;
        // if vm_if_get_type(self.vm_id()) == VmType::VmTBma {
        //     println!("vm {} bma ctx restore", self.vm_id());
        //     self.reset_vm_ctx();
        //     self.context_ext_regs_store(); // what the fuck ?? why store here ???
        // }
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
