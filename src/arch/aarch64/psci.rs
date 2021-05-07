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
    let r;
    match fid {
        PSCI_VERSION => {
            r = smc_call(PSCI_VERSION, 0, 0, 0);
        }
        PSCI_MIG_INFO_TYPE => {
            r = PSCI_TOS_NOT_PRESENT_MP;
        }
        PSCI_CPU_ON_AARCH64 => {
            r = psci_guest_cpu_on(x1, x2, x3);
        }
        _ => {
            return false;
        }
    }

    context_set_gpr(0, r);

    true
}

use crate::kernel::{active_vcpu, set_cpu_assign, set_cpu_ctx, set_cpu_state, CpuState};
fn psci_vcpu_on(entry: usize, ctx: usize) {
    set_cpu_assign(true);
    set_cpu_state(CpuState::CpuRun);
    let vcpu = active_vcpu().unwrap();
    vcpu.set_gpr(0, ctx);
    vcpu.set_elr(entry);

    vcpu.reset_state();
    set_cpu_ctx(vcpu.vcpu_ctx_addr() as *mut _);
}

use crate::kernel::IpiMessage;
pub fn psci_ipi_handler(msg: &IpiMessage) {
    match msg.ipi_message {
        IpiInnerMsg::Power(power_msg) => match power_msg.event {
            PowerEvent::PsciIpiCpuOn => {}
            PowerEvent::PsciIpiCpuOff => {
                unimplemented!();
            }
            PowerEvent::PsciIpiCpuReset => {
                unimplemented!();
            }
        },
        _ => {
            unimplemented!();
        }
    }
}

use crate::kernel::{active_vm, ipi_send_msg, IpiInnerMsg, IpiPowerMessage, IpiType, PowerEvent};
pub fn psci_guest_cpu_on(mpidr: usize, entry: usize, ctx: usize) -> usize {
    let vcpu_id = mpidr & 0xff;
    let vm = active_vm().unwrap();
    let physical_linear_id = vm.vcpuid_to_pcpuid(vcpu_id);

    if vcpu_id >= vm.cpu_num() || physical_linear_id.is_err() {
        println!("psci_guest_cpu_on: target vcpu {} not exist", vcpu_id);
        return (usize::MAX - 1);
    }
    // TODO: TX2

    let m = IpiPowerMessage {
        event: PowerEvent::PsciIpiCpuOn,
        entry,
        context: ctx,
    };

    if !ipi_send_msg(
        physical_linear_id.unwrap(),
        IpiType::IpiTPower,
        IpiInnerMsg::Power(m),
    ) {
        return (usize::MAX - 1);
    }

    0
}
