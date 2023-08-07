mod sched_rr;
mod sched_rt;

use alloc::boxed::Box;

use crate::board::SchedRule;

use super::Vcpu;

pub trait Scheduler {
    type SchedItem;
    /* full name for this scheduler */
    fn name(&self) -> &'static str;
    /* initialize the scheduler */
    fn init(&mut self);
    /* pop the next item object */
    fn next(&mut self) -> Option<Self::SchedItem>;
    /* remove item from scheduler */
    fn remove(&mut self, item: &Self::SchedItem);
    /* put a new item into the scheduler */
    fn put(&mut self, item: Self::SchedItem);
}

// factory mode
pub fn get_scheduler(rule: SchedRule) -> Box<dyn Scheduler<SchedItem = Vcpu>> {
    match rule {
        SchedRule::RoundRobin => Box::new(sched_rr::SchedulerRR::new(1)),
        SchedRule::RealTime => Box::new(sched_rt::SchedulerRT::new()),
    }
}
