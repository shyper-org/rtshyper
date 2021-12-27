use core::arch::asm;

pub fn tlb_invalidate_guest_all() {
    unsafe {
        asm!("dsb ish", "tlbi vmalls12e1is", "dsb ish", "isb");
    }
}