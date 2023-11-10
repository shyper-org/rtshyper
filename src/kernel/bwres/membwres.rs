use core::ptr::{self, NonNull};
use core::sync::atomic::AtomicU32;
use core::time::Duration;

use alloc::{boxed::Box, sync::Arc, vec::Vec};

use spin::Mutex;

use crate::{
    config,
    kernel::{
        current_cpu,
        timer::{now, start_timer_event},
    },
    util::timer_list::{TimerEvent, TimerValue},
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
        self.inner.next(consume as f32) as u32
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
    period: TimerValue,
    manager: NonNull<ReclaimManager>,
}

impl TimerEvent for ReclaimManagerRef {
    fn callback(self: alloc::sync::Arc<Self>, _now: crate::util::timer_list::TimerValue) {
        let mut val = unsafe { self.manager.as_ref() }.val.lock();
        if *val != 0 {
            trace!("reset GLOBAL_RECLAIM_MANAGER budget {} to 0", *val);
            *val = 0;
        }
        crate::kernel::timer::start_timer_event(self.period, self); // equals to DEFAULT_MEMORY_REPLENISHMENT_PERIOD
    }
}

static GLOBAL_RECLAIM_MANAGER: ReclaimManager = ReclaimManager::new();

#[allow(dead_code)]
fn apply_budget(budget: usize) -> usize {
    const MIN_BUDGET: usize = 10_usize.pow(5);

    let mut val = GLOBAL_RECLAIM_MANAGER.val.lock();
    if budget == 0 {
        // `budget == 0`: vcpu run out of its all budget (including prediction and donation)
        // Try get all the relaimed budget
        // TODO: add a more suitable method to apply excess budget
        let apply = *val;
        *val = 0;
        apply
    } else {
        // vcpu apply budget that it donate at this period
        let apply = usize::min(budget, *val);
        *val -= apply;
        // At least apply MIN_BUDGET to reduce frequent interrupt overhead
        usize::max(apply, MIN_BUDGET)
    }
}

fn giveup_budget(budget: usize) {
    let mut val = GLOBAL_RECLAIM_MANAGER.val.lock();
    *val += budget;
    trace!("GLOBAL_RECLAIM_MANAGER now has {} budget", *val);
}

const CACHE_LINE_SIZE: usize = 64;

pub struct MemoryBandwidth {
    budget: u32,
    period: Duration,
    last_predict_budget: AtomicU32,
    remaining_budget: AtomicU32,
    used_budget: AtomicU32,
    predictor: Mutex<BudgetPredictor>,
}

impl MemoryBandwidth {
    pub fn new(budget: u32, period: Duration) -> Self {
        Self {
            budget,
            period,
            last_predict_budget: AtomicU32::new(budget),
            remaining_budget: AtomicU32::new(budget),
            used_budget: AtomicU32::new(0),
            predictor: Mutex::new(BudgetPredictor::new()),
        }
    }

    pub fn period(&self) -> Duration {
        self.period
    }

    pub fn remaining_budget(&self) -> u32 {
        atomic_read_relaxed!(self.remaining_budget)
    }

    pub fn update_remaining_budget(&self, remaining_budget: u32) {
        let prev_remain = atomic_swap_relaxed!(self.remaining_budget, remaining_budget);
        use core::sync::atomic::Ordering::Relaxed;
        // Only update `used_budget` when the `remaining_budget` decreases
        if prev_remain > remaining_budget {
            self.used_budget.fetch_add(prev_remain - remaining_budget, Relaxed);
        }
    }

    pub fn reset_remaining_budget(&self) {
        self.update_remaining_budget(0);
    }

    pub fn used_budget(&self) -> u32 {
        atomic_read_relaxed!(self.used_budget)
    }

    pub fn supply_budget(&self) {
        let next_budget = if cfg!(feature = "dynamic-budget") {
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
            next_budget
        } else {
            self.budget
        };
        atomic_write_relaxed!(self.last_predict_budget, next_budget);

        atomic_write_relaxed!(self.remaining_budget, next_budget);

        atomic_write_relaxed!(self.used_budget, 0);
    }

    #[cfg(feature = "dynamic-budget")]
    pub fn budget_try_rescue(&self) -> bool {
        let donate = self.budget - atomic_read_relaxed!(self.last_predict_budget);
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
        let consume = self.used_budget();
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
    let g_mem_size = crate::kernel::get_llc_size() * 4;
    let workingset_size = g_mem_size / CACHE_LINE_SIZE;

    static_assert!(core::mem::size_of::<ListNode<usize>>() == CACHE_LINE_SIZE);

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
    let start = now();
    for _ in 0..repeat_time {
        let mut pos = head;
        while let Some(p) = unsafe { pos.as_ref() } {
            readsum += p.data;
            pos = p.next;
        }
    }
    let end = now();

    let nsdiff = (end - start).as_nanos() as usize;
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
            period: core::time::Duration::from_millis(100),
            manager: NonNull::new(ptr::addr_of!(GLOBAL_RECLAIM_MANAGER) as *mut _).unwrap(),
        });
        start_timer_event(reclaim_manager_timer_event.period, reclaim_manager_timer_event);
    }
}
