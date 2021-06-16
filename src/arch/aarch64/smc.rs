#[no_mangle]
pub fn smc_call(x0: usize, x1: usize, x2: usize, x3: usize) -> (usize, usize, usize, usize) {
    let r;
    unsafe {
        llvm_asm!("smc #0"
        : "={x0}" (r)
        : "{x0}" (x0), "{x1}" (x1), "{x2}" (x2), "{x3}" (x3)
        : "memory"
        : "volatile");
    }
    (r, x1, x2, x3)
}
