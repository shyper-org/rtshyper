mod sched_rr;
// mod sched_rt;

use alloc::boxed::Box;

use crate::board::SchedRule;

use super::Vcpu;

pub trait Scheduler {
    fn init(&mut self);
    /* pop the next vcpu object */
    fn next(&mut self) -> Option<Vcpu>;
    /* remove vcpu from scheduler */
    fn remove(&mut self, vcpu: &Vcpu);
    /* put a new vcpu into the scheduler */
    fn put(&mut self, vcpu: Vcpu);
}

// factory mode
pub fn get_scheduler(rule: SchedRule) -> Box<dyn Scheduler> {
    match rule {
        SchedRule::RoundRobin => Box::new(sched_rr::SchedulerRR::new(1)),
        // SchedRule::RealTime => Box::new(sched_rt::SchedulerRT::new()),
        _ => todo!(),
    }
}
