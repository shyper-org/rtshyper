mod sched_rr;
// mod sched_rt;

pub use self::sched_rr::SchedulerRR;
// pub use self::sched_rt::SchedulerRT;

use super::{current_cpu, Vcpu};

pub trait Scheduler {
    fn init(&mut self);
    /* pick the next vcpu object */
    fn next(&mut self) -> Option<Vcpu>;
    /* yield current vcpu */
    fn do_schedule(&mut self) {
        if let Some(next_vcpu) = self.next() {
            current_cpu().schedule_to(next_vcpu);
        }
    }
    /* put vcpu into sleep, and remove it from scheduler */
    fn sleep(&mut self, vcpu: Vcpu);
    /* wake up vcpu from sleep status, remember to set_active_vcpu when it is none */
    fn wakeup(&mut self, vcpu: Vcpu);
}
