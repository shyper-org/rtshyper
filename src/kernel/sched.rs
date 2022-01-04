use crate::kernel::{current_cpu, Vcpu, VcpuPool, VcpuState};

pub enum SchedType {
    SchedRR(SchedulerRR),
    None,
}

pub trait SchedulerTrait {
    fn init(&mut self);
    fn next(&self) -> Vcpu;
    fn schedule(&self);
    fn yield_next(&self, idx: usize);
}

pub struct SchedulerRR {
    pub pool: VcpuPool,
}

impl SchedulerTrait for SchedulerRR {
    fn init(&mut self) {}

    fn next(&self) -> Vcpu {
        self.pool.next_vcpu()
    }

    fn schedule(&self) {
        // println!("in schedule");
        // if self.pool.running() <= 1 &&
        //     current_cpu().active_vcpu.as_ref().unwrap().state() as usize == VcpuState::VcpuAct as usize {
        //     return;
        // }
        if self.pool.schedule() {
            let next = self.pool.next_vcpu_idx();
            self.pool.yield_vcpu(next);
        }
    }

    fn yield_next(&self, next_idx: usize) {
        self.pool.yield_vcpu(next_idx);
    }
}