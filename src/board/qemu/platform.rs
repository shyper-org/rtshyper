pub const KERNEL_ENTRY: usize = 0x43000000;

pub const TIMER_FREQUENCY: usize = 62500000;

pub const UART_0_ADDR: usize = 0x9000000;
pub const UART_1_ADDR: usize = 0x9100000;
pub const UART_2_ADDR: usize = 0x9110000;

pub const UART_1_INT: usize = 42;
pub const UART_2_INT: usize = 43;

pub const PLATFORM_GICD_BASE: usize = 0x08000000;
pub const PLATFORM_GICC_BASE: usize = 0x08010000;
pub const PLATFORM_GICH_BASE: usize = 0x08030000;
pub const PLATFORM_GICV_BASE: usize = 0x08040000;

pub const DISK_PARTITION_0_START: usize = 0;
pub const DISK_PARTITION_1_START: usize = 2097152;
pub const DISK_PARTITION_2_START: usize = 10289152;

pub const DISK_PARTITION_TOTAL_SIZE: usize = 18481152;
pub const DISK_PARTITION_0_SIZE: usize = 524288;
pub const DISK_PARTITION_1_SIZE: usize = 8192000;
pub const DISK_PARTITION_2_SIZE: usize = 8192000;

pub const PLATFORM_CPU_NUM_MAX: usize = 8;
pub const TOTAL_MEM_REGION_MAX: usize = 16;
pub const PLATFORM_VCPU_NUM_MAX: usize = 8;

// TODO: move these core name to device
const ARM_CORTEX_A57: u8 = 0;
const ARM_NVIDIA_DENVER: u8 = 0;

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
}

use crate::arch::GicDesc;

#[repr(C)]
pub struct ArchDesc {
    pub gic_desc: GicDesc,
}

#[repr(C)]
pub struct PlatformConfig {
    pub cpu_desc: PlatCpuConfig,
    pub mem_desc: PlatMemoryConfig,
    pub uart_base: usize,
    pub arch_desc: ArchDesc,
}

// holy shit, need to recode later
pub static PLAT_DESC: PlatformConfig = PlatformConfig {
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

pub fn platform_power_on_secondary_cores() {
    for i in 1..PLAT_DESC.cpu_desc.num {
        platform_cpu_on(PLAT_DESC.cpu_desc.mpidr_list[i], KERNEL_ENTRY, 0);
    }
}

// TODO: ipi register
pub fn power_arch_init() {}

pub fn platform_cpuid_to_cpuif(cpuid: usize) -> usize {
    cpuid
}

pub fn platform_cpuif_to_cpuid(cpuif: usize) -> usize {
    cpuif
}
