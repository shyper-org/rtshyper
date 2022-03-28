use core::arch::global_asm;

global_asm!(include_str!("smc.S"));
extern "C" {
    pub fn smc(x0: usize, x1: usize, x2: usize, x3: usize) -> (usize, usize, usize, usize);
}

#[inline(never)]
pub fn smc_call(x0: usize, x1: usize, x2: usize, x3: usize) -> (usize, usize, usize, usize) {
    unsafe { smc(x0, x1, x2, x3) }
    // let r0;
    // let r1;
    // let r2;
    // let r3;
    // unsafe {
    //     llvm_asm!("smc #0"
    //     : "={x0}" (r)
    //     : "{x0}" (x0), "{x1}" (x1), "{x2}" (x2), "{x3}" (x3)
    //     : "memory"
    //     : "volatile");
    // }
    // return (r, x1, x2, x3);
    // unsafe {
    //     asm!(
    //     "smc #0",
    //     inlateout("x0") x0 => r0,
    //     inlateout("x1") x1 => r1,
    //     inlateout("x2") x2 => r2,
    //     inlateout("x3") x3 => r3,
    //     options(nostack)
    //     );
    // }
    // (r0, r1, r2, r3)
}
