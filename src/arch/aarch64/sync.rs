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
use crate::kernel::{current_cpu, hvc_guest_handler};

pub fn data_abort_handler() {
    if !exception_data_abort_handleable() {
        panic!(
            "Core {} data abort not handleable 0x{:x}, esr 0x{:x}",
            current_cpu().id,
            exception_fault_ipa(),
            exception_esr()
        );
    }

    if !exception_data_abort_is_translate_fault() {
        panic!(
            "Core {} data abort is translate fault 0x{:x}",
            current_cpu().id,
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

    let elr = current_cpu().get_elr();
    // println!("emu_handler");
    if !emu_handler(&emu_ctx) {
        println!(
            "data_abort_handler: Failed to handler emul device request, ipa 0x{:x} elr 0x{:x}",
            emu_ctx.address, elr
        );
    }

    let val = elr + exception_next_instruction_step();
    current_cpu().set_elr(val);
}

pub fn smc_handler() {
    let idx = 0;
    let fid = current_cpu().get_gpr(idx);
    let idx = 1;
    let x1 = current_cpu().get_gpr(idx);
    let idx = 2;
    let x2 = current_cpu().get_gpr(idx);
    let idx = 3;
    let x3 = current_cpu().get_gpr(idx);

    if !smc_guest_handler(fid, x1, x2, x3) {
        println!("smc_handler: unknown fid 0x{:x}", fid);
        let idx = 0;
        let val = 0;
        current_cpu().set_gpr(idx, val);
    }

    let elr = current_cpu().get_elr();
    let val = elr + exception_next_instruction_step();
    current_cpu().set_elr(val);
}

pub fn hvc_handler() {
    let idx = 0;
    let x0 = current_cpu().get_gpr(idx);
    let idx = 1;
    let x1 = current_cpu().get_gpr(idx);
    let idx = 2;
    let x2 = current_cpu().get_gpr(idx);
    let idx = 3;
    let x3 = current_cpu().get_gpr(idx);
    let idx = 4;
    let x4 = current_cpu().get_gpr(idx);
    let idx = 5;
    let x5 = current_cpu().get_gpr(idx);
    let idx = 6;
    let x6 = current_cpu().get_gpr(idx);
    let idx = 7;
    let mode = current_cpu().get_gpr(idx);

    let hvc_type = (mode >> 8) & 0xff;
    let event = mode & 0xff;

    let idx = 0;
    let val = 0;
    current_cpu().set_gpr(idx, val);
    if !hvc_guest_handler(hvc_type, event, x0, x1, x2, x3, x4, x5, x6) {
        println!(
            "Failed to handle hvc request fid 0x{:x} event 0x{:x}",
            hvc_type, event
        );
        let idx = 0;
        let val = usize::MAX;
        current_cpu().set_gpr(idx, val);
    }
}
