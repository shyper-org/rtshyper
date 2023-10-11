use crate::arch::ArchTrait;
use crate::arch::GicDesc;
use crate::arch::SmmuDesc;
use crate::board::Platform;

use super::platform_common::{
    ArchDesc, PlatCpuConfig, PlatCpuCoreConfig, PlatMemoryConfig, PlatOperation, PlatformConfig, SchedRule,
    ARM_CORTEX_A57,
};

pub struct Tx2Platform;

impl PlatOperation for Tx2Platform {
    const UART_0_ADDR: usize = 0x3100000;
    const UART_1_ADDR: usize = 0xc280000;

    const UART_0_INT: usize = 32 + 0x70;
    const UART_1_INT: usize = 32 + 0x72;

    const HYPERVISOR_UART_BASE: usize = Self::UART_1_ADDR;

    const GICD_BASE: usize = 0x3881000;
    const GICC_BASE: usize = 0x3882000;
    const GICH_BASE: usize = 0x3884000;
    const GICV_BASE: usize = 0x3886000;

    fn cpuid_to_cpuif(cpuid: usize) -> usize {
        cpuid + PLAT_DESC.cpu_desc.num
    }

    fn cpuif_to_cpuid(cpuif: usize) -> usize {
        cpuif - PLAT_DESC.cpu_desc.num
    }

    #[inline]
    fn device_regions() -> &'static [core::ops::Range<usize>] {
        static DEVICES: &[core::ops::Range<usize>] = &[
            0x3000000..0x3200000,
            0xc200000..0xc400000,
            0x3400000..0x3600000,
            0x3800000..0x3a00000,
            0x12000000..0x13000000,
        ];
        DEVICES
    }

    #[inline]
    fn pmu_irq_list() -> &'static [usize] {
        // arm-pmu {
        //      compatible = "arm,armv8-pmuv3";
        //      interrupts = <0x0 0x140 0x4 0x0 0x141 0x4 0x0 0x128 0x4 0x0 0x129 0x4 0x0 0x12a 0x4 0x0 0x12b 0x4>;
        //      interrupt-affinity = <0x2 0x3 0x4 0x5 0x6 0x7>;
        // };
        &[32 + 0x128, 32 + 0x129, 32 + 0x12a, 32 + 0x12b]
    }

    #[inline]
    fn mpidr2cpuid(mpidr: usize) -> usize {
        if mpidr & 0x100 == 0 {
            loop {
                crate::arch::Arch::wait_for_interrupt();
            }
        } else {
            /*
             * only cluster 1 cpu 0,1,2,3 reach here
             * x0 holds core_id (indexed from zero)
             */
            mpidr & 0xff
        }
    }
}

pub static PLAT_DESC: PlatformConfig = PlatformConfig {
    cpu_desc: PlatCpuConfig {
        num: 4,
        core_list: &[
            PlatCpuCoreConfig {
                name: ARM_CORTEX_A57,
                mpidr: 0x80000100,
                sched: SchedRule::RoundRobin,
            },
            PlatCpuCoreConfig {
                name: ARM_CORTEX_A57,
                mpidr: 0x80000101,
                sched: SchedRule::RoundRobin,
            },
            PlatCpuCoreConfig {
                name: ARM_CORTEX_A57,
                mpidr: 0x80000102,
                sched: SchedRule::RoundRobin,
            },
            PlatCpuCoreConfig {
                name: ARM_CORTEX_A57,
                mpidr: 0x80000103,
                sched: SchedRule::RoundRobin,
            },
        ],
    },
    mem_desc: PlatMemoryConfig {
        /*
            cboot told me that
            [0003.848] I> added [base:0x80000000, size:0x70000000] to /memory
            [0003.854] I> added [base:0xf0200000, size:0x185600000] to /memory
        */
        regions: &[
            0x8000_0000..0x8000_0000 + 0x7000_0000,
            0xf020_0000..0xf020_0000 + 0x1_8560_0000,
        ],
        base: 0x80000000,
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
            base: 0x12000000,
            interrupt_id: 187,
            global_mask: 0x7f80,
        },
    },
};
