use core::ptr::{self, NonNull};
use core::sync::atomic::{AtomicU32, Ordering::Relaxed};

use alloc::{vec::Vec, boxed::Box, sync::Arc};

use spin::Mutex;

use crate::{
    kernel::{
        timer::{gettime_ns, start_timer_event},
        current_cpu,
    },
    config,
    util::timer_list::{TimerEvent, TimerTickValue},
};

use super::ema::ExponentialMovingAverage;

pub struct BudgetPredictor {
    inner: ExponentialMovingAverage,
}

impl BudgetPredictor {
    pub fn new() -> Self {
        const MAX_HISTORY: usize = 10;
        Self {
            inner: ExponentialMovingAverage::new(MAX_HISTORY).unwrap(),
        }
    }

    pub fn predict(&mut self, consume: u32) -> u32 {
        self.inner.next(consume as usize) as u32
    }
}

struct ReclaimManager {
    val: Mutex<usize>,
}

impl ReclaimManager {
    const fn new() -> Self {
        Self { val: Mutex::new(0) }
    }
}

struct ReclaimManagerRef {
    period: TimerTickValue,
    manager: NonNull<ReclaimManager>,
}

impl TimerEvent for ReclaimManagerRef {
    fn callback(self: alloc::sync::Arc<Self>, _now: crate::util::timer_list::TimerTickValue) {
        let manager = unsafe { self.manager.as_ref() };
        *manager.val.lock() = 0;
        crate::kernel::timer::start_timer_event(self.period, self); // equals to DEFAULT_MEMORY_REPLENISHMENT_PERIOD
    }
}

static GLOBAL_RECLAIM_MANAGER: ReclaimManager = ReclaimManager::new();

fn apply_budget(budget: usize) -> usize {
    const MIN_BUDGET: usize = 10_usize.pow(5);

    if budget == 0 {
        return 0;
    }

    let mut val = GLOBAL_RECLAIM_MANAGER.val.lock();
    if *val >= budget {
        *val -= budget;
        budget
    } else {
        let apply = *val;
        *val = 0;
        usize::min(apply, MIN_BUDGET)
    }
}

fn giveup_budget(budget: usize) {
    *GLOBAL_RECLAIM_MANAGER.val.lock() += budget;
}

const CACHE_LINE_SIZE: usize = 64;

pub struct MemoryBandwidth {
    budget: u32,
    period: u64,
    last_predict_budget: AtomicU32,
    remaining_budget: AtomicU32,
    predictor: Mutex<BudgetPredictor>,
}

impl MemoryBandwidth {
    pub fn new(budget: u32, period: u64) -> Self {
        Self {
            budget,
            period,
            last_predict_budget: AtomicU32::new(budget),
            remaining_budget: AtomicU32::new(budget),
            predictor: Mutex::new(BudgetPredictor::new()),
        }
    }

    pub fn period(&self) -> u64 {
        self.period
    }

    pub fn remaining_budget(&self) -> u32 {
        self.remaining_budget.load(Relaxed)
    }

    pub fn update_remaining_budget(&self, remaining_budget: u32) {
        self.remaining_budget.store(remaining_budget, Relaxed);
    }

    pub fn reset_remaining_budget(&self) {
        self.remaining_budget.store(0, Relaxed);
    }

    pub fn supply_budget(&self) {
        // Do prediction here
        let next_budget = u32::min(self.predict(), self.budget);
        let giveup = self.budget - next_budget;
        trace!(
            "predict budget {next_budget}, static allocated budget {}, giveup {giveup}",
            self.budget
        );
        if giveup > 0 {
            giveup_budget(giveup as usize);
        }

        self.remaining_budget.store(next_budget, Relaxed);
    }

    pub fn budget_try_rescue(&self) -> bool {
        let donate = self.budget - self.last_predict_budget.load(Relaxed);
        let apply = apply_budget(donate as usize) as u32;
        if apply != 0 {
            debug!("budget_try_rescue: apply {apply} additional budget");
            self.update_remaining_budget(apply);
            true
        } else {
            false
        }
    }

    fn predict(&self) -> u32 {
        let consume = self.last_predict_budget.load(Relaxed) - self.remaining_budget();
        self.predictor.lock().predict(consume)
    }
}

#[repr(align(64))] // CACHE_LINE_SIZE
struct ListNode<T> {
    data: T,
    next: *mut Self,
}

impl<T> ListNode<T> {
    fn new(data: T) -> Self {
        Self {
            data,
            next: ptr::null_mut(),
        }
    }
}

const DEFAULT_ITER: usize = 100;

fn latency_bench(repeat_time: usize) -> usize {
    let g_mem_size = crate::kernel::get_llc_size() * 2;
    let workingset_size = g_mem_size / CACHE_LINE_SIZE;

    const_assert_eq!(core::mem::size_of::<ListNode<usize>>(), CACHE_LINE_SIZE);

    // initialize
    let mut item_buffer = (0..workingset_size)
        .map(|i| Box::new(ListNode::new(i)))
        .collect::<Vec<_>>();
    let mut perm = (0..workingset_size).collect::<Vec<_>>();
    let mut rng = fastrand::Rng::with_seed(0);
    rng.shuffle(&mut perm);

    debug!("allocated: wokingsetsize={} entries", workingset_size);

    let mut head = ptr::null_mut();
    for i in perm {
        item_buffer[i].next = head;
        head = ptr::addr_of_mut!(*item_buffer[i]);
    }

    let mut readsum = 0;
    // actual access
    let start = gettime_ns();
    for _ in 0..repeat_time {
        let mut pos = head;
        while let Some(p) = unsafe { pos.as_ref() } {
            readsum += p.data;
            pos = p.next;
        }
    }
    let end = gettime_ns();

    let nsdiff = end - start;
    let avglat = nsdiff / workingset_size / repeat_time;
    info!("duration {nsdiff} ns, average {avglat} ns, readsum {readsum}");
    info!("bandwidth {} MB/s", CACHE_LINE_SIZE * 1000 / avglat);
    avglat
}

pub(super) fn init() {
    // Multiply by FACTOR is an empirical value, and round up to 100 for human readability
    let avglat = latency_bench(DEFAULT_ITER);
    const FACTOR: usize = 4;
    let mem_rand_read_per_sec = crate::util::round_up(FACTOR * 10_usize.pow(9) / avglat, 100);
    info!("memory random read: {mem_rand_read_per_sec} times per second");

    config::set_memory_budget_second(mem_rand_read_per_sec as u32);

    if current_cpu().id == 0 {
        let reclaim_manager_timer_event = Arc::new(ReclaimManagerRef {
            period: 10,
            manager: NonNull::new(ptr::addr_of!(GLOBAL_RECLAIM_MANAGER) as *mut _).unwrap(),
        });
        start_timer_event(reclaim_manager_timer_event.period, reclaim_manager_timer_event);
    }
}
