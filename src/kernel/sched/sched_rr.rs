use alloc::vec::Vec;
use crate::kernel::{Vcpu, current_cpu, VcpuState, timer_enable};

use super::Scheduler;

#[allow(dead_code)]
#[derive(Default)]
pub struct SchedulerRR {
    queue: Vec<Vcpu>,
    active_idx: usize,
    base_slice: usize,
}

impl SchedulerRR {
    pub fn new(slice: usize) -> Self {
        Self {
            queue: Vec::new(),
            active_idx: 0,
            base_slice: slice,
        }
    }
}

impl Scheduler for SchedulerRR {
    fn init(&mut self) {}

    fn next(&mut self) -> Option<Vcpu> {
        let queue = &self.queue;
        let len = queue.len();
        for i in 1..=len {
            let idx = (self.active_idx + i) % len;
            match queue.get(idx) {
                Some(vcpu) => {
                    if vcpu.state() != VcpuState::Inv {
                        self.active_idx = idx;
                        return Some(vcpu.clone());
                    }
                }
                None => break,
            }
        }
        None
    }

    fn sleep(&mut self, vcpu: Vcpu) {
        // println!(
        //     "SchedulerRR: Core {} sleep VM[{}] vcpu {}",
        //     current_cpu().id,
        //     vcpu.vm_id(),
        //     vcpu.id()
        // );
        let mut need_schedule = false;
        {
            let queue = &mut self.queue;
            if let Some(idx) = queue.iter().position(|x| x.vm_id() == vcpu.vm_id()) {
                queue.remove(idx);
                if idx < self.active_idx {
                    self.active_idx -= 1;
                } else if idx == self.active_idx {
                    // cpu.active_vcpu need remove
                    current_cpu().set_active_vcpu(None);
                    if !queue.is_empty() {
                        need_schedule = true;
                    }
                }
            }
        }
        if self.queue.len() <= 1 {
            timer_enable(false);
        }
        if need_schedule {
            self.do_schedule();
        }
    }

    fn wakeup(&mut self, vcpu: Vcpu) {
        let queue = &mut self.queue;
        vcpu.set_state(VcpuState::Runnable);
        queue.push(vcpu);
        if queue.len() == 1 {
            self.do_schedule();
        } else if queue.len() > 1 {
            timer_enable(true);
        }
    }
}
