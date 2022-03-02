use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;

use spin::mutex::Mutex;

use crate::board::{PLAT_DESC, SchedRule};
use crate::kernel::{active_vcpu_id, active_vm_id, current_cpu, SchedType, SchedulerRR, Vcpu, VcpuInner, VcpuState};
use crate::kernel::CPU;

pub const VCPU_POOL_MAX: usize = 4;

#[derive(Clone)]
pub struct VcpuPoolContent {
    pub vcpu: Vcpu,
    pub time_slice: usize,
}

#[derive(Clone)]
pub struct VcpuPool {
    inner: Arc<Mutex<VcpuPoolInner>>,
    // sched_lock: Mutex<{}>,
}

impl VcpuPool {
    pub fn default() -> VcpuPool {
        VcpuPool {
            inner: Arc::new(Mutex::new(VcpuPoolInner::default(1)))
            // sched_lock: Mutex::new({}),
        }
    }

    // return true if need to change another vcpu
    // pub fn schedule(&self) -> bool {
    //     let mut pool = self.inner.lock();
    //     let active_idx = pool.active_idx;
    //     if pool.content[active_idx].time_slice != 0 {
    //         pool.content[active_idx].time_slice -= 1;
    //         false
    //     } else {
    //         pool.content[active_idx].time_slice = pool.base_slice;
    //         true
    //     }
    // }

    pub fn slice(&self) -> usize {
        let pool = self.inner.lock();
        pool.base_slice
    }

    pub fn next_vcpu_idx(&self) -> usize {
        let mut pool = self.inner.lock();
        for i in (pool.active_idx + 1)..pool.content.len() {
            match pool.content[i].vcpu.state() {
                VcpuState::VcpuInv => {}
                _ => {
                    return i;
                }
            }
        }
        for i in 0..pool.active_idx {
            match pool.content[i].vcpu.state() {
                VcpuState::VcpuInv => {}
                _ => {
                    return i;
                }
            }
        }
        pool.active_idx
    }

    pub fn yield_vcpu(&self, idx: usize) {
        // println!("yield vcpu idx {}", idx);
        let mut pool = self.inner.lock();
        // for i in 0..pool.content.len() {
        //     println!("vm{} vcpu{} state {:#?}", pool.content[i].vcpu.vm_id(), pool.content[i].vcpu.id(), pool.content[i].vcpu.state());
        // }

        let active_idx = pool.active_idx;
        pool.content[active_idx].time_slice = pool.base_slice;

        let next_vcpu = pool.content[idx].vcpu.clone();
        if next_vcpu.phys_id() != current_cpu().id {
            panic!("illegal vcpu for cpu {}", current_cpu().id);
        }
        if current_cpu().active_vcpu.is_some() && next_vcpu.vm_id() == active_vm_id() {
            return;
        }
        if next_vcpu.state() as usize == VcpuState::VcpuInv as usize {
            pool.running += 1;
        }
        drop(pool);

        match &current_cpu().active_vcpu {
            None => {}
            Some(prev_vcpu) => {
                // println!("next vm{} vcpu {}, prev vm{} vcpu {}", next_vcpu.vm_id(), next_vcpu.id(), prev_vcpu.vm_id(), prev_vcpu.id());
                prev_vcpu.set_state(VcpuState::VcpuPend);
                prev_vcpu.context_vm_store();
            }
        }

        self.set_active_vcpu(idx);
        current_cpu().set_active_vcpu(next_vcpu.clone());

        next_vcpu.context_vm_restore();
        next_vcpu.inject_int_inlist();
    }

    pub fn next_vcpu(&self) -> Vcpu {
        let mut pool = self.inner.lock();
        for i in (pool.active_idx + 1)..pool.content.len() {
            match pool.content[i].vcpu.state() {
                VcpuState::VcpuInv => {}
                _ => {
                    return pool.content[i].vcpu.clone();
                }
            }
        }
        for i in 0..pool.active_idx {
            match pool.content[i].vcpu.state() {
                VcpuState::VcpuInv => {}
                _ => {
                    return pool.content[i].vcpu.clone();
                }
            }
        }
        pool.content[pool.active_idx].vcpu.clone()
    }

