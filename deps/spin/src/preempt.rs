#[inline(always)]
pub fn preempt_disable() {
    // #[cfg(target_arch = "aarch64")]
    // unsafe {
    //     core::arch::asm!("msr daifset, #2");
    // }
}

#[inline(always)]
pub fn preempt_enable() {
    // #[cfg(target_arch = "aarch64")]
    // unsafe {
    //     core::arch::asm!("msr daifclr, #2");
    // }
}
