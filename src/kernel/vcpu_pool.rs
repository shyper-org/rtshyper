use crate::kernel::{Vcpu, VcpuState};
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;

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

use crate::kernel::{set_cpu_vcpu_pool, CPU};
pub fn vcpu_pool_init() {
    set_cpu_vcpu_pool(Box::new(VcpuPool::default()));
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
