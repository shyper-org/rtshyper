use crate::kernel::timer;

pub fn sleep(us: usize) {
    let end = timer::now() + core::time::Duration::from_micros(us as u64);
    while timer::now() < end {
        core::hint::spin_loop();
    }
}
