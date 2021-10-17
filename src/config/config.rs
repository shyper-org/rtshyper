const NAME_MAX_LEN: usize = 32;
const PASSTHROUGH_DEV_MAX_NUM: usize = 128;
const EMULATED_DEV_MAX_NUM: usize = 16;

// use crate::board::*;
use crate::device::EmuDeviceType;
use crate::kernel::VmType;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

pub enum DtbDevType {
    DevSerial,
    DevGicd,
    DevGicc,
    DevVirtio,
}

pub struct VmEmulatedDeviceConfig {
    pub name: Option<&'static str>,
    pub base_ipa: usize,
    pub length: usize,
    pub irq_id: usize,
    pub cfg_list: Vec<usize>,
    pub emu_type: EmuDeviceType,
    pub mediated: bool,
}

#[derive(Default)]
pub struct PassthroughRegion {
    pub ipa: usize,
    pub pa: usize,
    pub length: usize,
}

// impl PassthroughRegion {
//     pub fn new(ipa: usize, pa: usize, length: usize) -> Self {
//         PassthroughRegion { ipa, pa, length }
//     }
// }

#[derive(Default)]
pub struct VmPassthroughDeviceConfig {
    pub regions: Vec<PassthroughRegion>,
    pub irqs: Vec<usize>,
    pub streams_ids: Vec<usize>,
}

pub struct VmRegion {
    pub ipa_start: usize,
    pub length: usize,
}

impl VmRegion {
    #[allow(dead_code)]
    pub const fn default() -> VmRegion {
        VmRegion {
            ipa_start: 0,
            length: 0,
        }
    }
}

pub struct VmMemoryConfig {
    pub num: u32,
    pub region: Option<Vec<VmRegion>>,
}

impl VmMemoryConfig {
    #[allow(dead_code)]
    pub const fn default() -> VmMemoryConfig {
        VmMemoryConfig {
            num: 0,
            region: None,
        }
    }
}

pub struct VmImageConfig {
    // pub kernel_name: Option<&'static str>,
    pub kernel_load_ipa: usize,
    pub kernel_entry_point: usize,
    // pub device_tree_filename: Option<&'static str>,
    pub device_tree_load_ipa: usize,
    // pub ramdisk_filename: Option<&'static str>,
    pub ramdisk_load_ipa: usize,
}

impl VmImageConfig {
    #[allow(dead_code)]
    pub const fn default() -> VmImageConfig {
        VmImageConfig {
            // kernel_name: None,
            kernel_load_ipa: 0,
            kernel_entry_point: 0,
            // device_tree_filename: None,
            device_tree_load_ipa: 0,
            // ramdisk_filename: None,
            ramdisk_load_ipa: 0,
        }
    }
}

pub struct VmCpuConfig {
    pub num: usize,
    pub allocate_bitmap: u32,
    pub master: i32,
}

impl VmCpuConfig {
    #[allow(dead_code)]
    pub const fn default() -> VmCpuConfig {
        VmCpuConfig {
            num: 0,
            allocate_bitmap: 0,
            master: 0,
        }
    }
}

pub struct AddrRegions {
    pub ipa: usize,
    pub length: usize,
}

pub struct VmDtbDev {
    pub name: &'static str,
    pub dev_type: DtbDevType,
    pub irqs: Vec<usize>,
    pub addr_region: AddrRegions,
}

pub struct VmConfigEntry {
    pub name: Option<&'static str>,
    pub os_type: VmType,
    pub memory: VmMemoryConfig,
    pub image: VmImageConfig,
    pub cpu: VmCpuConfig,
    pub vm_emu_dev_confg: Option<Vec<VmEmulatedDeviceConfig>>,
    pub vm_pt_dev_confg: Option<VmPassthroughDeviceConfig>,
    pub vm_dtb_devs: Option<Vec<VmDtbDev>>,
    pub cmdline: &'static str,
}

impl VmConfigEntry {
    #[allow(dead_code)]
    pub const fn default() -> VmConfigEntry {
        VmConfigEntry {
            name: None,
            os_type: VmType::VmTBma,
            memory: VmMemoryConfig::default(),
            image: VmImageConfig::default(),
            cpu: VmCpuConfig::default(),
            vm_emu_dev_confg: None,
            vm_pt_dev_confg: None,
            vm_dtb_devs: None,
            cmdline: "",
        }
    }

    pub fn gicc_addr(&self) -> usize {
        match &self.vm_dtb_devs {
            Some(vm_dtb_devs) => {
                for dev in vm_dtb_devs {
                    match dev.dev_type {
                        DtbDevType::DevGicc => {
                            return dev.addr_region.ipa;
                        }
                        _ => {}
                    }
                }
            }
            None => {
                return 0;
            }
        }
        0
    }

    pub fn gicd_addr(&self) -> usize {
        match &self.vm_dtb_devs {
            Some(vm_dtb_devs) => {
                for dev in vm_dtb_devs {
                    match dev.dev_type {
                        DtbDevType::DevGicd => {
                            return dev.addr_region.ipa;
                        }
                        _ => {}
                    }
                }
            }
            None => {
                return 0;
            }
        }
        0
    }
}

pub struct VmConfigTable {
    pub name: Option<&'static str>,
    pub vm_num: usize,
    pub entries: Vec<Arc<VmConfigEntry>>,
}

impl VmConfigTable {
    pub const fn default() -> VmConfigTable {
        VmConfigTable {
            name: None,
            vm_num: 0,
            entries: Vec::new(),
        }
    }
}

lazy_static! {
    pub static ref DEF_VM_CONFIG_TABLE: Mutex<VmConfigTable> = Mutex::new(VmConfigTable::default());
}

pub fn vm_num() -> usize {
    let vm_config = DEF_VM_CONFIG_TABLE.lock();
    vm_config.vm_num
}

pub fn vm_type(id: usize) -> VmType {
    let vm_config = DEF_VM_CONFIG_TABLE.lock();
    vm_config.entries[id].os_type
}
