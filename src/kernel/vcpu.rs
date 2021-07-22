use super::{CpuState, Vm, VmType};
use crate::arch::tlb_invalidate_guest_all;
use crate::arch::{Aarch64ContextFrame, ContextFrameTrait, VmContext};
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

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

    pub fn ptr_eq(&self, vcpu: Vcpu) -> bool {
        Arc::ptr_eq(&self.inner, &vcpu.inner())
    }

    pub fn inner(&self) -> Arc<Mutex<VcpuInner>> {
        self.inner().clone()
    }

    pub fn set_phys_id(&self, phys_id: usize) {
        let mut inner = self.inner.lock();
        inner.phys_id = phys_id;
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
        vm.vm_id()
    }

    #[allow(dead_code)]
    pub fn vm_pt_dir(&self) -> usize {
        let inner = self.inner.lock();
        let vm = inner.vm.clone().unwrap();
        drop(inner);
        vm.pt_dir()
    }

    #[allow(dead_code)]
    pub fn arch_reset(&self) {
        let inner = self.inner.lock();
        inner.arch_reset();
    }

    pub fn reset_state(&self) {
        let mut inner = self.inner.lock();
        inner.reset_state();
    }

    #[allow(dead_code)]
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

    pub fn set_gpr(&self, idx: usize, val: usize) {
        let mut inner = self.inner.lock();
        inner.set_gpr(idx, val);
    }
}

pub struct VcpuInner {
    pub id: usize,
    pub phys_id: usize,
    pub state: VcpuState,
    pub vm: Option<Vm>,
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
            vcpu_ctx: Aarch64ContextFrame::default(),
            vm_ctx: VmContext::default(),
        }
    }

    fn vcpu_ctx_addr(&self) -> usize {
        &(self.vcpu_ctx) as *const _ as usize
    }

    fn vm_id(&self) -> usize {
        let vm = self.vm.as_ref().unwrap();
        vm.vm_id()
    }

    fn vm_pt_dir(&self) -> usize {
        let vm = self.vm.as_ref().unwrap();
        vm.pt_dir()
    }

    fn arch_reset(&self) {
        unsafe {
            llvm_asm!("msr cntvoff_el2, $0" :: "r"(0) :: "volatile");
            llvm_asm!("msr sctlr_el1, $0" :: "r"(0x30C50830 as usize) :: "volatile");
            llvm_asm!("msr cntkctl_el1, $0" :: "r"(0) :: "volatile");
            llvm_asm!("msr pmcr_el0, $0" :: "r"(0) :: "volatile");
            llvm_asm!("msr vtcr_el2, $0" :: "r"(0x8001355c as usize) :: "volatile");
        };
        let vttbr = (self.vm_id() << 48) | self.vm_pt_dir();
        // println!("vttbr_el2 is {:x}", vttbr);
        // println!("vttbr_el2 pt addr is {:x}", self.vm_pt_dir());
        unsafe {
            llvm_asm!("msr vttbr_el2, $0" :: "r"(vttbr) :: "volatile");
            llvm_asm!("isb");
        }

        tlb_invalidate_guest_all();

        let mut vmpidr = 0;
        vmpidr |= 1 << 31;

        #[cfg(feature = "tx2")]
        if self.vm_id() == 0 {
            // A57 is cluster #1 for L4T
            vmpidr |= 0x100;
        }

        vmpidr |= self.id;
        unsafe {
            llvm_asm!("msr vmpidr_el2, $0" :: "r"(vmpidr) :: "volatile");
        }
    }

    fn reset_state(&mut self) {
        self.arch_reset();

        use crate::kernel::vm_if_list_get_type;
        match vm_if_list_get_type(self.vm_id()) {
            VmType::VmTBma => {
                self.context_ext_regs_store();
            }
            _ => {}
        }
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

use crate::board::PLATFORM_VCPU_NUM_MAX;
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

use crate::kernel::vm_if_list_set_state;
use crate::kernel::{
    active_vcpu, active_vcpu_id, active_vm_id, cpu_id, cpu_stack, set_cpu_state, CPU_STACK_SIZE,
};
pub fn vcpu_run() {
    println!(
        "Core {} (vm {}, vcpu {}) start running",
        cpu_id(),
        active_vm_id(),
        active_vcpu_id()
    );

    let sp = cpu_stack() + CPU_STACK_SIZE;
    let ctx = active_vcpu().unwrap().vcpu_ctx_addr();

    use core::mem::size_of;
    use rlibc::memcpy;
    let size = size_of::<Aarch64ContextFrame>();
    unsafe {
        memcpy((sp - size) as *mut u8, ctx as *mut u8, size);
    }

    set_cpu_state(CpuState::CpuRun);
    vm_if_list_set_state(active_vm_id(), super::VmState::VmActive);
    // TODO: vcpu_run
    extern "C" {
        fn context_vm_entry(ctx: usize) -> !;
    }
    unsafe {
        context_vm_entry(sp - size);
    }
}
