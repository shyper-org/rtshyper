use crate::kernel::{Vcpu, Scheduler};

pub struct SchedulerRT {}

impl Scheduler for SchedulerRT {
    fn init(&mut self) {
        todo!()
    }

    fn next(&mut self) -> Option<Vcpu> {
        todo!()
    }

    fn do_schedule(&mut self) {
        todo!()
    }

    fn sleep(&mut self, vcpu: Vcpu) {
        todo!()
    }

    fn wakeup(&mut self, vcpu: Vcpu) {
        todo!()
    }

    fn yield_to(&mut self, vcpu: Vcpu) {
        todo!()
    }
}
