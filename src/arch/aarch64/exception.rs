use crate::arch::ContextFrame;
use crate::arch::{data_abort_handler, smc_handler};
use crate::arch::{gicc_clear_current_irq, gicc_get_current_irq};
use crate::kernel::interrupt_handler;
use crate::kernel::{active_vm_id, cpu_id, set_cpu_ctx};
use cortex_a::regs::*;

global_asm!(include_str!("exception.S"));

#[inline(always)]
pub fn exception_esr() -> usize {
    let mut esr = 0;
    unsafe {
        llvm_asm!("mrs $0, esr_el2" : "=r"(esr) ::: "volatile");
    }
    esr
}

#[inline(always)]
fn exception_class() -> usize {
    (exception_esr() >> 26) & 0b111111
}

#[inline(always)]
fn exception_far() -> usize {
    let mut far = 0;
    unsafe {
        llvm_asm!("mrs $0, far_el2" : "=r"(far) ::: "volatile");
    }
    far
}

#[inline(always)]
fn exception_hpfar() -> usize {
    let mut hpfar = 0;
    unsafe {
        llvm_asm!("mrs $0, hpfar_el2" : "=r"(hpfar) ::: "volatile");
    }
    hpfar
}

#[inline(always)]
pub fn exception_fault_ipa() -> usize {
    (exception_far() & 0xfff) | (exception_hpfar() << 8)
}

/// \return 1 means 32-bit instruction, 0 means 16-bit instruction
#[inline(always)]
fn exception_instruction_length() -> usize {
    (exception_esr() >> 25) & 1
}

#[inline(always)]
pub fn exception_next_instruction_step() -> usize {
    2 + 2 * exception_instruction_length()
}

#[inline(always)]
pub fn exception_iss() -> usize {
    exception_esr() & ((1 << 25) - 1)
}

#[inline(always)]
pub fn exception_data_abort_handleable() -> bool {
    ((exception_iss() & (1 << 10)) | (exception_iss() & (1 << 24))) != 0
}

#[inline(always)]
pub fn exception_data_abort_is_translate_fault() -> bool {
    (exception_iss() & 0b111111 & (0xf << 2)) == 4
}

#[inline(always)]
pub fn exception_data_abort_access_width() -> usize {
    1 << ((exception_iss() >> 22) & 0b11)
}

#[inline(always)]
pub fn exception_data_abort_access_is_write() -> bool {
    (exception_iss() & (1 << 6)) != 0
}

#[inline(always)]
pub fn exception_data_abort_access_reg() -> usize {
    (exception_iss() >> 16) & 0b11111
}

#[inline(always)]
pub fn exception_data_abort_access_reg_width() -> usize {
    4 + 4 * ((exception_iss() >> 15) & 1)
}

#[inline(always)]
pub fn exception_data_abort_access_is_sign_ext() -> bool {
    ((exception_iss() >> 21) & 1) != 0
}

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
        cortex_a::regs::ELR_EL2.get()
    );
}

#[no_mangle]
unsafe extern "C" fn current_el_spx_irq(ctx: *mut ContextFrame) {
    lower_aarch64_irq(ctx);
}

#[no_mangle]
unsafe extern "C" fn current_el_spx_serror() {
    panic!("current_elx_serror");
}

#[no_mangle]
unsafe extern "C" fn lower_aarch64_synchronous(ctx: *mut ContextFrame) {
    set_cpu_ctx(ctx);

    // println!("exception class {}", exception_class());
    match exception_class() {
        0x24 => {
            data_abort_handler();
        }
        0x17 => {
            smc_handler();
        }
        0x16 => {
            unimplemented!();
        }
        _ => {
            panic!(
                "core {} vm {}: handler not presents for EC_{} @ipa 0x{:x}, @pc 0x{:x}",
                cpu_id(),
                active_vm_id(),
                exception_class(),
                exception_fault_ipa(),
                (*ctx).elr()
            );
        }
    }
}

#[no_mangle]
unsafe extern "C" fn lower_aarch64_irq(ctx: *mut ContextFrame) {
    let (id, src) = gicc_get_current_irq();
    println!("id {}, src {}", id, src);
    set_cpu_ctx(ctx);

    if id >= 1022 {
        return;
    }
    let handled_by_hypervisor = interrupt_handler(id, src);
    gicc_clear_current_irq(handled_by_hypervisor);
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
