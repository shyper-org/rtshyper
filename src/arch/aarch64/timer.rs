use core::arch::asm;

use spin::Mutex;
use tock_registers::interfaces::*;

const CTL_IMASK: usize = 1 << 1;

pub static TIMER_FREQ: Mutex<usize> = Mutex::new(0);
pub static TIMER_SLICE: Mutex<usize> = Mutex::new(0); // ms

pub fn timer_arch_set(num: usize) {
    let slice_lock = TIMER_SLICE.lock();
    let val = *slice_lock * num;
    drop(slice_lock);
    unsafe {
        asm!("msr CNTHP_TVAL_EL2, {0}", "isb", in(reg) val);
    };
}

pub fn timer_arch_enable_irq() {
    let val = 1;
    unsafe {
        asm!("msr CNTHP_CTL_EL2, {0:x}", "isb", in(reg) val);
    };
}

pub fn timer_arch_disable_irq() {
    let val = 2;
    unsafe {
        asm!("msr CNTHP_CTL_EL2, {0:x}", "isb", in(reg) val);
    };
}

pub fn timer_arch_get_counter() -> usize {
    cortex_a::registers::CNTPCT_EL0.get() as usize
}

pub fn timer_arch_get_frequency() -> usize {
    cortex_a::registers::CNTFRQ_EL0.get() as usize
}

pub fn timer_arch_init() {
    let mut freq_lock = TIMER_FREQ.lock();
    let mut slice_lock = TIMER_SLICE.lock();
    *freq_lock = timer_arch_get_frequency();
    *slice_lock = (*freq_lock) / 1000; // ms

    let ctl = 0x3 & (1 | !CTL_IMASK);
    let tval = *slice_lock * 10;
    unsafe {
        asm!("msr CNTHP_CTL_EL2, {0}", "isb", in(reg) ctl);
        asm!("msr CNTHP_TVAL_EL2, {0}", "isb", in(reg) tval);
    }
}
