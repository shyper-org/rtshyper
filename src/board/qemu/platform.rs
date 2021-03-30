pub const TIMER_FREQUENCY: u64 = 62500000;

pub const UART_0_ADDR: u64 = 0x9000000;
pub const UART_1_ADDR: u64 = 0x9100000;
pub const UART_2_ADDR: u64 = 0x9110000;

pub const UART_1_INT: u64 = 42;
pub const UART_2_INT: u64 = 43;

pub const PLATFORM_GICD_BASE: u64 = 0x08000000;
pub const PLATFORM_GICC_BASE: u64 = 0x08010000;
pub const PLATFORM_GICH_BASE: u64 = 0x08030000;
pub const PLATFORM_GICV_BASE: u64 = 0x08040000;

pub const DISK_PARTITION_0_START: u64 = 0;
pub const DISK_PARTITION_1_START: u64 = 2097152;
pub const DISK_PARTITION_2_START: u64 = 10289152;

pub const DISK_PARTITION_TOTAL_SIZE: u64 = 18481152;
pub const DISK_PARTITION_0_SIZE: u64 = 524288;
pub const DISK_PARTITION_1_SIZE: u64 = 8192000;
pub const DISK_PARTITION_2_SIZE: u64 = 8192000;

const CPU_NUM_MAX: usize = 8;
const REGION_NUM_MAX: usize = 4;

// TODO: move these core name to device
const ARM_CORTEX_A57: u8 = 0;
const ARM_NVIDIA_DENVER: u8 = 0;

#[repr(C)]
struct PlatMemRegion {
    base: u64,
    size: u64,
}

#[repr(C)]
pub struct PlatMemoryConfig {
    region_num: u64,
    base: u64,
    regions: [PlatMemRegion; REGION_NUM_MAX],
}

#[repr(C)]
pub struct PlatCpuConfig {
    num: u64,
    name: [u8; CPU_NUM_MAX],
    mpidr_list: [u64; CPU_NUM_MAX],
}

use crate::arch::GicDesc;

#[repr(C)]
pub struct ArchDesc {
    gic_desc: GicDesc,
}

#[repr(C)]
pub struct PlatformConfig {
    cpu_desc: PlatCpuConfig,
    mem_desc: PlatMemoryConfig,
    uart_base: u64,
    arch_desc: ArchDesc,
}

// holy shit, need to recode later
pub const PLAT_DESC: PlatformConfig = PlatformConfig {
    cpu_desc: PlatCpuConfig {
        num: 8,
        mpidr_list: [0, 1, 2, 3, 4, 5, 6, 7],
        name: [ARM_CORTEX_A57; 8],
    },
    mem_desc: PlatMemoryConfig {
        region_num: 2,
        regions: [
            PlatMemRegion {
                base: 0x40000000,
                size: 0x10000000,
            },
            PlatMemRegion {
                base: 0x50000000,
                size: 0x1f0000000,
            },
            PlatMemRegion { base: 0, size: 0 },
            PlatMemRegion { base: 0, size: 0 },
        ],
        base: 0x40000000,
    },
    uart_base: UART_0_ADDR,
    arch_desc: ArchDesc {
        gic_desc: GicDesc {
            gicd_addr: PLATFORM_GICD_BASE,
            gicc_addr: PLATFORM_GICC_BASE,
            gich_addr: PLATFORM_GICH_BASE,
            gicv_addr: PLATFORM_GICV_BASE,
            maintenance_int_id: 25,
        },
    },
};
