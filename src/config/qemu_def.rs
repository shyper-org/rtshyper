const NAME_MAX_LEN: usize = 32;
const PASSTHROUGH_DEV_MAX_NUM: usize = 128;
const EMULATED_DEV_MAX_NUM: usize = 16;

use crate::kernel::VM_MEM_REGION_MAX;
use crate::kernel::VM_NUM_MAX;
use alloc::vec::Vec;
use spin::Mutex;

pub enum VmType {
    VmTOs = 0,
    VmTBma = 1,
}

pub struct VmEmulatedDeviceConfig {}
pub struct VmPassthroughDeviceConfig {}

pub struct VmRegion {
    ipa_start: usize,
    length: usize,
}

impl VmRegion {
    const fn default() -> VmRegion {
        VmRegion {
            ipa_start: 0,
            length: 0,
        }
    }
}

pub struct VmMemoryConfig {
    num: u32,
    region: Option<Vec<VmRegion>>,
}

impl VmMemoryConfig {
    const fn default() -> VmMemoryConfig {
        VmMemoryConfig {
            num: 0,
            region: None,
        }
    }
}

pub struct VmImageConfig {
    kernel_name: Option<&'static str>,
    kernel_load_ipa: usize,
    kernel_entry_point: usize,
    device_tree_filename: Option<&'static str>,
    device_tree_load_ipa: usize,
    ramdisk_filename: Option<&'static str>,
    ramdisk_load_ipa: usize,
}

impl VmImageConfig {
    const fn default() -> VmImageConfig {
        VmImageConfig {
            kernel_name: None,
            kernel_load_ipa: 0,
            kernel_entry_point: 0,
            device_tree_filename: None,
            device_tree_load_ipa: 0,
            ramdisk_filename: None,
            ramdisk_load_ipa: 0,
        }
    }
}

pub struct VmCpuConfig {
    num: usize,
    allocate_bitmap: u32,
    master: i32,
}

impl VmCpuConfig {
    const fn default() -> VmCpuConfig {
        VmCpuConfig {
            num: 0,
            allocate_bitmap: 0,
            master: 0,
        }
    }
}

pub struct VmConfigEntry {
    pub name: Option<&'static str>,
    pub os_type: VmType,
    pub memory: VmMemoryConfig,
    pub image: VmImageConfig,
    pub cpu: VmCpuConfig,
    pub vm_emu_dev_confg: Option<Vec<VmEmulatedDeviceConfig>>,
    pub vm_pt_dev_confg: Option<Vec<VmPassthroughDeviceConfig>>,
}

impl VmConfigEntry {
    const fn default() -> VmConfigEntry {
        VmConfigEntry {
            name: None,
            os_type: VmType::VmTBma,
            memory: VmMemoryConfig::default(),
            image: VmImageConfig::default(),
            cpu: VmCpuConfig::default(),
            vm_emu_dev_confg: None,
            vm_pt_dev_confg: None,
        }
    }
}

pub struct VmConfigTable {
    pub name: Option<&'static str>,
    pub vm_num: usize,
    pub entries: Vec<VmConfigEntry>,
}

impl VmConfigTable {
    const fn default() -> VmConfigTable {
        VmConfigTable {
            name: None,
            vm_num: 0,
            entries: Vec::new(),
        }
    }
}

pub static DEF_VM_CONFIG_TABLE: Mutex<VmConfigTable> = Mutex::new(VmConfigTable::default());

pub fn config_init() {
    let mut vm_config = DEF_VM_CONFIG_TABLE.lock();
    vm_config.name = Some("qemu-default");
    vm_config.vm_num = 1;

    // vm0
    let mut emu_dev_config: Vec<VmEmulatedDeviceConfig> = Vec::new();
    emu_dev_config.push(VmEmulatedDeviceConfig {});
    let mut pt_dev_config: Vec<VmPassthroughDeviceConfig> = Vec::new();
    pt_dev_config.push(VmPassthroughDeviceConfig {});
    let mut vm_region: Vec<VmRegion> = Vec::new();
    vm_region.push(VmRegion {
        ipa_start: 0x50000000,
        length: 0x80000000,
    });

    vm_config.entries.push(VmConfigEntry {
        name: Some("supervisor"),
        os_type: VmType::VmTOs,
        memory: VmMemoryConfig {
            num: 1,
            region: Some(vm_region),
        },
        image: VmImageConfig {
            kernel_name: Some("Image"),
            kernel_load_ipa: 0x58080000,
            kernel_entry_point: 0x58080000,
            device_tree_filename: Some("qemu1.bin"),
            device_tree_load_ipa: 0x52000000,
            ramdisk_filename: Some("initrd.gz"),
            ramdisk_load_ipa: 0x53000000,
        },
        cpu: VmCpuConfig {
            num: 4,
            allocate_bitmap: 0b1111,
            master: 0,
        },
        vm_emu_dev_confg: Some(emu_dev_config),
        vm_pt_dev_confg: Some(pt_dev_config),
    });
}
