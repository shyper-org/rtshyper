use core::ops::Range;

use crate::arch::GicDesc;
use crate::arch::SmmuDesc;

pub const PLATFORM_CPU_NUM_MAX: usize = 8;
pub const TOTAL_MEM_REGION_MAX: usize = 16;
pub const PLATFORM_VCPU_NUM_MAX: usize = 8;

#[repr(C)]
pub enum SchedRule {
    RoundRobin,
    None,
}

#[repr(C)]
pub struct PlatMemRegion {
    pub base: usize,
    pub size: usize,
}

#[repr(C)]
pub struct PlatMemoryConfig {
    pub region_num: usize,
    pub base: usize,
    pub regions: [PlatMemRegion; TOTAL_MEM_REGION_MAX],
}

#[repr(C)]
pub struct PlatCpuConfig {
    pub num: usize,
    pub name: [u8; PLATFORM_CPU_NUM_MAX],
    pub mpidr_list: [usize; PLATFORM_CPU_NUM_MAX],
    pub sched_list: [SchedRule; PLATFORM_CPU_NUM_MAX],
}

#[repr(C)]
pub struct ArchDesc {
    pub gic_desc: GicDesc,
    pub smmu_desc: SmmuDesc,
}

#[repr(C)]
pub struct PlatformConfig {
    pub cpu_desc: PlatCpuConfig,
    pub mem_desc: PlatMemoryConfig,
    pub uart_base: usize,
    pub arch_desc: ArchDesc,
}

pub trait PlatOperation {
    // must offer UART_0 and UART_1 address
    const UART_0_ADDR: usize;
    const UART_1_ADDR: usize;
    const UART_2_ADDR: usize = usize::MAX;

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
            Self::cpu_on(PLAT_DESC.cpu_desc.mpidr_list[i], _image_start as usize, 0);
        }
    }

    fn sys_reboot() -> ! {
        println!("Hypervisor reset...");
        crate::arch::power_arch_sys_reset();
        loop {
            core::hint::spin_loop();
        }
    }

    fn sys_shutdown() -> ! {
        println!("Hypervisor shutdown...");
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
}
