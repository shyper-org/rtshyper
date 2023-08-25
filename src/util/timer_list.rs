use core::cmp::{Ordering, Reverse};

use alloc::collections::BinaryHeap;
use alloc::sync::Arc;

use super::downcast::Downcast;

pub type TimerTickValue = u64;

pub trait TimerEvent: Downcast {
    fn callback(self: Arc<Self>, now: TimerTickValue);
}

struct TimerEventWrapper {
    timeout_tick: TimerTickValue,
    event: Arc<dyn TimerEvent>,
}

impl PartialOrd for TimerEventWrapper {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.timeout_tick.partial_cmp(&other.timeout_tick)
    }
}

impl Ord for TimerEventWrapper {
    fn cmp(&self, other: &Self) -> Ordering {
        self.timeout_tick.cmp(&other.timeout_tick)
    }
}

impl PartialEq for TimerEventWrapper {
    fn eq(&self, other: &Self) -> bool {
        self.timeout_tick == other.timeout_tick
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

    pub fn push(&mut self, timeout_tick: TimerTickValue, event: Arc<dyn TimerEvent>) {
        self.events.push(Reverse(TimerEventWrapper { timeout_tick, event }));
    }

    pub fn pop(&mut self, current_tick: TimerTickValue) -> Option<(TimerTickValue, Arc<dyn TimerEvent>)> {
        if let Some(e) = self.events.peek() {
            if e.0.timeout_tick <= current_tick {
                return self.events.pop().map(|e| (e.0.timeout_tick, e.0.event));
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
