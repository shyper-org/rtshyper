/// Mask (disable) interrupt from perspective of CPU
#[inline(always)]
pub fn cpu_interrupt_mask() {
    unsafe {
        llvm_asm!("msr daifset, $0" :: "I"(2) :: "volatile");
    }
}

/// Unmask (enable) interrupt from perspective of CPU
#[inline(always)]
pub fn cpu_interrupt_unmask() {
    unsafe {
        llvm_asm!("msr daifclr, $0" :: "I"(2) :: "volatile");
    }
}
