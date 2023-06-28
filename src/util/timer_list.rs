use core::cmp::{PartialOrd, Ord, PartialEq, Ordering, Reverse};

use alloc::boxed::Box;
use alloc::collections::BinaryHeap;

pub type TimerTickValue = u64;

pub trait TimerEvent {
    fn callback(self: Box<Self>, now: TimerTickValue);
}

struct TimerEventWrapper {
    timeout_tick: TimerTickValue,
    event: Box<dyn TimerEvent>,
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

impl TimerList {
    pub fn new() -> Self {
        Self {
            events: BinaryHeap::new(),
        }
    }

    pub fn push(&mut self, timeout_tick: TimerTickValue, event: impl TimerEvent + 'static) {
        self.events.push(Reverse(TimerEventWrapper {
            timeout_tick,
            event: Box::new(event),
        }));
    }

    pub fn pop(&mut self, current_tick: TimerTickValue) -> Option<(TimerTickValue, Box<dyn TimerEvent>)> {
        if let Some(e) = self.events.peek() {
            if e.0.timeout_tick <= current_tick {
                return self.events.pop().map(|e| (e.0.timeout_tick, e.0.event));
            }
        }
        None
    }

    pub fn remove_all<F>(&mut self, condition: F)
    where
        F: Fn(&dyn TimerEvent) -> bool,
    {
        self.events.retain(|e| !condition(e.0.event.as_ref()));
    }
}

pub struct TimerEventFn(Box<dyn FnOnce(TimerTickValue) + 'static>);

impl TimerEventFn {
    #[allow(dead_code)]
    pub fn new<F>(f: F) -> Self
    where
        F: FnOnce(TimerTickValue) + 'static,
    {
        Self(Box::new(f))
    }
}

impl TimerEvent for TimerEventFn {
    fn callback(self: Box<TimerEventFn>, now: TimerTickValue) {
        (self.0)(now)
    }
}
