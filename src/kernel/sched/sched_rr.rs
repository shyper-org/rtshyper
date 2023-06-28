use alloc::collections::VecDeque;
use crate::kernel::Vcpu;

use super::Scheduler;

#[derive(Default)]
pub struct SchedulerRR {
    queue: VecDeque<Vcpu>,
    #[allow(unused)]
    base_slice: usize,
}

impl SchedulerRR {
    pub fn new(slice: usize) -> Self {
        Self {
            queue: VecDeque::new(),
            base_slice: slice,
        }
    }
}

impl Scheduler for SchedulerRR {
    fn init(&mut self) {}

    fn next(&mut self) -> Option<Vcpu> {
        self.queue.pop_front()
    }

    fn remove(&mut self, vcpu: &Vcpu) {
        let queue = &mut self.queue;
        if let Some(idx) = queue.iter().position(|x| x.eq(vcpu)) {
            queue.remove(idx);
        }
    }

    fn put(&mut self, vcpu: Vcpu) {
        let queue = &mut self.queue;
        queue.push_back(vcpu);
    }
}
