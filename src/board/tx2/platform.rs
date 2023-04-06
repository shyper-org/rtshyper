use crate::arch::GicDesc;
use crate::arch::SmmuDesc;
use crate::board::{
    PlatOperation, Platform, PlatCpuCoreConfig, ArchDesc, PlatCpuConfig, PlatformConfig, PlatMemoryConfig,
    PlatMemRegion,
};
use crate::board::SchedRule::RoundRobin;
use crate::device::ARM_CORTEX_A57;
#[allow(unused_imports)]
use crate::device::ARM_NVIDIA_DENVER;

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

    // start sector number (LBA)
    const DISK_PARTITION_0_START: usize = 43643256;
    const DISK_PARTITION_1_START: usize = 4104;
    const DISK_PARTITION_2_START: usize = 45740408;

    // size in sector (512-byte)
    // pub const DISK_PARTITION_TOTAL_SIZE: usize = 31457280;
    const DISK_PARTITION_0_SIZE: usize = 2097152;
    const DISK_PARTITION_1_SIZE: usize = 41943040;
    const DISK_PARTITION_2_SIZE: usize = 8388608;

    const SHARE_MEM_BASE: usize = 0xd_0000_0000;

    fn cpuid_to_cpuif(cpuid: usize) -> usize {
        cpuid + PLAT_DESC.cpu_desc.num
    }

    fn cpuif_to_cpuid(cpuif: usize) -> usize {
        cpuif - PLAT_DESC.cpu_desc.num
    }

    fn blk_init() {
        todo!()
    }

    fn blk_read(_sector: usize, _count: usize, _buf: usize) {
        todo!()
    }

    fn blk_write(_sector: usize, _count: usize, _buf: usize) {
        todo!()
    }

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
}

pub static PLAT_DESC: PlatformConfig = PlatformConfig {
    cpu_desc: PlatCpuConfig {
        num: 4,
        core_list: &[
            PlatCpuCoreConfig {
                name: ARM_CORTEX_A57,
                mpidr: 0x80000100,
                sched: RoundRobin,
            },
            PlatCpuCoreConfig {
                name: ARM_CORTEX_A57,
                mpidr: 0x80000101,
                sched: RoundRobin,
            },
            PlatCpuCoreConfig {
                name: ARM_CORTEX_A57,
                mpidr: 0x80000102,
                sched: RoundRobin,
            },
            PlatCpuCoreConfig {
                name: ARM_CORTEX_A57,
                mpidr: 0x80000103,
                sched: RoundRobin,
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
            PlatMemRegion {
                base: 0x8000_0000,
                size: 0x7000_0000,
            },
            PlatMemRegion {
                base: 0xf020_0000,
                size: 0x1_8560_0000,
            },
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
