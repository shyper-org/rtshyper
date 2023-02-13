use alloc::vec::Vec;
use crate::kernel::{Vcpu, Scheduler, current_cpu, VcpuState, timer_enable};

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

impl Default for SchedulerRR {
    fn default() -> Self {
        Self {
            queue: Default::default(),
            active_idx: Default::default(),
            base_slice: Default::default(),
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
                Some(vcpu) => match vcpu.state() {
                    VcpuState::VcpuInv => {}
                    _ => {
                        self.active_idx = idx;
                        return Some(vcpu.clone());
                    }
                },
                None => panic!("len != 0 but front is None"),
            }
        }
        None
    }

    fn do_schedule(&mut self) {
        let next_vcpu = self.next().unwrap();
        current_cpu().schedule_to(next_vcpu);
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
            match queue.iter().position(|x| x.vm_id() == vcpu.vm_id()) {
                Some(idx) => {
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
                None => {}
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
        vcpu.set_state(VcpuState::VcpuPend);
        queue.push(vcpu);
        if queue.len() > 1 {
            timer_enable(true);
        }
        if queue.len() == 1 {
            self.do_schedule();
        }
    }

    fn yield_to(&mut self, vcpu: Vcpu) {
        let queue = &mut self.queue;
        queue.push(vcpu.clone());
        self.active_idx = queue.len() - 1;
        current_cpu().schedule_to(vcpu);
        if queue.len() > 1 {
            timer_enable(true);
        }
    }
}
