use spin::Mutex;
const CTL_IMASK: usize = 1 << 1;

static TIMER_FREQ: Mutex<usize> = Mutex::new(0);
static TIMER_SLICE: Mutex<usize> = Mutex::new(0);

pub fn timer_arch_set(num: usize) {
    let slice_lock = TIMER_SLICE.lock();
    let val = *slice_lock * num;
    drop(slice_lock);
    unsafe {
        llvm_asm!("msr CNTHP_TVAL_EL2, $0" :: "r"(val) :: "volatile");
        llvm_asm!("isb");
    };
}

pub fn timer_arch_enable_irq() {
    unsafe {
        llvm_asm!("msr CNTHP_CTL_EL2, $0" :: "r"(1) :: "volatile");
        llvm_asm!("isb");
    };
}

pub fn timer_arch_disable_irq() {
    unsafe {
        llvm_asm!("msr CNTHP_CTL_EL2, $0" :: "r"(2) :: "volatile");
        llvm_asm!("isb");
    };
}

pub fn timer_arch_get_counter() -> usize {
    let cnt;
    unsafe {
        llvm_asm!("mrs $0, CNTPCT_EL0" : "=r"(cnt) ::: "volatile");
        llvm_asm!("isb");
    };
    cnt
}

pub fn timer_arch_get_frequency() -> usize {
    let freq;
    unsafe {
        llvm_asm!("mrs $0, CNTFRQ_EL0" : "=r"(freq) ::: "volatile");
        llvm_asm!("isb");
    };
    freq
}

pub fn timer_arch_init() {
    let mut freq_lock = TIMER_FREQ.lock();
    let mut slice_lock = TIMER_SLICE.lock();
    *freq_lock = timer_arch_get_frequency();
    *slice_lock = (*freq_lock) / 100;

    unsafe {
        llvm_asm!("msr CNTHP_CTL_EL2, $0" :: "r"(0x3 & (1 | !CTL_IMASK)) :: "volatile");
        llvm_asm!("msr CNTHP_TVAL_EL2, $0" :: "r"(*slice_lock * 10) :: "volatile");
    }
}
