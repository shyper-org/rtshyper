use core::arch::global_asm;

use volatile::Volatile;

use crate::board::PLAT_DESC;
use crate::lib::round_up;

global_asm!(include_str!("../arch/aarch64/barrier.S"));

#[repr(C)]
struct CpuSyncToken {
    lock: u32,
    n: usize,
    count: usize,
    ready: bool,
}

static mut CPU_GLB_SYNC: CpuSyncToken = CpuSyncToken {
    lock: 0,
    n: PLAT_DESC.cpu_desc.num,
    count: 0,
    ready: true,
};

static mut CPU_FUNC_SYNC: CpuSyncToken = CpuSyncToken {
    lock: 0,
    n: 0,
    count: 0,
    ready: true,
};

extern "C" {
    pub fn spin_lock(lock: usize);
    pub fn spin_unlock(lock: usize);
}

#[inline(never)]
pub fn barrier() {
    unsafe {
        let lock_addr = &CPU_GLB_SYNC.lock as *const _ as usize;
        spin_lock(lock_addr);
        let mut count = Volatile::new(&mut CPU_GLB_SYNC.count);
        count.update(|count| *count += 1);
        let next_count = round_up(count.read(), CPU_GLB_SYNC.n);
        spin_unlock(lock_addr);
        while count.read() < next_count {}
    }
}

#[inline(never)]
pub fn func_barrier() {
    unsafe {
        let lock_addr = &CPU_FUNC_SYNC.lock as *const _ as usize;
        spin_lock(lock_addr);
        let mut count = Volatile::new(&mut CPU_FUNC_SYNC.count);
        count.update(|count| *count += 1);
        let next_count = round_up(count.read(), CPU_FUNC_SYNC.n);
        spin_unlock(lock_addr);
        while count.read() < next_count {}
    }
}

pub fn set_barrier_num(num: usize) {
    unsafe {
        let mut n = Volatile::new(&mut CPU_FUNC_SYNC.n);
        n.update(|n| *n = num);
    }
}
