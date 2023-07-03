use crate::arch::{gic_cpu_init, interrupt_arch_deactive_irq, vcpu_arch_init};
use crate::board::PlatOperation;
use crate::kernel::{cpu_idle, current_cpu, ipi_intra_broadcast_msg, Vcpu, VcpuState, Vm};
use crate::kernel::{active_vm, ipi_send_msg, IpiInnerMsg, IpiPowerMessage, IpiType, PowerEvent};
use crate::kernel::CpuState;
use crate::kernel::IpiMessage;
use crate::vmm::vmm_reboot;

use super::smc::smc_call;

const PSCI_VERSION: usize = 0x84000000;
const PSCI_MIG_INFO_TYPE: usize = 0x84000006;
const PSCI_FEATURES: usize = 0x8400000A;
// const PSCI_CPU_SUSPEND_AARCH64: usize = 0xc4000001;
// const PSCI_CPU_OFF: usize = 0xc4000002;
const PSCI_CPU_ON_AARCH64: usize = 0xc4000003;
const PSCI_AFFINITY_INFO_AARCH64: usize = 0xc4000004;
const PSCI_SYSTEM_OFF: usize = 0x84000008;
const PSCI_SYSTEM_RESET: usize = 0x84000009;

const PSCI_E_SUCCESS: usize = 0;
const PSCI_E_NOT_SUPPORTED: usize = usize::MAX;

#[cfg(feature = "tx2")]
const TEGRA_SIP_GET_ACTMON_CLK_COUNTERS: usize = 0xC2FFFE02;

const PSCI_TOS_NOT_PRESENT_MP: usize = 2;

pub fn power_arch_init() {
    use crate::kernel::ipi_register;
    ipi_register(IpiType::IpiTPower, psci_ipi_handler);
}

pub fn power_arch_vm_shutdown_secondary_cores(vm: &Vm) {
    let m = IpiPowerMessage {
        src: vm.id(),
        event: PowerEvent::PsciIpiCpuReset,
        entry: 0,
        context: 0,
    };

    if !ipi_intra_broadcast_msg(vm, IpiType::IpiTPower, IpiInnerMsg::Power(m)) {
        warn!("power_arch_vm_shutdown_secondary_cores: fail to ipi_intra_broadcast_msg");
    }
}

pub fn power_arch_cpu_on(mpidr: usize, entry: usize, ctx: usize) -> usize {
    // println!("power_arch_cpu_on, {:x}, {:x}, {:x}", PSCI_CPU_ON_AARCH64, mpidr, entry);

    // println!("smc return val is {}", r);
    smc_call(PSCI_CPU_ON_AARCH64, mpidr, entry, ctx).0
}

#[allow(dead_code)]
pub fn power_arch_cpu_shutdown() {
    gic_cpu_init();
    interrupt_arch_deactive_irq(true);
    cpu_idle();
}

pub fn power_arch_sys_reset() {
    smc_call(PSCI_SYSTEM_RESET, 0, 0, 0);
}

pub fn power_arch_sys_shutdown() {
    smc_call(PSCI_SYSTEM_OFF, 0, 0, 0);
}

fn psci_guest_sys_reset() {
    vmm_reboot();
}

fn psci_guest_sys_off() {
    let vm_id = active_vm().unwrap().id();
    if vm_id == 0 {
        crate::board::Platform::sys_shutdown();
    } else {
        info!("VM[{}] system off, please remove it on MVM", vm_id);
        // vmm_remove_vm(vm_id);
    }
}

#[inline(never)]
pub fn smc_guest_handler(fid: usize, x1: usize, x2: usize, x3: usize) -> bool {
    debug!(
        "smc_guest_handler: fid {:#x}, x1 {:#x}, x2 {:#x}, x3 {:#x}",
        fid, x1, x2, x3
    );
    let r = match fid {
        PSCI_VERSION => smc_call(PSCI_VERSION, 0, 0, 0).0,
        PSCI_MIG_INFO_TYPE => PSCI_TOS_NOT_PRESENT_MP,
        PSCI_CPU_ON_AARCH64 => psci_guest_cpu_on(x1, x2, x3),
        PSCI_AFFINITY_INFO_AARCH64 => 0,
        PSCI_SYSTEM_RESET => {
            psci_guest_sys_reset();
            0
        }
        PSCI_SYSTEM_OFF => {
            psci_guest_sys_off();
            0
        }
        #[cfg(feature = "tx2")]
        TEGRA_SIP_GET_ACTMON_CLK_COUNTERS => {
            let result = smc_call(fid, x1, x2, x3);
            // println!("x1 {:#x}, x2 {:#x}, x3 {:#x}", x1, x2, x3);
            // println!(
            //     "result.0 {:#x}, result.1 {:#x}, result.2 {:#x}",
            //     result.0, result.1, result.2
            // );
            current_cpu().set_gpr(1, result.1);
            current_cpu().set_gpr(2, result.2);
            result.0
        }
        PSCI_FEATURES => match x1 {
            PSCI_VERSION | PSCI_CPU_ON_AARCH64 | PSCI_FEATURES => PSCI_E_SUCCESS,
            _ => PSCI_E_NOT_SUPPORTED,
        },
        _ => {
            // unimplemented!();
            return false;
        }
    };

    current_cpu().set_gpr(0, r);

    true
}

