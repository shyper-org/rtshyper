use super::{Vm, VmInner};
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
    pub vm: Option<Arc<Mutex<VmInner>>>,
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
    vcpu.vm = Some(vm.inner());
    // TODO: vcpu.vm
    vcpu.phys_id = 0;
    // crate::arch::vcpu_arch_init(vm, vcpu);
}
