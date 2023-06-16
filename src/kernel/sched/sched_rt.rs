use alloc::collections::LinkedList;

use crate::kernel::Vcpu;

use super::Scheduler;

#[derive(Default)]
pub struct SchedulerRT {
    run_queue: LinkedList<Vcpu>,      /* ordered list of runnable units */
    depleted_queue: LinkedList<Vcpu>, /* unordered list of depleted units */
}

impl SchedulerRT {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Scheduler for SchedulerRT {
    fn init(&mut self) {}

    fn next(&mut self) -> Option<Vcpu> {
        todo!()
    }

    fn sleep(&mut self, vcpu: Vcpu) {
        todo!()
    }

    fn wakeup(&mut self, vcpu: Vcpu) {
        todo!()
    }
}
