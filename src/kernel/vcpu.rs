use alloc::sync::Arc;
use alloc::vec::Vec;
use core::arch::asm;
use core::mem::size_of;

use spin::Mutex;

use crate::arch::{Aarch64ContextFrame, ContextFrameTrait, VmContext};
use crate::arch::tlb_invalidate_guest_all;
use crate::board::PLATFORM_VCPU_NUM_MAX;
use crate::kernel::{current_cpu, interrupt_vm_inject, timer_enable, vm_if_list_set_state};
use crate::kernel::{active_vcpu_id, active_vm_id, CPU_STACK_SIZE};
use crate::lib::{cache_invalidate_d, memcpy_safe};

use super::{CpuState, Vm, VmType};

#[derive(Clone, Debug)]
pub enum VcpuState {
    VcpuInv = 0,
    VcpuPend = 1,
    VcpuAct = 2,
}

#[derive(Clone)]
pub struct Vcpu {
    pub inner: Arc<Mutex<VcpuInner>>,
}

impl Vcpu {
    pub fn default() -> Vcpu {
        Vcpu {
            inner: Arc::new(Mutex::new(VcpuInner::default())),
        }
    }

    pub fn init(&self, vm: Vm, vcpu_id: usize) {
        let mut inner = self.inner.lock();
        inner.vm = Some(vm.clone());
        inner.id = vcpu_id;
        inner.phys_id = 0;
        drop(inner);
        crate::arch::vcpu_arch_init(vm.clone(), self.clone());
    }

    pub fn shutdown(&self) {
        println!(
            "Core {} (vm {} vcpu {}) shutdown ok",
            current_cpu().id,
            active_vm_id(),
            active_vcpu_id()
        );
        crate::board::platform_cpu_shutdown();
    }

    pub fn context_vm_store(&self) {
        self.save_cpu_ctx();

        let mut inner = self.inner.lock();
        inner.vm_ctx.ext_regs_store();
        inner.vm_ctx.fpsimd_save_context();
        inner.vm_ctx.gic_save_state();
    }

    pub fn context_vm_restore(&self) {
        // println!("context_vm_restore");
        self.restore_cpu_ctx();

        let inner = self.inner.lock();
        inner.vm_ctx.ext_regs_restore();

        // restore vm's VFP and SIMD
        inner.vm_ctx.fpsimd_restore_context();
        inner.vm_ctx.gic_restore_state();
        drop(inner);

        // restore vm's Stage2 MMU context
        let vttbr = (self.vm_id() << 48) | self.vm_pt_dir();
        // println!("vttbr {:x}", vttbr);
        unsafe {
            asm!("msr VTTBR_EL2, {0}", "isb", in(reg) vttbr);
        }
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
        match current_cpu().ctx {
            None => {
                println!("save_cpu_ctx: cpu{} ctx is NULL", current_cpu().id);
            }
            Some(ctx) => {
                memcpy_safe(
                    &(inner.vcpu_ctx) as *const _ as *const u8,
                    ctx as *const u8,
                    size_of::<Aarch64ContextFrame>(),
                );
            }
        }
    }

    pub fn restore_cpu_ctx(&self) {
        let inner = self.inner.lock();
        match current_cpu().ctx {
            None => {
                println!("restore_cpu_ctx: cpu{} ctx is NULL", current_cpu().id);
            }
            Some(ctx) => {
                memcpy_safe(
                    ctx as *const u8,
                    &(inner.vcpu_ctx) as *const _ as *const u8,
                    size_of::<Aarch64ContextFrame>(),
                );
            }
        }
    }

    pub fn set_phys_id(&self, phys_id: usize) {
        let mut inner = self.inner.lock();
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
        inner.state.clone()
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
        // inner.vm.clone()
        match &inner.vm {
            None => None,
            Some(vm) => Some(vm.clone()),
        }
        // if inner.vm.is_none() {
        //     None
        // } else {
        //     inner.vm.clone()
        //     // Some(inner.vm.as_ref().unwrap().clone())
        // }
    }

    pub fn phys_id(&self) -> usize {
        let inner = self.inner.lock();
        inner.phys_id
    }

    pub fn vm_id(&self) -> usize {
        let inner = self.inner.lock();
        let vm = inner.vm.clone().unwrap();
        drop(inner);
        vm.id()
    }

    pub fn vm_pt_dir(&self) -> usize {
        let inner = self.inner.lock();
        let vm = inner.vm.clone().unwrap();
        drop(inner);
        vm.pt_dir()
    }

