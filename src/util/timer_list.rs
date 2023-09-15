use core::cmp::{Ordering, Reverse};
use core::time::Duration;

use alloc::collections::BinaryHeap;
use alloc::sync::Arc;

use super::downcast::Downcast;

pub type TimerValue = Duration;

pub trait TimerEvent: Downcast {
    fn callback(self: Arc<Self>, now: TimerValue);
}

struct TimerEventWrapper {
    timeout: TimerValue,
    event: Arc<dyn TimerEvent>,
}

impl PartialOrd for TimerEventWrapper {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.timeout.partial_cmp(&other.timeout)
    }
}

impl Ord for TimerEventWrapper {
    fn cmp(&self, other: &Self) -> Ordering {
        self.timeout.cmp(&other.timeout)
    }
}

impl PartialEq for TimerEventWrapper {
    fn eq(&self, other: &Self) -> bool {
        self.timeout == other.timeout
    }
}

impl Eq for TimerEventWrapper {}

#[derive(Default)]
pub struct TimerList {
    events: BinaryHeap<Reverse<TimerEventWrapper>>, // reverse ordering for Min-heap
}

#[allow(dead_code)]
impl TimerList {
    pub fn new() -> Self {
        Self {
            events: BinaryHeap::new(),
        }
    }

    pub fn push(&mut self, timeout: TimerValue, event: Arc<dyn TimerEvent>) {
        self.events.push(Reverse(TimerEventWrapper { timeout, event }));
    }

    pub fn pop(&mut self, current_time: TimerValue) -> Option<(TimerValue, Arc<dyn TimerEvent>)> {
        if let Some(e) = self.events.peek() {
            if e.0.timeout <= current_time {
                return self.events.pop().map(|e| (e.0.timeout, e.0.event));
            }
        }
        None
    }

    pub fn remove_all<F>(&mut self, condition: F)
    where
        F: Fn(&Arc<dyn TimerEvent>) -> bool,
    {
        self.events.retain(|e| !condition(&e.0.event));
    }
}
