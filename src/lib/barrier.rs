use core::ptr;
use core::sync::atomic::AtomicUsize;
use core::sync::atomic::Ordering;

use crate::board::PLAT_DESC;
use crate::lib::round_up;

struct CpuSyncToken {
    n: usize,
    count: AtomicUsize,
    ready: bool,
}

static mut CPU_GLB_SYNC: CpuSyncToken = CpuSyncToken {
    n: PLAT_DESC.cpu_desc.num,
    count: AtomicUsize::new(0),
    ready: true,
};

static mut CPU_FUNC_SYNC: CpuSyncToken = CpuSyncToken {
    n: 0,
    count: AtomicUsize::new(0),
    ready: true,
};

#[inline(never)]
pub fn barrier() {
    unsafe {
        let ori = CPU_GLB_SYNC.count.fetch_add(1, Ordering::Relaxed);
        let next_count = round_up(ori + 1, CPU_GLB_SYNC.n);
        while CPU_GLB_SYNC.count.load(Ordering::Acquire) < next_count {
            core::hint::spin_loop();
        }
    }
}

#[inline(never)]
pub fn func_barrier() {
    unsafe {
        let ori = CPU_FUNC_SYNC.count.fetch_add(1, Ordering::Relaxed);
        let next_count = round_up(ori + 1, CPU_FUNC_SYNC.n);
        while CPU_FUNC_SYNC.count.load(Ordering::Acquire) < next_count {
            core::hint::spin_loop();
        }
    }
}

pub fn set_barrier_num(num: usize) {
    unsafe {
        ptr::write_volatile(&mut CPU_FUNC_SYNC.n, num);
    }
}
