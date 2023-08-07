use crate::arch::GicDesc;
use crate::arch::SmmuDesc;
use crate::board::Platform;

use super::platform_common::{
    ARM_CORTEX_A57, PlatOperation, PlatCpuCoreConfig, ArchDesc, PlatCpuConfig, PlatformConfig, PlatMemoryConfig,
    SchedRule,
};

pub struct QemuPlatform;

impl PlatOperation for QemuPlatform {
    const UART_0_ADDR: usize = 0x9000000;
    const UART_1_ADDR: usize = 0x9100000;
    const UART_2_ADDR: usize = 0x9110000;

    const UART_0_INT: usize = 32 + 0x70;
    const UART_1_INT: usize = 32 + 0x72;

    const HYPERVISOR_UART_BASE: usize = Self::UART_0_ADDR;

    const GICD_BASE: usize = 0x08000000;
    const GICC_BASE: usize = 0x08010000;
    const GICH_BASE: usize = 0x08030000;
    const GICV_BASE: usize = 0x08040000;

    const SHARE_MEM_BASE: usize = 0x7_0000_0000;

    fn cpuid_to_cpuif(cpuid: usize) -> usize {
        cpuid
    }

    fn cpuif_to_cpuid(cpuif: usize) -> usize {
        cpuif
    }

    #[inline]
    fn device_regions() -> &'static [core::ops::Range<usize>] {
        debug_assert_eq!(PLAT_DESC.mem_desc.base, 0x40000000);
        static DEVICES: &[core::ops::Range<usize>] = &[0..PLAT_DESC.mem_desc.base];
        DEVICES
    }

    #[inline]
    fn pmu_irq_list() -> &'static [usize] {
        &[16 + 0x07; 4]
    }
}

pub static PLAT_DESC: PlatformConfig = PlatformConfig {
    cpu_desc: PlatCpuConfig {
        num: 4,
        core_list: &[
            PlatCpuCoreConfig {
                name: ARM_CORTEX_A57,
                mpidr: 0,
                sched: SchedRule::RoundRobin,
            },
            PlatCpuCoreConfig {
                name: ARM_CORTEX_A57,
                mpidr: 1,
                sched: SchedRule::RoundRobin,
            },
            PlatCpuCoreConfig {
                name: ARM_CORTEX_A57,
                mpidr: 2,
                sched: SchedRule::RoundRobin,
            },
            PlatCpuCoreConfig {
                name: ARM_CORTEX_A57,
                mpidr: 3,
                sched: SchedRule::RoundRobin,
            },
        ],
    },
    mem_desc: PlatMemoryConfig {
        regions: &[
            // reserve 0x48000000 ~ 0x48100000 for QEMU dtb
            0x40000000..0x48000000,
            0x50000000..0x50000000 + 0x1f0000000,
        ],
        base: 0x40000000,
    },
    arch_desc: ArchDesc {
        gic_desc: GicDesc {
            gicd_addr: Platform::GICD_BASE,
            gicc_addr: Platform::GICC_BASE,
            gich_addr: Platform::GICH_BASE,
            gicv_addr: Platform::GICV_BASE,
            maintenance_int_id: 25,
        },
        smmu_desc: SmmuDesc {
            base: 0,
            interrupt_id: 0,
            global_mask: 0,
        },
    },
};
