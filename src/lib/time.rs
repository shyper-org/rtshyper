use core::arch::asm;

use crate::arch::{timer_arch_get_counter, timer_arch_get_frequency};

pub fn time_current_us() -> usize {
    let count = timer_arch_get_counter();
    let freq = timer_arch_get_frequency();
    count * 1000000 / freq
}

pub fn time_current_ms() -> usize {
    let count = timer_arch_get_counter();
    let freq = timer_arch_get_frequency();
    count * 1000 / freq
}

pub fn sleep(us: usize) {
    // let end = time_current_us() + us;
    // while time_current_us() < end {
    //     unsafe {
    //         // asm!("wfi");
    //         asm!("nop");
    //     }
    // }
    for _ in 0..us * 1000 {}
}
