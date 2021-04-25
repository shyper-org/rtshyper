// TODO: fix these two fn
// #[inline(always)]
// pub fn mrs(reg: &str) -> usize {
//     let val = 0;
//     unsafe {
//         // llvm_asm!("mrs %0, " #reg : "=r"(val));
//     }
//     val
// }

// #[inline(always)]
// pub fn msr(val: usize, reg: &str) {
//     unsafe {
//         // llvm_asm!("msr " #reg ", %0\n\r" ::"r"(val));
//     }
// }
