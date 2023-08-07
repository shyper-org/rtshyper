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
    type SchedItem = Vcpu;

    fn name(&self) -> &'static str {
        "Round Robin"
    }

    fn init(&mut self) {}

    fn next(&mut self) -> Option<Self::SchedItem> {
        self.queue.pop_front()
    }

    fn remove(&mut self, item: &Self::SchedItem) {
        if let Some(idx) = self.queue.iter().position(|x| x.eq(item)) {
            self.queue.remove(idx);
        }
    }

    fn put(&mut self, item: Self::SchedItem) {
        self.queue.push_back(item);
    }
}
