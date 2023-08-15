use core::ptr;

use alloc::{vec::Vec, boxed::Box};

// use spin::Mutex;

use crate::{kernel::timer::gettime_ns, config};

// struct ReclaimManager {
//     val: usize,
// }

// impl ReclaimManager {
//     const fn new() -> Self {
//         Self { val: 0 }
//     }
// }

// static GLOBAL_RECLAIM_MANAGER: Mutex<ReclaimManager> = Mutex::new(ReclaimManager::new());

// pub fn apply_budget() -> usize {
//     0
// }

// pub fn giveup_budget(_budget: usize) {
//     todo!()
// }

const CACHE_LINE_SIZE: usize = 64;

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
    let g_mem_size = super::get_llc_size() * 2;
    let workingset_size = g_mem_size / CACHE_LINE_SIZE;

    // initialize
    let mut item_buffer = (0..workingset_size)
        .map(|i| Box::new(ListNode::new(i)))
        .collect::<Vec<_>>();
    let mut rng = fastrand::Rng::with_seed(0);
    let mut perm = (0..workingset_size).collect::<Vec<_>>();
    for i in 0..perm.len() {
        let next = rng.usize(0..perm.len());
        perm.swap(i, next);
    }

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
        while !pos.is_null() {
            unsafe {
                readsum += (*pos).data;
                pos = (*pos).next;
            }
        }
    }
    let end = gettime_ns();

    let nsdiff = end - start;
    let avglat = nsdiff / workingset_size / repeat_time;
    info!("duration {nsdiff} ns, average {avglat} ns, readsum {readsum}");
    info!("bandwidth {} MB/s", CACHE_LINE_SIZE * 1000 / avglat);
    avglat
}

pub fn init() {
    // Multiply by FACTOR is an empirical value, and round up to 100 for human readability
    let avglat = latency_bench(DEFAULT_ITER);
    const HUNDRED: usize = 100;
    const FACTOR: usize = 4;
    let mem_rand_read_per_sec = crate::util::round_up(FACTOR * 10_usize.pow(9) / avglat, HUNDRED);
    info!("memory random read: {mem_rand_read_per_sec} times per second");

    config::set_memory_budget_second(mem_rand_read_per_sec as u32);
}
