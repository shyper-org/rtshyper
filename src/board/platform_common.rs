use crate::arch::GicDesc;

pub const PLATFORM_CPU_NUM_MAX: usize = 8;
pub const TOTAL_MEM_REGION_MAX: usize = 16;
pub const PLATFORM_VCPU_NUM_MAX: usize = 8;

#[repr(C)]
pub enum SchedRule {
    RoundRobin,
    None,
}

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
    pub sched_list: [SchedRule; PLATFORM_CPU_NUM_MAX],
}

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