fn psci_vcpu_on(vcpu: &Vcpu, entry: usize, ctx: usize) {
    // println!("psci vcpu on， entry {:x}, ctx {:x}", entry, ctx);
    if vcpu.phys_id() != current_cpu().id {
        panic!(
            "cannot psci on vcpu on cpu {} by cpu {}",
            vcpu.phys_id(),
            current_cpu().id
        );
    }
    current_cpu().cpu_state = CpuState::Run;
    vcpu.reset_context();
    vcpu.set_gpr(0, ctx);
    vcpu.set_elr(entry);
    // Just wake up the vcpu
    current_cpu().vcpu_array.wakeup_vcpu(vcpu);
}

// Todo: need to support more vcpu in one Core
pub fn psci_ipi_handler(msg: IpiMessage) {
    match msg.ipi_message {
        IpiInnerMsg::Power(power_msg) => {
            let trgt_vcpu = match current_cpu().vcpu_array.pop_vcpu_through_vmid(power_msg.src) {
                None => {
                    warn!(
                        "Core {} failed to find target vcpu, source vmid {}",
                        current_cpu().id,
                        power_msg.src
                    );
                    return;
                }
                Some(vcpu) => vcpu,
            };
            match power_msg.event {
                PowerEvent::PsciIpiCpuOn => {
                    if trgt_vcpu.state() != VcpuState::Inv {
                        warn!(
                            "psci_ipi_handler: target VCPU {} in VM {} is already running",
                            trgt_vcpu.id(),
                            trgt_vcpu.vm().unwrap().id()
                        );
                        return;
                    }
                    info!(
                        "Core {} (vm {}, vcpu {}) is woke up",
                        current_cpu().id,
                        trgt_vcpu.vm().unwrap().id(),
                        trgt_vcpu.id()
                    );
                    psci_vcpu_on(trgt_vcpu, power_msg.entry, power_msg.context);
                }
                PowerEvent::PsciIpiCpuOff => {
                    // TODO: 为什么ipi cpu off是当前vcpu shutdown，而vcpu shutdown 最后是把平台的物理核心shutdown
                    // 没有用到。不用管
                    // current_cpu().active_vcpu.clone().unwrap().shutdown();
                    unimplemented!("PowerEvent::PsciIpiCpuOff")
                }
                PowerEvent::PsciIpiCpuReset => {
                    vcpu_arch_init(
                        active_vm().unwrap().config(),
                        current_cpu().active_vcpu.as_ref().unwrap(),
                    );
                }
            }
        }
        _ => {
            panic!(
                "psci_ipi_handler: cpu{} receive illegal psci ipi type",
                current_cpu().id
            );
        }
    }
}

fn psci_guest_cpu_on(mpidr: usize, entry: usize, ctx: usize) -> usize {
    let vcpu_id = mpidr & 0xff;
    let vm = active_vm().unwrap();
    let physical_linear_id = vm.vcpuid_to_pcpuid(vcpu_id);

    if vcpu_id >= vm.cpu_num() || physical_linear_id.is_err() {
        warn!("psci_guest_cpu_on: target vcpu {} not exist", vcpu_id);
        return usize::MAX - 1;
    }
    #[cfg(feature = "tx2")]
    {
        let cluster = (mpidr >> 8) & 0xff;
        if vm.id() == 0 && cluster != 1 {
            warn!("psci_guest_cpu_on: L4T only support cluster #1");
            return usize::MAX - 1;
        }
    }

    let m = IpiPowerMessage {
        src: vm.id(),
        event: PowerEvent::PsciIpiCpuOn,
        entry,
        context: ctx,
    };

    if !ipi_send_msg(physical_linear_id.unwrap(), IpiType::IpiTPower, IpiInnerMsg::Power(m)) {
        warn!("psci_guest_cpu_on: fail to send msg");
        return usize::MAX - 1;
    }

    0
}
