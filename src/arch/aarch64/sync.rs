use crate::arch::smc_guest_handler;
use crate::device::{emu_handler, emu_reg_handler, EmuContext};
use crate::kernel::{active_vm, current_cpu, hvc_guest_handler};

use super::exception::{
    exception_data_abort_access_is_sign_ext, exception_data_abort_access_is_write, exception_data_abort_access_reg,
    exception_data_abort_access_reg_width, exception_data_abort_access_width, exception_data_abort_handleable,
    exception_data_abort_is_permission_fault, exception_data_abort_is_translate_fault, exception_iss, exception_esr,
    exception_fault_addr, exception_next_instruction_step,
};

pub const HVC_RETURN_REG: usize = 0;

pub fn data_abort_handler() {
    // let time0 = time_current_us();
    let emu_ctx = EmuContext {
        address: exception_fault_addr(),
        width: exception_data_abort_access_width(),
        write: exception_data_abort_access_is_write(),
        sign_ext: exception_data_abort_access_is_sign_ext(),
        reg: exception_data_abort_access_reg(),
        reg_width: exception_data_abort_access_reg_width(),
    };
    let elr = current_cpu().get_elr();

    if !exception_data_abort_handleable() {
        panic!(
            "Core {} data abort not handleable {:#x}, esr {:#x}",
            current_cpu().id,
            exception_fault_addr(),
            exception_esr()
        );
    }

    if !exception_data_abort_is_translate_fault() {
        if exception_data_abort_is_permission_fault() {
            // println!(
            //     "write {}, width {}, reg width {}, addr {:x}, iss {:x}, reg idx {}, reg val {:#x}, esr {:#x}",
            //     exception_data_abort_access_is_write(),
            //     emu_ctx.width,
            //     emu_ctx.reg_width,
            //     emu_ctx.address,
            //     exception_iss(),
            //     emu_ctx.reg,
            //     current_cpu().get_gpr(emu_ctx.reg),
            //     exception_esr()
            // );
            // no need to rewrite elr

            // let time1 = time_current_us();
            // println!("migrate_data_abort_handler: {}us", time1 - time0);
            return;
        } else {
            panic!(
                "Core {} data abort is not translate fault {:#x}",
                current_cpu().id,
                exception_fault_addr(),
            );
        }
    }
    if !emu_handler(&emu_ctx) {
        active_vm().unwrap().show_pagetable(emu_ctx.address);
        error!(
            "write {}, width {}, reg width {}, addr {:x}, iss {:x}, reg idx {}, reg val {:#x}, esr {:#x}",
            exception_data_abort_access_is_write(),
            emu_ctx.width,
            emu_ctx.reg_width,
            emu_ctx.address,
            exception_iss(),
            emu_ctx.reg,
            current_cpu().get_gpr(emu_ctx.reg),
            exception_esr()
        );
        panic!(
            "data_abort_handler: Failed to handler emul device request, ipa {:#x} elr {:#x}",
            emu_ctx.address, elr
        );
    }
    let val = elr + exception_next_instruction_step();
    current_cpu().set_elr(val);
}

pub fn smc_handler() {
    let fid = current_cpu().get_gpr(0);
    let x1 = current_cpu().get_gpr(1);
    let x2 = current_cpu().get_gpr(2);
    let x3 = current_cpu().get_gpr(3);

    if !smc_guest_handler(fid, x1, x2, x3) {
        warn!("smc_handler: unknown fid {:#x}", fid);
        current_cpu().set_gpr(0, 0);
    }

    let elr = current_cpu().get_elr();
    let val = elr + exception_next_instruction_step();
    current_cpu().set_elr(val);
}

pub fn hvc_handler() {
    // let time_start = timer_arch_get_counter();
    let x0 = current_cpu().get_gpr(0);
    let x1 = current_cpu().get_gpr(1);
    let x2 = current_cpu().get_gpr(2);
    let x3 = current_cpu().get_gpr(3);
    let x4 = current_cpu().get_gpr(4);
    let x5 = current_cpu().get_gpr(5);
    let x6 = current_cpu().get_gpr(6);
    let mode = current_cpu().get_gpr(7);

    let hvc_type = (mode >> 8) & 0xff;
    let event = mode & 0xff;

    match hvc_guest_handler(hvc_type, event, x0, x1, x2, x3, x4, x5, x6) {
        Ok(val) => {
            current_cpu().set_gpr(HVC_RETURN_REG, val);
        }
        Err(_) => {
            warn!("Failed to handle hvc request fid {:#x} event {:#x}", hvc_type, event);
            current_cpu().set_gpr(HVC_RETURN_REG, usize::MAX);
        }
    }
    // let time_end = timer_arch_get_counter();
    // println!(
    //     "hvc fid {:#x} event {:#x} counter {}, freq {:x}",
    //     hvc_type,
    //     event,
    //     time_end - time_start,
    //     timer_arch_get_frequency()
    // );
}

#[inline(always)]
fn exception_sysreg_addr(iss: u32) -> u32 {
    // (Op0[21..20] + Op2[19..17] + Op1[16..14] + CRn[13..10]) + CRm[4..1]
    const ESR_ISS_SYSREG_ADDR: u32 = (0xfff << 10) | (0xf << 1);
    iss & ESR_ISS_SYSREG_ADDR
}

#[inline(always)]
fn exception_sysreg_direction_write(iss: u32) -> bool {
    const ESR_ISS_SYSREG_DIRECTION: u32 = 0b1;
    (iss & ESR_ISS_SYSREG_DIRECTION) == 0
}

#[inline(always)]
fn exception_sysreg_gpr(iss: u32) -> u32 {
    const ESR_ISS_SYSREG_REG_OFF: u32 = 5;
    const ESR_ISS_SYSREG_REG_LEN: u32 = 5;
    const ESR_ISS_SYSREG_REG_MASK: u32 = (1 << ESR_ISS_SYSREG_REG_LEN) - 1;
    (iss >> ESR_ISS_SYSREG_REG_OFF) & ESR_ISS_SYSREG_REG_MASK
}

pub fn sysreg_handler(iss: u32) {
    let reg_addr = exception_sysreg_addr(iss);

    let emu_ctx = EmuContext {
        address: reg_addr as usize,
        width: 8,
        write: exception_sysreg_direction_write(iss),
        sign_ext: false,
        reg: exception_sysreg_gpr(iss) as usize,
        reg_width: 8,
    };

    let elr = current_cpu().get_elr();
    if !emu_reg_handler(&emu_ctx) {
        panic!(
            "sysreg_handler: Failed to handler emu reg request, ({:#x} at {:#x})",
            emu_ctx.address, elr
        );
    }

    let val = elr + exception_next_instruction_step();
    current_cpu().set_elr(val);
}
