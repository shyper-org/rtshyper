use crate::arch::GicDesc;
use crate::board::{ArchDesc, PlatCpuConfig, PlatformConfig, PlatMemoryConfig, PlatMemRegion, SchedRule};
use crate::board::SchedRule::RoundRobin;
use crate::device::ARM_CORTEX_A57;
#[allow(unused_imports)]
use crate::device::ARM_NVIDIA_DENVER;

pub const KERNEL_ENTRY: usize = 0x83000000;

// pub const TIMER_FREQUENCY: usize = 62500000;

pub const UART_0_ADDR: usize = 0x3100000;
pub const UART_1_ADDR: usize = 0xc280000;

pub const UART_0_INT: usize = 32 + 0x70;
pub const UART_1_INT: usize = 32 + 0x72;

pub const PLATFORM_GICD_BASE: usize = 0x3881000;
pub const PLATFORM_GICC_BASE: usize = 0x3882000;
pub const PLATFORM_GICH_BASE: usize = 0x3884000;
pub const PLATFORM_GICV_BASE: usize = 0x3886000;

// pub const DISK_PARTITION_1_ADDR_SIZE: usize = 0x400;
// pub const DISK_PARTITION_1_ADDR: usize = 0xa000000;
// pub const DISK_PARTITION_2_ADDR: usize = 0xa000400;
// pub const DISK_PARTITION_3_ADDR: usize = 0xa000800;
// pub const DISK_PARTITION_4_ADDR: usize = 0xa000c00;

// start sector number (LBA)
pub const DISK_PARTITION_0_START: usize = 43643256;
pub const DISK_PARTITION_1_START: usize = 4104;
pub const DISK_PARTITION_2_START: usize = 45740408;

// size in sector (512-byte)
// pub const DISK_PARTITION_TOTAL_SIZE: usize = 31457280;
pub const DISK_PARTITION_0_SIZE: usize = 2097152;
pub const DISK_PARTITION_1_SIZE: usize = 41943040;
pub const DISK_PARTITION_2_SIZE: usize = 8388608;

// pub const DISK_PARTITION_1_INT: usize = 32 + 0x10;
// pub const DISK_PARTITION_2_INT: usize = 32 + 0x11;
// pub const DISK_PARTITION_3_INT: usize = 32 + 0x12;
// pub const DISK_PARTITION_4_INT: usize = 32 + 0x13;

//end tx2 platform const

// extern "C" {
//     fn tegra_emmc_init();
//     fn tegra_emmc_blk_read(sector: usize, count: usize, buf: *mut u8);
//     fn tegra_emmc_blk_write(sector: usize, count: usize, buf: *const u8);
// }

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

fn platform_cpu_on(arch_core_id: usize, entry: usize, ctx: usize) {
    use crate::arch::power_arch_cpu_on;
    power_arch_cpu_on(arch_core_id, entry, ctx);
}

pub fn platform_cpu_shutdown() {
    crate::arch::power_arch_cpu_shutdown();
}

pub fn platform_power_on_secondary_cores() {
    for i in 1..PLAT_DESC.cpu_desc.num {
        platform_cpu_on(PLAT_DESC.cpu_desc.mpidr_list[i], KERNEL_ENTRY, 0);
    }
}

pub fn platform_sys_reboot() {
    println!("Hypervisor reset...");
    // mem_heap_reset();
    crate::arch::power_arch_sys_reset();
    loop {}
}

pub fn platform_sys_shutdown() {
    println!("Hypervisor shutdown...");
    // mem_heap_reset();
    crate::arch::power_arch_sys_shutdown();
    loop {}
}

// TODO
// pub fn platform_blk_init() {
//     unsafe {
//         tegra_emmc_init();
//     }
//     println!("Platform block driver init ok");
// }
//
// pub fn platform_blk_read(sector: usize, count: usize, buf: usize) {
//     unsafe {
//         tegra_emmc_blk_read(sector, count, buf as *mut u8);
//     }
// }
//
// pub fn platform_blk_write(sector: usize, count: usize, buf: usize) {
//     unsafe {
//         tegra_emmc_blk_write(sector, count, buf as *const u8);
//     }
// }

pub fn platform_cpuid_to_cpuif(cpuid: usize) -> usize {
    cpuid + PLAT_DESC.cpu_desc.num
}

pub fn platform_cpuif_to_cpuid(cpuif: usize) -> usize {
    cpuif - PLAT_DESC.cpu_desc.num
}
