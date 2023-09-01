use aarch64_cpu::registers::DAIF;
use tock_registers::interfaces::*;

/// Mask (disable) interrupt from perspective of CPU
#[allow(dead_code)]
pub fn cpu_interrupt_mask() {
    DAIF.write(DAIF::I::Masked)
}

/// Unmask (enable) interrupt from perspective of CPU
#[allow(dead_code)]
pub fn cpu_interrupt_unmask() {
    DAIF.write(DAIF::I::Unmasked)
}

#[cfg(feature = "preempt")]
pub fn cpu_interrupt_disable() -> u64 {
    let level = DAIF.get();
    cpu_interrupt_mask();
    unsafe {
        core::arch::asm!("dsb sy");
    }
    level
}

#[cfg(feature = "preempt")]
pub fn cpu_interrupt_enable(level: u64) {
    unsafe {
        core::arch::asm!("dsb sy");
    }
    let cur = DAIF.get() & !0xc0;
    DAIF.set((level & 0xc0) | cur);
}
