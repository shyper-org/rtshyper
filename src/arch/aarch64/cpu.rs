use cortex_a::registers::DAIF;
use tock_registers::interfaces::*;

/// Mask (disable) interrupt from perspective of CPU
pub fn cpu_interrupt_mask() {
    DAIF.write(DAIF::I::Masked)
}

/// Unmask (enable) interrupt from perspective of CPU
pub fn cpu_interrupt_unmask() {
    DAIF.write(DAIF::I::Unmasked)
}
