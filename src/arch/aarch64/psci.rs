use super::smc::smc_call;
use crate::arch::Aarch64ContextFrame;
use crate::kernel::context_set_gpr;
use crate::kernel::{
    active_vcpu, active_vcpu_id, active_vm_id, cpu_ctx, cpu_id, set_cpu_assign, set_cpu_ctx,
    set_cpu_state, CpuState,
};

pub const PSCI_VERSION: usize = 0x84000000;
pub const PSCI_MIG_INFO_TYPE: usize = 0x84000006;
pub const PSCI_CPU_SUSPEND_AARCH64: usize = 0xc4000001;
pub const PSCI_CPU_OFF: usize = 0xc4000002;
pub const PSCI_CPU_ON_AARCH64: usize = 0xc4000003;
#[cfg(feature = "tx2")]
const TEGRA_SIP_GET_ACTMON_CLK_COUNTERS: usize = 0xC2FFFE02;

pub const PSCI_TOS_NOT_PRESENT_MP: usize = 2;

pub fn power_arch_cpu_on(mpidr: usize, entry: usize, ctx: usize) -> usize {
    println!(
        "power_arch_cpu_on, {:x}, {:x}, {:x}",
        PSCI_CPU_ON_AARCH64, mpidr, entry
    );
    smc_call(PSCI_CPU_ON_AARCH64, mpidr, entry, ctx).0
}

pub fn smc_guest_handler(fid: usize, x1: usize, x2: usize, x3: usize) -> bool {
    // println!(
    //     "smc_guest_handler: fid {:x}, x1 {}, x2 {}, x3 {}",
    //     fid, x1, x2, x3
    // );
    let r;
    match fid {
        PSCI_VERSION => {
            r = smc_call(PSCI_VERSION, 0, 0, 0).0;
        }
        PSCI_MIG_INFO_TYPE => {
            r = PSCI_TOS_NOT_PRESENT_MP;
        }
        PSCI_CPU_ON_AARCH64 => {
            r = psci_guest_cpu_on(x1, x2, x3);
        }
        #[cfg(feature = "tx2")]
        TEGRA_SIP_GET_ACTMON_CLK_COUNTERS => {
            let result = smc_call(fid, x1, x2, x3);
            r = result.0;
            context_set_gpr(1, result.1);
            context_set_gpr(2, result.2);
        }
        _ => {
            unimplemented!();
            return false;
        }
    }

    context_set_gpr(0, r);

    true
}

fn psci_vcpu_on(entry: usize, ctx: usize) {
    set_cpu_assign(true);
    set_cpu_state(CpuState::CpuRun);
    let vcpu = active_vcpu().unwrap();
    vcpu.set_gpr(0, ctx);
    vcpu.set_elr(entry);

    vcpu.reset_state();
    match cpu_ctx() {
        Some(ctx) => {
            use core::mem::size_of;
            use rlibc::memcpy;
            let size = size_of::<Aarch64ContextFrame>();
            unsafe {
                memcpy(ctx as *mut u8, vcpu.vcpu_ctx_addr() as *mut u8, size);
            }
            println!("cpu_ctx {:x}", cpu_ctx().unwrap());
        }
        None => {
            panic!("psci_vcpu_on: cpu_ctx is NULL");
        }
    }
}

use crate::kernel::IpiMessage;
pub fn psci_ipi_handler(msg: &IpiMessage) {
    match msg.ipi_message {
        IpiInnerMsg::Power(power_msg) => match power_msg.event {
            PowerEvent::PsciIpiCpuOn => {
                println!(
                    "Core {} (vm {}, vcpu {}) is woke up",
                    cpu_id(),
                    active_vm_id(),
                    active_vcpu_id()
                );
                println!(
                    "entry {:x}, context {:x}",
                    power_msg.entry, power_msg.context
                );
                psci_vcpu_on(power_msg.entry, power_msg.context);
            }
            PowerEvent::PsciIpiCpuOff => {
                unimplemented!();
            }
            PowerEvent::PsciIpiCpuReset => {
                unimplemented!();
            }
        },
        _ => {
            panic!(
                "psci_ipi_handler: cpu{} receive illegal psci ipi type",
                cpu_id()
            );
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
        return usize::MAX - 1;
    }
    // #[cfg(feature = "tx2")]
    // {
    //     let cluster = (mpidr >> 8) & 0xff;
    //     if vm.vm_id() == 0 && cluster != 1 {
    //         println!("psci_guest_cpu_on: L4T only support cluster #1");
    //         return usize::MAX - 1;
    //     }
    // }

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
        println!("psci_guest_cpu_on: fail to send msg");
        return usize::MAX - 1;
    }
    // println!("success send msg to cpu {}", physical_linear_id.unwrap());

    0
}