    pub fn arch_reset(&self) {
        let mut inner = self.inner.lock();
        inner.arch_ctx_reset();
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

    pub fn show_ctx(&self) {
        let inner = self.inner.lock();
        inner.show_ctx();
    }

    pub fn push_int(&self, int: usize) {
        let mut inner = self.inner.lock();
        for i in &inner.int_list {
            if *i == int {
                return;
            }
        }
        inner.int_list.push(int);
    }

    // TODO: ugly lock for self.inner
    pub fn inject_int_inlist(&self) {
        let inner = self.inner.lock();
        match inner.vm.clone() {
            None => {}
            Some(vm) => {
                let int_list = inner.int_list.clone();
                drop(inner);
                for int in int_list {
                    // println!("schedule: inject int {} for vm {}", int, vm.id());
                    interrupt_vm_inject(vm.clone(), self.clone(), int, 0);
                }
                let mut inner = self.inner.lock();
                inner.int_list.clear();
            }
        }
    }
}

pub struct VcpuInner {
    pub id: usize,
    pub phys_id: usize,
    pub state: VcpuState,
    pub vm: Option<Vm>,
    pub int_list: Vec<usize>,
    pub vcpu_ctx: Aarch64ContextFrame,
    pub vm_ctx: VmContext,
}

impl VcpuInner {
    pub fn default() -> VcpuInner {
        VcpuInner {
            id: 0,
            phys_id: 0,
            state: VcpuState::VcpuInv,
            vm: None,
            int_list: vec![],
            vcpu_ctx: Aarch64ContextFrame::default(),
            vm_ctx: VmContext::default(),
        }
    }

    fn vcpu_ctx_addr(&self) -> usize {
        &(self.vcpu_ctx) as *const _ as usize
    }

    fn vm_id(&self) -> usize {
        let vm = self.vm.as_ref().unwrap();
        vm.id()
    }

    fn arch_ctx_reset(&mut self) {
        self.vm_ctx.cntvoff_el2 = 0;
        self.vm_ctx.sctlr_el1 = 0x30C50830;
        self.vm_ctx.cntkctl_el1 = 0;
        self.vm_ctx.pmcr_el0 = 0;
        self.vm_ctx.vtcr_el2 = 0x8001355c;

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

        use crate::kernel::vm_if_list_get_type;
        match vm_if_list_get_type(self.vm_id()) {
            VmType::VmTBma => {
                self.reset_vm_ctx();
                self.context_ext_regs_store();
            }
            _ => {}
        }
    }

    fn gic_ctx_reset(&mut self) {
        use crate::arch::gich_lrs_num;
        for i in 0..gich_lrs_num() {
            self.vm_ctx.gic_state.lr[i] = 0;
        }
        self.vm_ctx.gic_state.hcr |= 1 << 2;
    }

    fn context_ext_regs_store(&mut self) {
        self.vm_ctx.ext_regs_store();
    }

    fn reset_vm_ctx(&mut self) {
        self.vm_ctx.reset();
    }

    fn set_elr(&mut self, elr: usize) {
        self.vcpu_ctx.set_exception_pc(elr);
    }

    fn set_gpr(&mut self, idx: usize, val: usize) {
        self.vcpu_ctx.set_gpr(idx, val);
    }

    fn show_ctx(&self) {
        println!(
            "cntvoff_el2 {:x}, sctlr_el1 {:x}, cntkctl_el1 {:x}, pmcr_el0 {:x}, vtcr_el2 {:x} x0 {:x}",
            self.vm_ctx.cntvoff_el2,
            self.vm_ctx.sctlr_el1,
            self.vm_ctx.cntkctl_el1,
            self.vm_ctx.pmcr_el0,
            self.vm_ctx.vtcr_el2,
            self.vcpu_ctx.gpr(0)
        );
    }
}

static VCPU_LIST: Mutex<Vec<Vcpu>> = Mutex::new(Vec::new());

pub fn vcpu_alloc() -> Option<Vcpu> {
    let mut vcpu_list = VCPU_LIST.lock();
    if vcpu_list.len() >= PLATFORM_VCPU_NUM_MAX {
        return None;
    }

    let val = Vcpu::default();
    vcpu_list.push(val.clone());
    Some(val.clone())
}

pub fn vcpu_idle() {
    crate::kernel::cpu_idle();
}

pub fn vcpu_run() {
    println!(
        "Core {} (vm {}, vcpu {}) start running",
        current_cpu().id,
        active_vm_id(),
        active_vcpu_id()
    );

    if current_cpu().vcpu_pool().running() > 1 {
        timer_enable(true);
    }

    // let vm = crate::kernel::active_vm().unwrap();
    // vm.show_pagetable(0x17000000);
    let vcpu = current_cpu().active_vcpu.clone().unwrap();
    vcpu.set_state(VcpuState::VcpuAct);
    let vm = vcpu.vm().unwrap().clone();
    let sp = &(current_cpu().stack) as *const _ as usize + CPU_STACK_SIZE;
    let size = size_of::<Aarch64ContextFrame>();
    current_cpu().set_ctx((sp - size) as *mut _);

    vcpu.context_vm_restore();
    tlb_invalidate_guest_all();
    // vcpu.show_ctx();

    current_cpu().cpu_state = CpuState::CpuRun;
    vm_if_list_set_state(active_vm_id(), super::VmState::VmActive);

    for i in 0..vm.mem_region_num() {
        unsafe {
            cache_invalidate_d(vm.pa_start(i), vm.pa_length(i));
        }
    }

    println!(
        "vcpu run elr {:x} x0 {:016x}",
        current_cpu().active_vcpu.clone().unwrap().elr(),
        current_cpu().get_gpr(0)
    );
    // TODO: vcpu_run
    extern "C" {
        fn context_vm_entry(ctx: usize) -> !;
    }
    unsafe {
        context_vm_entry(sp - size);
    }
}
