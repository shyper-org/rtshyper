use core::sync::atomic::{AtomicUsize, Ordering};

use tock_registers::interfaces::*;

const CTL_IMASK: usize = 1 << 1;

static TIMER_FREQ: AtomicUsize = AtomicUsize::new(0);
static TIMER_MS_TICKS: AtomicUsize = AtomicUsize::new(0); // ms
static TIMER_TICK_NS: AtomicUsize = AtomicUsize::new(0); // nano second in one timer tick

pub fn timer_arch_set(num: usize) {
    let slice = TIMER_MS_TICKS.load(Ordering::Relaxed);
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
    aarch64_cpu::registers::CNTPCT_EL0.get() as usize
}

pub fn timer_arch_get_frequency() -> usize {
    aarch64_cpu::registers::CNTFRQ_EL0.get() as usize
}

#[allow(dead_code)]
pub fn gettime_ns() -> usize {
    timer_arch_get_counter() * TIMER_TICK_NS.load(Ordering::Relaxed)
}

pub fn timer_arch_init() {
    let freq = timer_arch_get_frequency();
    let ticks_per_ms = freq / 1000; // ms
    TIMER_FREQ.store(freq, Ordering::Relaxed);
    TIMER_MS_TICKS.store(ticks_per_ms, Ordering::Relaxed);
    TIMER_TICK_NS.store(10usize.pow(9) / freq, Ordering::Relaxed);

    let ctl = 0x3 & (1 | !CTL_IMASK);
    let tval = ticks_per_ms * 10;
    msr!(CNTHP_CTL_EL2, ctl);
    msr!(CNTHP_TVAL_EL2, tval);
}

#[repr(C, align(16))]
#[derive(Debug, Copy, Clone)]
pub struct GenericTimerContext {
    cntvoff_el2: u64,   // Virtual Offset register
    cntkctl_el1: u64,   // Kernel Control register
    cntp_cval_el0: u64, // Physical Timer CompareValue register
    cntv_cval_el0: u64, // Virtual Timer CompareValue register
    cntp_ctl_el0: u64,  // Physical Timer Control register
    cntv_ctl_el0: u64,  // Virtual Timer Control register
}

const GENERIC_TIMER_CTRL_IMASK: u64 = 1 << 1;

impl Default for GenericTimerContext {
    fn default() -> Self {
        Self::new()
    }
}

impl GenericTimerContext {
    pub const fn new() -> Self {
        Self {
            cntvoff_el2: 0,
            cntkctl_el1: 0,
            cntp_cval_el0: 0,
            cntv_cval_el0: 0,
            cntp_ctl_el0: GENERIC_TIMER_CTRL_IMASK,
            cntv_ctl_el0: GENERIC_TIMER_CTRL_IMASK,
        }
    }

    #[allow(dead_code)]
    pub fn reset(&mut self) {
        *self = Default::default();
    }

    #[cfg(feature = "vtimer")]
    pub fn set_offset(&mut self, vtimer_offset: u64) {
        self.cntvoff_el2 = vtimer_offset;
    }

    pub fn save(&mut self) {
        // no need to save offset register
        mrs!(self.cntkctl_el1, CNTKCTL_EL1);
        mrs!(self.cntp_cval_el0, CNTP_CVAL_EL0);
        mrs!(self.cntv_cval_el0, CNTV_CVAL_EL0);
        mrs!(self.cntp_ctl_el0, CNTP_CTL_EL0);
        mrs!(self.cntv_ctl_el0, CNTV_CTL_EL0);
        // mask the interrupt
        msr!(CNTP_CTL_EL0, GENERIC_TIMER_CTRL_IMASK);
        msr!(CNTV_CTL_EL0, GENERIC_TIMER_CTRL_IMASK);
    }

    pub fn restore(&self) {
        msr!(CNTVOFF_EL2, self.cntvoff_el2);
        msr!(CNTKCTL_EL1, self.cntkctl_el1);
        msr!(CNTP_CVAL_EL0, self.cntp_cval_el0);
        msr!(CNTV_CVAL_EL0, self.cntv_cval_el0);
        msr!(CNTP_CTL_EL0, self.cntp_ctl_el0);
        msr!(CNTV_CTL_EL0, self.cntv_ctl_el0);
    }
}
