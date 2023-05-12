#[inline(always)]
pub fn preempt_disable() -> bool {
    #[cfg(target_arch = "aarch64")]
    {
        let flags: usize;
        unsafe {
            core::arch::asm!("mrs {}, daif", out(reg) flags, options(nostack, nomem));
        }
        let was_enabled = flags & (0b1 << 7) == 0; // only I
        unsafe {
            core::arch::asm!("msr daifset, #7", options(nostack, nomem));
        }
        was_enabled
    }
    #[cfg(not(target_arch = "aarch64"))]
    compile_error!("unsupported target_arch");
}

fn enable() {
    #[cfg(target_arch = "aarch64")]
    unsafe {
        core::arch::asm!("msr daifclr, #7", options(nostack, nomem));
    }
    #[cfg(not(target_arch = "aarch64"))]
    compile_error!("unsupported target_arch");
}

#[inline(always)]
pub fn preempt_enable(was_enabled: bool) {
    if was_enabled {
        enable();
    }
}
