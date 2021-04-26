use crate::arch::ContextFrame;
use crate::arch::{gicc_clear_current_irq, gicc_get_current_irq};
use crate::kernel::interrupt_handler;
use crate::kernel::{active_vm_id, cpu_id};
use cortex_a::{barrier, regs::*};

global_asm!(include_str!("exception.S"));

#[inline(always)]
fn exception_esr() -> usize {
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
fn exception_fault_ipa() -> usize {
    (exception_far() & 0xfff) | (exception_hpfar() << 8)
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
    // TODO: cpu.ctx = ctx
    println!("exception class {}", exception_class());
    match exception_class() {
        0x24 => {
            unimplemented!();
        }
        0x17 => {
            // TODO
            unimplemented!();
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
    // TODO: cpu.ctx = ctx

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
