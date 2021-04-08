const NAME_MAX_LEN: usize = 32;
const PASSTHROUGH_DEV_MAX_NUM: usize = 128;
const EMULATED_DEV_MAX_NUM: usize = 16;
use crate::kernel::VM_MEM_REGION_MAX;
use crate::kernel::VM_NUM_MAX;

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
    region: Option<[VmRegion; VM_MEM_REGION_MAX]>,
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
    kernel_name: Option<[u8; NAME_MAX_LEN]>,
    kernel_load_ipa: usize,
    kernel_entry_point: usize,
    device_tree_filename: Option<[u8; NAME_MAX_LEN]>,
    device_tree_load_ipa: usize,
    ramdisk_filename: Option<[u8; NAME_MAX_LEN]>,
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
    pub name: Option<[u8; NAME_MAX_LEN]>,
    pub os_type: VmType,
    pub memory: VmMemoryConfig,
    pub image: VmImageConfig,
    pub cpu: VmCpuConfig,
    pub emu_dev_num: usize,
    pub vm_emu_dev_confg: Option<[VmEmulatedDeviceConfig; EMULATED_DEV_MAX_NUM]>,
    pub pt_dev_num: usize,
    pub vm_pt_dev_confg: Option<[VmPassthroughDeviceConfig; PASSTHROUGH_DEV_MAX_NUM]>,
}

impl VmConfigEntry {
    const fn default() -> VmConfigEntry {
        VmConfigEntry {
            name: None,
            os_type: VmType::VmTBma,
            memory: VmMemoryConfig::default(),
            image: VmImageConfig::default(),
            cpu: VmCpuConfig::default(),
            emu_dev_num: 0,
            vm_emu_dev_confg: None,
            pt_dev_num: 0,
            vm_pt_dev_confg: None,
        }
    }
}

pub struct VmConfigTable {
    pub name: Option<[u8; NAME_MAX_LEN]>,
    pub vm_num: usize,
    pub entries: Option<[VmConfigEntry; VM_NUM_MAX]>,
}

static DEF_VM_CONFIG_TABLE: VmConfigTable = VmConfigTable {
    name: None,
    vm_num: 1,
    entries: Some([
        VmConfigEntry {
            name: None,
            os_type: VmType::VmTOs,
            memory: VmMemoryConfig {
                num: 1,
                region: Some([
                    VmRegion {
                        ipa_start: 0x50000000,
                        length: 0x80000000,
                    },
                    VmRegion::default(),
                    VmRegion::default(),
                    VmRegion::default(),
                ]),
            },
            image: VmImageConfig {
                kernel_name: None,
                kernel_load_ipa: 0x58080000,
                kernel_entry_point: 0x58080000,
                device_tree_filename: None,
                device_tree_load_ipa: 0x52000000,
                ramdisk_filename: None,
                ramdisk_load_ipa: 0x53000000,
            },
            cpu: VmCpuConfig {
                num: 4,
                allocate_bitmap: 0b1111,
                master: 0,
            },
            emu_dev_num: 0,
            vm_emu_dev_confg: None,
            pt_dev_num: 0,
            vm_pt_dev_confg: None,
        },
        VmConfigEntry::default(),
        VmConfigEntry::default(),
        VmConfigEntry::default(),
        VmConfigEntry::default(),
        VmConfigEntry::default(),
        VmConfigEntry::default(),
        VmConfigEntry::default(),
    ]),
};
