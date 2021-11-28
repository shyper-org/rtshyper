use alloc::boxed::Box;
use alloc::vec::Vec;

use crate::kernel::{current_cpu, Vcpu, VcpuState};
use crate::kernel::CPU;

pub const VCPU_POOL_MAX: usize = 4;

pub struct VcpuPoolContent {
    pub vcpu: Vcpu,
}

pub struct VcpuPool {
    pub content: Vec<VcpuPoolContent>,
    pub active_idx: usize,
    pub running: usize,
}

impl VcpuPool {
    fn default() -> VcpuPool {
        VcpuPool {
            content: Vec::new(),
            active_idx: 0,
            running: 0,
        }
    }

    fn append_vcpu(&mut self, vcpu: Vcpu) {
        self.content.push(VcpuPoolContent { vcpu });
        self.running += 1;
    }
}

pub fn vcpu_pool_init() {
    let pool = Box::new(VcpuPool::default());
    current_cpu().vcpu_pool = Some(pool);
}

pub fn vcpu_pool_append(vcpu: Vcpu) -> bool {
    if let Some(vcpu_pool) = unsafe { &mut CPU.vcpu_pool } {
        if vcpu_pool.content.len() >= VCPU_POOL_MAX {
            println!("can't append more vcpu!");
            return false;
        }
        vcpu.set_state(VcpuState::VcpuPend);

        vcpu_pool.append_vcpu(vcpu.clone());
    } else {
        panic!("CPU's vcpu pool is NULL");
    }
    true
}

pub fn vcpu_pool_pop_through_vmid(vm_id: usize) -> Option<Vcpu> {
    let vcpu_pool = current_cpu().vcpu_pool.as_ref().unwrap();
    let size = vcpu_pool.content.len();
    if size == 0 {
        println!("vcpu pool is empty");
        return None;
    }
    for idx in 0..size {
        let vcpu = vcpu_pool.content[idx].vcpu.clone();
        if vcpu.vm_id() == vm_id {
            return Some(vcpu);
        }
    }
    None
}
