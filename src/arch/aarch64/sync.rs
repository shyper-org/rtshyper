use crate::arch::exception_next_instruction_step;
use crate::arch::smc_guest_handler;
use crate::kernel::{context_get_gpr, context_set_gpr};
use crate::kernel::{get_cpu_ctx_elr, set_cpu_ctx_elr};

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
