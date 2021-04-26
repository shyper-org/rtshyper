pub fn tlb_invalidate_guest_all() {
    unsafe {
        llvm_asm!("dsb ish");
        llvm_asm!("tlbi vmalls12e1is");
        llvm_asm!("dsb ish");
        llvm_asm!("isb");
    }
}