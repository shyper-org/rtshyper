use core::sync::atomic::{AtomicUsize, Ordering};

use tock_registers::interfaces::*;

const CTL_IMASK: usize = 1 << 1;

static TIMER_FREQ: AtomicUsize = AtomicUsize::new(0);
static TIMER_SLICE: AtomicUsize = AtomicUsize::new(0); // ms

pub fn timer_arch_set(num: usize) {
    let slice = TIMER_SLICE.load(Ordering::Relaxed);
    let val = slice * num;
    msr!(CNTHP_TVAL_EL2, val);
}

pub fn timer_arch_enable_irq() {
    let val = 1;
    msr!(CNTHP_CTL_EL2, val, "x");
}

pub fn timer_arch_disable_irq() {
    let val = 2;
    msr!(CNTHP_CTL_EL2, val, "x");
}

pub fn timer_arch_get_counter() -> usize {
    cortex_a::registers::CNTPCT_EL0.get() as usize
}

pub fn timer_arch_get_frequency() -> usize {
    cortex_a::registers::CNTFRQ_EL0.get() as usize
}

pub fn timer_arch_init() {
    let freq = timer_arch_get_frequency();
    let slice = freq / 1000; // ms
    TIMER_FREQ.store(freq, Ordering::Relaxed);
    TIMER_SLICE.store(slice, Ordering::Relaxed);

    let ctl = 0x3 & (1 | !CTL_IMASK);
    let tval = slice * 10;
    msr!(CNTHP_CTL_EL2, ctl);
    msr!(CNTHP_TVAL_EL2, tval);
}
