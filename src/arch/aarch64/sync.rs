use crate::arch::exception_next_instruction_step;
use crate::arch::smc_guest_handler;
use crate::arch::{
    exception_data_abort_access_is_sign_ext, exception_data_abort_access_is_write,
    exception_data_abort_access_reg, exception_data_abort_access_reg_width,
    exception_data_abort_access_width, exception_data_abort_handleable,
    exception_data_abort_is_translate_fault,
};
use crate::arch::{exception_esr, exception_fault_ipa};
use crate::device::{emu_handler, EmuContext};
use crate::kernel::cpu_id;
use crate::kernel::hvc_guest_handler;
use crate::kernel::{context_get_gpr, context_set_gpr};
use crate::kernel::{get_cpu_ctx_elr, set_cpu_ctx_elr};

pub fn data_abort_handler() {
    if !exception_data_abort_handleable() {
        panic!(
            "Core {} data abort not handleable 0x{:x}, esr 0x{:x}",
            cpu_id(),
            exception_fault_ipa(),
            exception_esr()
        );
    }

    if !exception_data_abort_is_translate_fault() {
        panic!(
            "Core {} data abort is translate fault 0x{:x}",
            cpu_id(),
            exception_fault_ipa(),
        );
    }

    let emu_ctx = EmuContext {
        address: exception_fault_ipa(),
        width: exception_data_abort_access_width(),
        write: exception_data_abort_access_is_write(),
        sign_ext: exception_data_abort_access_is_sign_ext(),
        reg: exception_data_abort_access_reg(),
        reg_width: exception_data_abort_access_reg_width(),
    };

    let elr = get_cpu_ctx_elr();
    // println!("emu_handler");
    if !emu_handler(&emu_ctx) {
        println!(
            "data_abort_handler: Failed to handler emul device request, ipa 0x{:x} elr 0x{:x}",
            emu_ctx.address, elr
        );
    }

    set_cpu_ctx_elr(elr + exception_next_instruction_step());
}

pub fn smc_handler() {
    let fid = context_get_gpr(0);
    let x1 = context_get_gpr(1);
    let x2 = context_get_gpr(2);
    let x3 = context_get_gpr(3);

    if !smc_guest_handler(fid, x1, x2, x3) {
        println!("smc_handler: unknown fid 0x{:x}", fid);
        context_set_gpr(0, 0);
    }

    let elr = get_cpu_ctx_elr();
    set_cpu_ctx_elr(elr + exception_next_instruction_step());
}

pub fn hvc_handler() {
    let x0 = context_get_gpr(0);
    let x1 = context_get_gpr(1);
    let x2 = context_get_gpr(2);
    let x3 = context_get_gpr(3);
    let x4 = context_get_gpr(4);
    let x5 = context_get_gpr(5);
    let x6 = context_get_gpr(6);
    let mode = context_get_gpr(7);

    let hvc_type = (mode >> 8) & 0xff;
    let event = mode & 0xff;

    context_set_gpr(0, 0);
    if !hvc_guest_handler(hvc_type, event, x0, x1, x2, x3, x4, x5, x6) {
        println!(
            "Failed to handle hvc request fid 0x{:x} event 0x{:x}",
            hvc_type, event
        );
        context_set_gpr(0, usize::MAX);
    }
}