    pub fn running(&self) -> usize {
        let pool = self.inner.lock();
        pool.running
    }

    pub fn add_running(&self) {
        let mut pool = self.inner.lock();
        pool.running += 1;
    }

    pub fn pop_vcpu_through_vmid(&self, vmid: usize) -> Option<Vcpu> {
        let pool = self.inner.lock();
        for i in 0..pool.content.len() {
            let vcpu = pool.content[i].vcpu.clone();
            if vcpu.vm_id() == vmid {
                return Some(vcpu);
            }
        }
        None
    }

    pub fn pop_vcpuidx_through_vmid(&self, vmid: usize) -> Option<usize> {
        let pool = self.inner.lock();
        for i in 0..pool.content.len() {
            let vcpu = pool.content[i].vcpu.clone();
            if vcpu.vm_id() == vmid {
                return Some(i);
            }
        }
        None
    }

    pub fn vcpu(&self, idx: usize) -> Vcpu {
        let pool = self.inner.lock();
        if pool.content.len() <= idx {
            panic!("to large idx {} for vcpu_pool", idx);
        }
        pool.content[idx].vcpu.clone()
    }

    pub fn vcpu_num(&self) -> usize {
        let pool = self.inner.lock();
        pool.content.len()
    }

    pub fn set_active_vcpu(&self, idx: usize) -> Vcpu {
        let mut pool = self.inner.lock();
        if idx >= pool.content.len() {
            panic!("to large idx {} for vcpu_pool", idx);
        }
        let vcpu = pool.content[idx].vcpu.clone();
        pool.active_idx = idx;
        vcpu.set_state(VcpuState::VcpuAct);
        vcpu.clone()
    }

    pub fn append_vcpu(&self, vcpu: Vcpu) -> bool {
        let mut pool = self.inner.lock();
        if pool.content.len() >= VCPU_POOL_MAX {
            println!("can't append more vcpu!");
            return false;
        }
        pool.append_vcpu(vcpu.clone());

        true
    }
}

#[derive(Clone)]
struct VcpuPoolInner {
    pub content: Vec<VcpuPoolContent>,
    pub base_slice: usize,
    pub active_idx: usize,
    pub running: usize,
}

impl VcpuPoolInner {
    fn default(base_slice: usize) -> VcpuPoolInner {
        VcpuPoolInner {
            content: Vec::new(),
            base_slice,
            active_idx: 0,
            running: 0,
        }
    }

    fn append_vcpu(&mut self, vcpu: Vcpu) {
        self.content.push(VcpuPoolContent { vcpu, time_slice: self.base_slice });
    }
}

// Todo: add config for base slice
pub fn cpu_sched_init() {
    match PLAT_DESC.cpu_desc.sched_list[current_cpu().id] {
        SchedRule::RoundRobin => {
            println!("cpu[{}] init Round Robin Scheduler", current_cpu().id);
            current_cpu().sched = SchedType::SchedRR(SchedulerRR { pool: VcpuPool::default() })
        }
        _ => {
            todo!();
        }
    }
}

pub fn restore_vcpu_gic(cur_vcpu: Option<Vcpu>, trgt_vcpu: Vcpu) {
    // println!("restore_vcpu_gic");
    match cur_vcpu {
        None => {
            // println!("None cur vmid trgt {}", trgt_vcpu.vm_id());
            trgt_vcpu.gic_restore_context();
        }
        Some(active_vcpu) => {
            if trgt_vcpu.vm_id() != active_vcpu.vm_id() {
                // println!("different vm_id cur {}, trgt {}", active_vcpu.vm_id(), trgt_vcpu.vm_id());
                active_vcpu.gic_save_context();
                trgt_vcpu.gic_restore_context();
            }
        }
    }
}

pub fn save_vcpu_gic(cur_vcpu: Option<Vcpu>, trgt_vcpu: Vcpu) {
    // println!("save_vcpu_gic");
    match cur_vcpu {
        None => {
            trgt_vcpu.gic_save_context();
        }
        Some(active_vcpu) => {
            if trgt_vcpu.vm_id() != active_vcpu.vm_id() {
                trgt_vcpu.gic_save_context();
                active_vcpu.gic_restore_context();
            }
        }
    }
}