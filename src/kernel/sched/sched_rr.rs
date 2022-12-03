use alloc::vec::Vec;
use crate::kernel::{Vcpu, Scheduler, SchedulerUpdate, current_cpu, VcpuState, timer_enable, cpu_idle};

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

impl SchedulerRR {
    fn schedule_to(&mut self, next_vcpu: Vcpu) {
        {
            let queue = &self.queue;
            while queue.is_empty() {
                println!("Core {} comes IDLE because of scheduler empty", current_cpu().id);
                cpu_idle();
            }
            if queue.len() <= 1 {
                timer_enable(false);
            }
        }
        if let Some(prev_vcpu) = &current_cpu().active_vcpu {
            if prev_vcpu.vm_id() != next_vcpu.vm_id() {
                // println!(
                //     "next vm{} vcpu {}, prev vm{} vcpu {}",
                //     next_vcpu.vm_id(),
                //     next_vcpu.id(),
                //     prev_vcpu.vm_id(),
                //     prev_vcpu.id()
                // );
                prev_vcpu.set_state(VcpuState::VcpuPend);
                prev_vcpu.context_vm_store();
            }
        }
        current_cpu().set_active_vcpu(Some(next_vcpu.clone()));
        next_vcpu.context_vm_restore();
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
        self.schedule_to(next_vcpu);
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
        queue.push(vcpu.clone());
        if current_cpu().active_vcpu.is_none() {
            current_cpu().set_active_vcpu(Some(vcpu.clone()));
        }
        if queue.len() > 1 {
            timer_enable(true);
        }
    }

    fn yield_to(&mut self, vcpu: Vcpu) {
        let queue = &mut self.queue;
        queue.push(vcpu.clone());
        self.active_idx = queue.len() - 1;
        self.schedule_to(vcpu.clone());
        {
            if self.queue.len() > 1 {
                timer_enable(true);
            }
        }
    }
}

// #[cfg(feature = "update")]
impl SchedulerUpdate for SchedulerRR {
    fn update(&self) -> Self {
        let src_rr = self;
        let mut new_rr = SchedulerRR::default();
        for vcpu in src_rr.queue.iter() {
            new_rr.queue.push(vcpu.clone());
        }
        new_rr.active_idx = src_rr.active_idx;
        new_rr.base_slice = src_rr.base_slice;

        let active_vcpu = if src_rr.active_idx < src_rr.queue.len() {
            Some(new_rr.queue[src_rr.active_idx].clone())
        } else {
            None
        };
        if current_cpu().active_vcpu.is_none() {
            current_cpu().set_active_vcpu(active_vcpu);
        }
        new_rr
    }
}
