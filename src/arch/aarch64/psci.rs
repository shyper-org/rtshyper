use super::smc::smc_call;
use crate::arch::{Aarch64ContextFrame, gic_cpu_init, gicc_clear_current_irq, vcpu_arch_init};
use crate::kernel::{cpu_idle, current_cpu, ipi_intra_broadcast_msg, timer_enable, Vm};
use crate::kernel::{
    active_vcpu_id, active_vm_id, CpuState,
};
use crate::kernel::{active_vm, ipi_send_msg, IpiInnerMsg, IpiPowerMessage, IpiType, PowerEvent};

pub const PSCI_VERSION: usize = 0x84000000;
pub const PSCI_MIG_INFO_TYPE: usize = 0x84000006;
pub const PSCI_FEATURES: usize = 0x8400000A;
// pub const PSCI_CPU_SUSPEND_AARCH64: usize = 0xc4000001;
// pub const PSCI_CPU_OFF: usize = 0xc4000002;
pub const PSCI_CPU_ON_AARCH64: usize = 0xc4000003;
pub const PSCI_AFFINITY_INFO_AARCH64: usize = 0xc4000004;
pub const PSCI_SYSTEM_OFF: usize = 0x84000008;
pub const PSCI_SYSTEM_RESET: usize = 0x84000009;

pub const PSCI_E_SUCCESS: usize = 0;
pub const PSCI_E_NOT_SUPPORTED: usize = usize::MAX;

#[cfg(feature = "tx2")]
const TEGRA_SIP_GET_ACTMON_CLK_COUNTERS: usize = 0xC2FFFE02;

pub const PSCI_TOS_NOT_PRESENT_MP: usize = 2;

pub fn power_arch_init() {
    use crate::kernel::ipi_register;
    if !ipi_register(IpiType::IpiTPower, psci_ipi_handler) {
        panic!("power_arch_init: failed to register ipi IpiTPower");
    }
}

pub fn power_arch_vm_shutdown_secondary_cores(vm: Vm) {
    let m = IpiPowerMessage {
        event: PowerEvent::PsciIpiCpuReset,
        entry: 0,
        context: 0,
    };

    if !ipi_intra_broadcast_msg(vm, IpiType::IpiTPower, IpiInnerMsg::Power(m)) {
        println!("power_arch_vm_shutdown_secondary_cores: fail to ipi_intra_broadcast_msg");
    }
}

pub fn power_arch_cpu_on(mpidr: usize, entry: usize, ctx: usize) -> usize {
    println!(
        "power_arch_cpu_on, {:x}, {:x}, {:x}",
        PSCI_CPU_ON_AARCH64, mpidr, entry
    );
    let r = smc_call(PSCI_CPU_ON_AARCH64, mpidr, entry, ctx).0;
    println!("smc return val is {}", r);
    r
}

pub fn power_arch_cpu_shutdown() {
    gic_cpu_init();
    gicc_clear_current_irq(true);
    timer_enable(false);
    cpu_idle();
}

fn psci_guest_sys_reset() {
    vmm_reboot_vm(active_vm().unwrap());
}

#[inline(never)]
pub fn smc_guest_handler(fid: usize, x1: usize, x2: usize, x3: usize) -> bool {
    println!(
        "smc_guest_handler: fid {:x}, x1 {}, x2 {}, x3 {}",
        fid, x1, x2, x3
    );
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
        PSCI_AFFINITY_INFO_AARCH64 => {
            r = 0;
        }
        PSCI_SYSTEM_RESET => {
            psci_guest_sys_reset();
            r = 0;
        }
        #[cfg(feature = "tx2")]
        TEGRA_SIP_GET_ACTMON_CLK_COUNTERS => {
            let result = smc_call(fid, x1, x2, x3);
            r = result.0;
            // println!("x1 0x{:x}, x2 0x{:x}, x3 0x{:x}", x1, x2, x3);
            println!(
                "result.0 0x{:x}, result.1 0x{:x}, result.2 0x{:x}, sub {:x}",
                result.0, result.1, result.2,
                if result.1 > result.2 { result.1 - result.2 } else { result.2 - result.1 }
            );
            let idx = 1;
            let val = result.1;
            current_cpu().set_gpr(idx, val);
            let idx = 2;
            let val = result.2;
            current_cpu().set_gpr(idx, val);
        }
        PSCI_FEATURES => match x1 {
            PSCI_VERSION | PSCI_CPU_ON_AARCH64 | PSCI_FEATURES => {
                r = PSCI_E_SUCCESS;
            }
            _ => {
                r = PSCI_E_NOT_SUPPORTED;
            }
        },
        _ => {
            // unimplemented!();
            return false;
        }
    }

    let idx = 0;
    let val = r;
    current_cpu().set_gpr(idx, val);

    true
}

fn psci_vcpu_on(entry: usize, ctx: usize) {
    let assigned = true;
    current_cpu().assigned = assigned;
    let state = CpuState::CpuRun;
    current_cpu().cpu_state = state;
    let vcpu = current_cpu().active_vcpu.clone().unwrap();
    vcpu.set_gpr(0, ctx);
    vcpu.set_elr(entry);

    vcpu.reset_state();
    match current_cpu().ctx {
        Some(ctx) => {
            use crate::lib::memcpy_safe;
            use core::mem::size_of;
            let size = size_of::<Aarch64ContextFrame>();

            if trace() && (ctx < 0x1000 || vcpu.vcpu_ctx_addr() < 0x1000) {
                panic!("illegal des ctx addr {} vcpu ctx {}", ctx, vcpu.vcpu_ctx_addr());
            }
            memcpy_safe(ctx as *mut u8, vcpu.vcpu_ctx_addr() as *mut u8, size);

            println!("cpu_ctx {:x}", current_cpu().ctx.unwrap());
        }
        None => {
            panic!("psci_vcpu_on: cpu_ctx is NULL");
        }
    }
}

use crate::kernel::IpiMessage;
use crate::lib::trace;
use crate::vmm::vmm_reboot_vm;

pub fn psci_ipi_handler(msg: &IpiMessage) {
    match msg.ipi_message {
        IpiInnerMsg::Power(power_msg) => match power_msg.event {
            PowerEvent::PsciIpiCpuOn => {
                println!(
                    "Core {} (vm {}, vcpu {}) is woke up",
                    current_cpu().id,
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
                current_cpu().active_vcpu.clone().unwrap().shutdown();
            }
            PowerEvent::PsciIpiCpuReset => {
                vcpu_arch_init(active_vm().unwrap(), current_cpu().active_vcpu.clone().unwrap());
            }
        },
        _ => {
            panic!(
                "psci_ipi_handler: cpu{} receive illegal psci ipi type",
                current_cpu().id
            );
        }
    }
}


pub fn psci_guest_cpu_on(mpidr: usize, entry: usize, ctx: usize) -> usize {
    let vcpu_id = mpidr & 0xff;
    let vm = active_vm().unwrap();
    let physical_linear_id = vm.vcpuid_to_pcpuid(vcpu_id);

    if vcpu_id >= vm.cpu_num() || physical_linear_id.is_err() {
        println!("psci_guest_cpu_on: target vcpu {} not exist", vcpu_id);
        return usize::MAX - 1;
    }
    #[cfg(feature = "tx2")]
        {
            let cluster = (mpidr >> 8) & 0xff;
            if vm.id() == 0 && cluster != 1 {
                println!("psci_guest_cpu_on: L4T only support cluster #1");
                return usize::MAX - 1;
            }
        }

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
