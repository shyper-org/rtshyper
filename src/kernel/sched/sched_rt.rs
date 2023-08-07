/*
 * See Xen xen/common/sched/rt.c
 */

use core::{cell::Cell, ptr::NonNull};
use alloc::{
    collections::{LinkedList, BinaryHeap},
    sync::Arc,
};

use crate::{
    kernel::{
        Vcpu, VcpuState, current_cpu,
        timer::{start_timer_event, remove_timer_event},
    },
    util::timer_list::{TimerEvent, TimerTickValue},
};

use super::Scheduler;

pub struct SchedulerRT {
    run_queue: BinaryHeap<Arc<SchedUnit>>,      /* ordered list of runnable units */
    depleted_queue: LinkedList<Arc<SchedUnit>>, /* unordered list of depleted units */

    replenishment_queue: BinaryHeap<Arc<SchedUnit>>, /* units that need replenishment */

    self_ref: SchedulerRTRef,
}

#[derive(Clone, PartialEq, Eq)]
#[repr(transparent)]
struct SchedulerRTRef(NonNull<SchedulerRT>);

impl TimerEvent for SchedulerRTRef {
    fn callback(self: Arc<Self>, now: TimerTickValue) {
        let scheduler = unsafe { &mut *(self.0.as_ptr() as *mut SchedulerRT) };
        scheduler.repl_timer_handler(now);
    }
}

const DEFAULT_PERIOD: usize = 10;
const DEFAULT_BUDGET: usize = 4;

type SchedItemInner = Vcpu;

#[derive(PartialEq, Eq)]
struct SchedUnit {
    item: SchedItemInner,
    budget: usize,
    period: usize,

    current_budget: Cell<usize>,
    last_start: Cell<usize>,
    current_deadline: Cell<usize>,

    priority: Cell<usize>,
}

impl SchedUnit {
    fn new(item: SchedItemInner) -> Self {
        Self {
            item,
            budget: DEFAULT_BUDGET,
            period: DEFAULT_PERIOD,

            priority: Cell::new(0),
            current_budget: Cell::new(0),
            last_start: Cell::new(0),
            current_deadline: Cell::new(0),
        }
    }
}

impl SchedUnit {
    fn update_deadline(&self, now: TimerTickValue) {
        debug_assert!(self.period != 0);
        assert!(now as usize >= self.current_deadline.get());

        let count = (now as usize - self.current_deadline.get()) / self.period + 1;
        self.current_deadline
            .set(self.current_deadline.get() + count * self.period);

        self.current_budget.set(self.budget);
        self.last_start.set(now as usize);
        self.priority.set(0);
    }
}

impl PartialOrd for SchedUnit {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SchedUnit {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        match self.priority.cmp(&other.priority) {
            i if i != core::cmp::Ordering::Equal => i,
            _ => self.current_budget.cmp(&other.current_budget),
        }
    }
}

impl SchedulerRT {
    pub fn new() -> Self {
        let mut this = Self {
            run_queue: Default::default(),
            depleted_queue: Default::default(),
            replenishment_queue: Default::default(),
            self_ref: SchedulerRTRef(NonNull::dangling()),
        };
        this.self_ref = SchedulerRTRef(NonNull::new(&mut this).unwrap());
        this
    }
}

fn current_time() -> u64 {
    current_cpu().sys_tick
}

impl Scheduler for SchedulerRT {
    type SchedItem = SchedItemInner;

    fn name(&self) -> &'static str {
        "EDF (per-core)"
    }

    fn init(&mut self) {}

    fn next(&mut self) -> Option<Self::SchedItem> {
        if let Some(unit) = self.run_queue.pop() {
            self.burn_budget(&unit);
            Some(unit.item.clone())
        } else {
            None
        }
    }

    fn remove(&mut self, item: &Self::SchedItem) {
        self.run_queue.retain(|unit| &unit.item != item);
        self.depleted_queue.drain_filter(|unit| &unit.item == item);
        self.replenishment_queue_remove(item);
    }

    fn put(&mut self, item: Self::SchedItem) {
        let item_state = item.state();
        let unit = SchedUnit::new(item);

        let now = current_time();
        if now as usize >= unit.current_deadline.get() {
            unit.update_deadline(now);
        }

        let unit = Arc::new(unit);
        self.replenishment_queue_insert(unit.clone());
        if item_state != VcpuState::Running {
            self.run_queue_push(unit);
        }
    }
}

impl SchedulerRT {
    fn remove_timer(&self) {
        remove_timer_event(|event| {
            if let Some(event) = event.as_any().downcast_ref::<SchedulerRTRef>() {
                &self.self_ref == event
            } else {
                false
            }
        });
    }

    fn burn_budget(&mut self, unit: &SchedUnit) {
        let now = current_time() as usize;
        let delta = now - unit.last_start.get();

        unit.last_start.set(now);

        if unit.current_budget.get() < delta {
            // mark the unit run out of its budget
            unit.current_budget.set(0);
        } else {
            unit.current_budget.set(unit.current_budget.get() - delta);
        }
    }

    fn on_queue(&self, other: &SchedUnit) -> bool {
        self.run_queue.iter().any(|unit| unit.as_ref() == other)
            || self.depleted_queue.iter().any(|unit| unit.as_ref() == other)
    }

    fn queue_remove(&mut self, other: &SchedUnit) {
        self.run_queue.retain(|unit| unit.as_ref() != other);
        self.depleted_queue.drain_filter(|unit| unit.as_ref() == other);
    }

    fn run_queue_push(&mut self, unit: Arc<SchedUnit>) {
        if unit.current_budget.get() > 0 {
            self.run_queue.push(unit);
        } else {
            self.depleted_queue.push_back(unit);
        }
    }

    fn replenishment_queue_insert(&mut self, unit: Arc<SchedUnit>) {
        if let Some(current_peek) = self.replenishment_queue.peek() {
            if &unit > current_peek {
                self.remove_timer();
                start_timer_event(unit.current_deadline.get() as u64, Arc::new(self.self_ref.clone()));
            }
        }
        self.replenishment_queue.push(unit);
    }

    fn replenishment_queue_remove(&mut self, item: &SchedItemInner) {
        if self.replenishment_queue.iter().any(|unit| &unit.item == item) {
            self.remove_timer();
            self.replenishment_queue.retain(|unit| &unit.item != item);
            if let Some(peek) = self.replenishment_queue.peek() {
                start_timer_event(peek.current_deadline.get() as u64, Arc::new(self.self_ref.clone()));
            }
        } else {
            error!("replenishment_queue_remove VM {} vcpu {}", item.vm_id(), item.id());
        }
    }

    fn repl_timer_handler(&mut self, now: TimerTickValue) {
        let mut tmp_queue = LinkedList::new();
        /*
         * Do the replenishment and move replenished units
         * to the temporary list to tickle.
         * If unit is on run queue, we need to put it at
         * the correct place since its deadline changes.
         */
        while let Some(unit) = self.replenishment_queue.pop() {
            if (now as usize) < unit.current_deadline.get() {
                break;
            }
            unit.update_deadline(now);
            self.queue_remove(&unit);
            tmp_queue.push_back(unit.clone());

            if self.on_queue(&unit) {
                self.queue_remove(&unit);
                self.run_queue_push(unit);
            }
        }
        for unit in tmp_queue {
            self.replenishment_queue.push(unit)
        }
        // if the replenishment queue is not empty
        if let Some(unit) = self.replenishment_queue.peek() {
            // set the timer
            start_timer_event(unit.current_deadline.get() as u64, Arc::new(self.self_ref.clone()));
        }
    }
}
