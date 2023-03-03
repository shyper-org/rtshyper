use crate::arch::GicDesc;
use crate::arch::SmmuDesc;
use crate::board::{PlatOperation, Platform};
use crate::board::{ArchDesc, PlatCpuConfig, PlatformConfig, PlatMemoryConfig, PlatMemRegion, SchedRule};
use crate::board::SchedRule::RoundRobin;
use crate::device::ARM_CORTEX_A57;
#[allow(unused_imports)]
use crate::device::ARM_NVIDIA_DENVER;

pub struct Pi4Platform;

impl PlatOperation for Pi4Platform {
    const UART_0_ADDR: usize = 0xFE201000;
    const UART_1_ADDR: usize = 0xFE201400;

    const UART_0_INT: usize = 32 + 0x79;
    const UART_1_INT: usize = 32 + 0x79;

    const GICD_BASE: usize = 0xFF841000;
    const GICC_BASE: usize = 0xFF842000;
    const GICH_BASE: usize = 0xFF844000;
    const GICV_BASE: usize = 0xFF846000;

    const SHARE_MEM_BASE: usize = 0x7_0000_0000;

    // start sector number (LBA)
    const DISK_PARTITION_0_START: usize = 2048;
    const DISK_PARTITION_1_START: usize = 526336;
    const DISK_PARTITION_2_START: usize = 17303552;
    const DISK_PARTITION_3_START: usize = 34082816;
    const DISK_PARTITION_4_START: usize = 50862080;

    // size in sector (512-byte)
    const DISK_PARTITION_0_SIZE: usize = 524288;
    const DISK_PARTITION_1_SIZE: usize = 16777216;
    const DISK_PARTITION_2_SIZE: usize = 16777216;
    const DISK_PARTITION_3_SIZE: usize = 16777216;
    const DISK_PARTITION_4_SIZE: usize = 11471872;

    fn cpuid_to_cpuif(cpuid: usize) -> usize {
        cpuid
    }

    fn cpuif_to_cpuid(cpuif: usize) -> usize {
        cpuif
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
}

pub static PLAT_DESC: PlatformConfig = PlatformConfig {
    cpu_desc: PlatCpuConfig {
        num: 4,
        mpidr_list: [0x80000000, 0x80000001, 0x80000002, 0x80000003, 0, 0, 0, 0],
        name: [ARM_CORTEX_A57; 8],
        sched_list: [
            RoundRobin,
            RoundRobin,
            RoundRobin,
            RoundRobin,
            SchedRule::None,
            SchedRule::None,
            SchedRule::None,
            SchedRule::None,
        ],
    },
    mem_desc: PlatMemoryConfig {
        region_num: 4,
        regions: [
            PlatMemRegion {
                base: 0xf0000000,
                size: 0xc000000,
            },
            PlatMemRegion {
                base: 0x200000,
                size: 0x3e000000 - 0x200000,
            },
            PlatMemRegion {
                base: 0x40000000,
                size: 0xf0000000 - 0x40000000,
            },
            PlatMemRegion {
                base: 0x100000000,
                size: 0x100000000,
            },
            PlatMemRegion { base: 0, size: 0 },
            PlatMemRegion { base: 0, size: 0 },
            PlatMemRegion { base: 0, size: 0 },
            PlatMemRegion { base: 0, size: 0 },
            PlatMemRegion { base: 0, size: 0 },
            PlatMemRegion { base: 0, size: 0 },
            PlatMemRegion { base: 0, size: 0 },
            PlatMemRegion { base: 0, size: 0 },
            PlatMemRegion { base: 0, size: 0 },
            PlatMemRegion { base: 0, size: 0 },
            PlatMemRegion { base: 0, size: 0 },
            PlatMemRegion { base: 0, size: 0 },
        ],
        base: 0xf0000000,
    },
    uart_base: Platform::UART_0_ADDR,
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
