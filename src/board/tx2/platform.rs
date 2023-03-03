use crate::arch::GicDesc;
use crate::arch::SmmuDesc;
use crate::board::{PlatOperation, Platform};
use crate::board::{ArchDesc, PlatCpuConfig, PlatformConfig, PlatMemoryConfig, PlatMemRegion, SchedRule};
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
}

pub static PLAT_DESC: PlatformConfig = PlatformConfig {
    cpu_desc: PlatCpuConfig {
        num: 4,
        mpidr_list: [0x80000100, 0x80000101, 0x80000102, 0x80000103, 0, 0, 0, 0],
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
        region_num: 3,
        regions: [
            PlatMemRegion {
                base: 0x80000000,
                size: 0x10000000,
            },
            PlatMemRegion {
                base: 0x90000000,
                size: 0x60000000,
            },
            PlatMemRegion {
                base: 0xf0200000,
                size: 0x185600000,
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
            PlatMemRegion { base: 0, size: 0 },
        ],
        base: 0x80000000,
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
            base: 0x12000000,
            interrupt_id: 187,
            global_mask: 0x7f80,
        },
    },
};
