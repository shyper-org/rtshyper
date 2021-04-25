use super::{Vm, VmInner, VmType};
use crate::arch::{Aarch64ContextFrame, VmContext};
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

pub enum VcpuState {
    VcpuInv = 0,
    VcpuPend = 1,
    VcpuAct = 2,
}

pub struct Vcpu {
    pub id: usize,
    pub phys_id: usize,
    pub state: VcpuState,
    pub vm: Option<Vm>,
    pub vcpu_ctx: Aarch64ContextFrame,
    pub vm_ctx: VmContext,
}

impl Vcpu {
    pub fn default() -> Vcpu {
        Vcpu {
            id: 0,
            phys_id: 0,
            state: VcpuState::VcpuInv,
            vm: None,
            vcpu_ctx: Aarch64ContextFrame::default(),
            vm_ctx: VmContext::default(),
        }
    }

    pub fn vm_id(&self) -> usize {
        let vm = self.vm.as_ref().unwrap();
        let id = vm.inner().lock().id;
        id
    }

    pub fn vm_pt_dir(&self) -> usize {
        let vm = self.vm.as_ref().unwrap();
        vm.pt_dir()
    }

    pub fn arch_reset(&self) {
        unsafe {
            llvm_asm!("msr cntvoff_el2, $0" :: "r"(0) :: "volatile");
            llvm_asm!("msr sctlr_el1, $0" :: "r"(0x30C50830 as usize) :: "volatile");
            llvm_asm!("msr cntkctl_el1, $0" :: "r"(0) :: "volatile");
            llvm_asm!("msr pmcr_el0, $0" :: "r"(0) :: "volatile");
            llvm_asm!("msr vtcr_el2, $0" :: "r"(0x8001355c as usize) :: "volatile");
        };
        let vttbr = (self.vm_id() << 48) | self.vm_pt_dir();
        // println!("vttbr is {:x}", vttbr);
        unsafe {
            llvm_asm!("msr vttbr_el2, $0" :: "r"(vttbr) :: "volatile");
            llvm_asm!("isb");
        }

        // TODO: tlb_invalidate_guest_all
        let mut vmpidr = 0;
        vmpidr |= 1 << 31;
        vmpidr |= self.id;
        unsafe {
            llvm_asm!("msr vmpidr_el2, $0" :: "r"(vmpidr) :: "volatile");
        }
    }

    pub fn reset_state(&mut self) {
        self.arch_reset();

        use crate::kernel::vm_if_list_get_type;
        match vm_if_list_get_type(self.vm_id()) {
            VmType::VmTBma => {
                self.context_ext_regs_store();
            }
            _ => {}
        }
    }

    pub fn context_ext_regs_store(&mut self) {
        self.vm_ctx.ext_regs_store();
    }
}

use crate::board::PLATFORM_VCPU_NUM_MAX;
static VCPU_LIST: Mutex<Vec<Arc<Mutex<Vcpu>>>> = Mutex::new(Vec::new());

pub fn vcpu_alloc() -> Option<Arc<Mutex<Vcpu>>> {
    let mut vcpu_list = VCPU_LIST.lock();
    if vcpu_list.len() >= PLATFORM_VCPU_NUM_MAX {
        return None;
    }

    let val = Arc::new(Mutex::new(Vcpu::default()));
    vcpu_list.push(val.clone());
    Some(val)
}

pub fn vcpu_init(vm: &Vm, vcpu: &mut Vcpu, vcpu_id: usize) {
    vcpu.id = vcpu_id;
    vcpu.vm = Some(vm.clone());
    vcpu.phys_id = 0;
    crate::arch::vcpu_arch_init(vm, vcpu);
}

pub fn vcpu_idle() {
    crate::kernel::cpu_idle();
}
