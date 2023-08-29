use crate::kernel::timer;

pub fn current_us() -> usize {
    timer::gettime_ns() / 1000
}

pub fn current_ms() -> usize {
    timer::gettime_ns() / 10_usize.pow(6)
}

pub fn sleep(us: usize) {
    let end = current_us() + us;
    while current_us() < end {
        core::hint::spin_loop();
    }
}
