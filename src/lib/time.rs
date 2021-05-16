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
