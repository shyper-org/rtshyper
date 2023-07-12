use core::ops::Range;

use crate::arch::GicDesc;
use crate::arch::SmmuDesc;

pub const PLATFORM_CPU_NUM_MAX: usize = 8;

pub const ARM_CORTEX_A57: u8 = 0;
// pub const ARM_NVIDIA_DENVER: u8 = 1;

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub enum SchedRule {
    RoundRobin,
    RealTime,
}

pub struct PlatMemoryConfig {
    pub base: usize,
    pub regions: &'static [Range<usize>],
}

pub struct PlatCpuCoreConfig {
    pub name: u8,
    pub mpidr: usize,
    pub sched: SchedRule,
}

pub struct PlatCpuConfig {
    pub num: usize,
    pub core_list: &'static [PlatCpuCoreConfig],
}

pub struct ArchDesc {
    pub gic_desc: GicDesc,
    pub smmu_desc: SmmuDesc,
}

pub struct PlatformConfig {
    pub cpu_desc: PlatCpuConfig,
    pub mem_desc: PlatMemoryConfig,
    pub arch_desc: ArchDesc,
}

pub trait PlatOperation {
    // must offer UART_0 and UART_1 address
    const UART_0_ADDR: usize;
    const UART_1_ADDR: usize;
    const UART_2_ADDR: usize = usize::MAX;

    // must offer hypervisor used uart
    const HYPERVISOR_UART_BASE: usize;

    const UART_0_INT: usize = usize::MAX;
    const UART_1_INT: usize = usize::MAX;
    const UART_2_INT: usize = usize::MAX;

    // must offer interrupt controller
    const GICD_BASE: usize;
    const GICC_BASE: usize;
    const GICH_BASE: usize;
    const GICV_BASE: usize;

    const DISK_PARTITION_0_START: usize = usize::MAX;
    const DISK_PARTITION_1_START: usize = usize::MAX;
    const DISK_PARTITION_2_START: usize = usize::MAX;
    const DISK_PARTITION_3_START: usize = usize::MAX;
    const DISK_PARTITION_4_START: usize = usize::MAX;

    const DISK_PARTITION_TOTAL_SIZE: usize = usize::MAX;
    const DISK_PARTITION_0_SIZE: usize = usize::MAX;
    const DISK_PARTITION_1_SIZE: usize = usize::MAX;
    const DISK_PARTITION_2_SIZE: usize = usize::MAX;
    const DISK_PARTITION_3_SIZE: usize = usize::MAX;
    const DISK_PARTITION_4_SIZE: usize = usize::MAX;

    const SHARE_MEM_BASE: usize;

    fn cpu_on(arch_core_id: usize, entry: usize, ctx: usize) {
        crate::arch::power_arch_cpu_on(arch_core_id, entry, ctx);
    }

    fn cpu_shutdown() {
        crate::arch::power_arch_cpu_shutdown();
    }

    fn power_on_secondary_cores() {
        use super::PLAT_DESC;
        use crate::mm::_image_start;
        for i in 1..PLAT_DESC.cpu_desc.num {
            Self::cpu_on(PLAT_DESC.cpu_desc.core_list[i].mpidr, _image_start as usize, 0);
        }
    }

    fn sys_reboot() -> ! {
        info!("Hypervisor reset...");
        crate::arch::power_arch_sys_reset();
        loop {
            core::hint::spin_loop();
        }
    }

    fn sys_shutdown() -> ! {
        info!("Hypervisor shutdown...");
        crate::arch::power_arch_sys_shutdown();
        loop {
            core::hint::spin_loop();
        }
    }

    fn cpuid_to_cpuif(cpuid: usize) -> usize;

    fn cpuif_to_cpuid(cpuif: usize) -> usize;

    fn blk_init();

    fn blk_read(sector: usize, count: usize, buf: usize);

    fn blk_write(sector: usize, count: usize, buf: usize);

    fn device_regions() -> &'static [Range<usize>];

    fn pmu_irq_list() -> &'static [usize];
}
