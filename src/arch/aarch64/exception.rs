use crate::arch::ContextFrame;
use cortex_a::{barrier, regs::*};

global_asm!(include_str!("exception.S"));

#[no_mangle]
unsafe extern "C" fn current_el_sp0_synchronous() {
    panic!("current_el_sp0_synchronous");
}

#[no_mangle]
unsafe extern "C" fn current_el_sp0_irq(ctx: *mut ContextFrame) {
    lower_aarch64_irq(ctx);
}

#[no_mangle]
unsafe extern "C" fn current_el_sp0_serror() {
    panic!("current_el0_serror");
}

#[no_mangle]
#[inline(never)]
unsafe extern "C" fn current_el_spx_synchronous() {
    panic!(
        "current_elx_synchronous {:016x}",
        cortex_a::regs::ELR_EL1.get()
    );
}

#[no_mangle]
unsafe extern "C" fn current_el_spx_irq() {
    panic!("current_elx_irq");
}

#[no_mangle]
unsafe extern "C" fn current_el_spx_serror() {
    panic!("current_elx_serror");
}

#[no_mangle]
unsafe extern "C" fn lower_aarch64_synchronous(ctx: *mut ContextFrame) {
    panic!("TODO: lower aarch64 synchronous");
}

#[no_mangle]
unsafe extern "C" fn lower_aarch64_irq(ctx: *mut ContextFrame) {
    panic!("TODO: lower aarch64 irq");
}

#[no_mangle]
unsafe extern "C" fn lower_aarch64_serror(ctx: *mut ContextFrame) {
    panic!("TODO: lower aarch64 serror");
}

// pub fn exception_init() {
//     extern "C" {
//         fn vectors();
//     }
//     unsafe {
//         let addr: u64 = vectors as usize as u64;
//         VBAR_EL1.set(addr);
//         // barrier::isb(barrier::SY);
//     }
// }
