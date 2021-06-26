pub const KERNEL_ENTRY: usize = 0x83000000;

pub const TIMER_FREQUENCY: usize = 62500000;

pub const UART_0_ADDR: usize = 0x3100000;
pub const UART_1_ADDR: usize = 0xc280000;

pub const UART_1_INT: usize = 32 + 0x70;
pub const UART_2_INT: usize = 32 + 0x72;

pub const PLATFORM_GICD_BASE: usize = 0x3881000;
pub const PLATFORM_GICC_BASE: usize = 0x3882000;
pub const PLATFORM_GICH_BASE: usize = 0x03884000;
pub const PLATFORM_GICV_BASE: usize = 0x03886000;

pub const DISK_PARTITION_1_ADDR_SIZE: usize = 0x400;
pub const DISK_PARTITION_1_ADDR: usize = 0xa000000;
pub const DISK_PARTITION_2_ADDR: usize = 0xa000400;
pub const DISK_PARTITION_3_ADDR: usize = 0xa000800;
pub const DISK_PARTITION_4_ADDR: usize = 0xa000c00;

// start sector number (LBA)
pub const DISK_PARTITION_0_START: usize = 43643256;
pub const DISK_PARTITION_1_START: usize = 4104;
pub const DISK_PARTITION_2_START: usize = 45740408;

// size in sector (512-byte)
pub const DISK_PARTITION_TOTAL_SIZE: usize = 31457280;
pub const DISK_PARTITION_0_SIZE: usize = 2097152;
pub const DISK_PARTITION_1_SIZE: usize = 41943040;
pub const DISK_PARTITION_2_SIZE: usize = 8388608;

pub const DISK_PARTITION_1_INT: usize = 32 + 0x10;
pub const DISK_PARTITION_2_INT: usize = 32 + 0x11;
pub const DISK_PARTITION_3_INT: usize = 32 + 0x12;
pub const DISK_PARTITION_4_INT: usize = 32 + 0x13;

//end tx2 platform const

use crate::arch::GicDesc;
use crate::board::{ArchDesc, PlatCpuConfig, PlatMemRegion, PlatMemoryConfig, PlatformConfig};
use crate::device::ARM_NVIDIA_DENVER;

// pub static PLAT_DESC: PlatformConfig = PlatformConfig {
// }

pub fn platform_blk_init() {}
