// TODO: move these core name to device
use crate::arch::GicDesc;
use crate::arch::SmmuDesc;
use crate::board::{PlatOperation, Platform};
use crate::board::{ArchDesc, PlatCpuConfig, PlatformConfig, PlatMemoryConfig, PlatMemRegion};
use crate::board::SchedRule::{self, RoundRobin};
use crate::device::ARM_CORTEX_A57;
use crate::driver::{read, write};

pub struct QemuPlatform;

impl PlatOperation for QemuPlatform {
    const UART_0_ADDR: usize = 0x9000000;
    const UART_1_ADDR: usize = 0x9100000;
    const UART_2_ADDR: usize = 0x9110000;

    const UART_0_INT: usize = 32 + 0x70;
    const UART_1_INT: usize = 32 + 0x72;

    const GICD_BASE: usize = 0x08000000;
    const GICC_BASE: usize = 0x08010000;
    const GICH_BASE: usize = 0x08030000;
    const GICV_BASE: usize = 0x08040000;

    const SHARE_MEM_BASE: usize = 0x7_0000_0000;

    const DISK_PARTITION_0_START: usize = 0;
    const DISK_PARTITION_1_START: usize = 2097152;
    const DISK_PARTITION_2_START: usize = 10289152;

    const DISK_PARTITION_TOTAL_SIZE: usize = 18481152;
    const DISK_PARTITION_0_SIZE: usize = 524288;
    const DISK_PARTITION_1_SIZE: usize = 8192000;
    const DISK_PARTITION_2_SIZE: usize = 8192000;

    fn cpuid_to_cpuif(cpuid: usize) -> usize {
        cpuid
    }

    fn cpuif_to_cpuid(cpuif: usize) -> usize {
        cpuif
    }

    fn blk_init() {
        todo!()
    }

    fn blk_read(sector: usize, count: usize, buf: usize) {
        read(sector, count, buf);
    }

    fn blk_write(sector: usize, count: usize, buf: usize) {
        write(sector, count, buf);
    }

    fn device_regions() -> &'static [core::ops::Range<usize>] {
        assert_eq!(PLAT_DESC.mem_desc.base, 0x40000000);
        static DEVICES: &[core::ops::Range<usize>] = &[0..PLAT_DESC.mem_desc.base];
        DEVICES
    }
}

// holy shit, need to recode later
pub static PLAT_DESC: PlatformConfig = PlatformConfig {
    cpu_desc: PlatCpuConfig {
        num: 4,
        mpidr_list: [0, 1, 2, 3, 4, 5, 6, 7],
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
        region_num: 2,
        regions: [
            // reserve 0x48000000 ~ 0x48100000 for QEMU dtb
            PlatMemRegion {
                base: 0x40000000,
                size: 0x08000000,
            },
            PlatMemRegion {
                base: 0x50000000,
                size: 0x1f0000000,
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
            PlatMemRegion { base: 0, size: 0 },
        ],
        base: 0x40000000,
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
