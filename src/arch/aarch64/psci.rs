use super::smc::smc_call;
use crate::kernel::context_set_gpr;

pub const PSCI_VERSION: usize = 0x84000000;
pub const PSCI_MIG_INFO_TYPE: usize = 0x84000006;
pub const PSCI_CPU_SUSPEND_AARCH64: usize = 0xc4000001;
pub const PSCI_CPU_OFF: usize = 0xc4000002;
pub const PSCI_CPU_ON_AARCH64: usize = 0xc4000003;

pub const PSCI_TOS_NOT_PRESENT_MP: usize = 2;

pub fn power_arch_cpu_on(mpidr: usize, entry: usize, ctx: usize) -> usize {
    smc_call(PSCI_CPU_ON_AARCH64, mpidr, entry, ctx)
}

pub fn smc_guest_handler(fid: usize, x1: usize, x2: usize, x3: usize) -> bool {
    println!(
        "smc_guest_handler: fid {:x}, x1 {}, x2 {}, x3 {}",
        fid, x1, x2, x3
    );
    let mut r: usize = usize::MAX;
    match fid {
        PSCI_VERSION => {
            r = smc_call(PSCI_VERSION, 0, 0, 0);
        }
        PSCI_MIG_INFO_TYPE => {
            r = PSCI_TOS_NOT_PRESENT_MP;
        }
        _ => {
            return false;
        }
    }

    context_set_gpr(0, r);

    true
}
