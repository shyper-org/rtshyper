const NAME_MAX_LEN: usize = 32;
const PASSTHROUGH_DEV_MAX_NUM: usize = 128;
const EMULATED_DEV_MAX_NUM: usize = 16;

use alloc::vec::Vec;
use spin::Mutex;

use crate::board::*;

use crate::kernel::VmType;

use crate::device::EmuDeviceType;

pub struct VmEmulatedDeviceConfig {
    pub name: Option<&'static str>,
    pub base_ipa: usize,
    pub length: usize,
    pub irq_id: usize,
    pub cfg_list: Vec<usize>,
    pub emu_type: EmuDeviceType,
}

pub struct VmPassthroughDeviceConfig {
    pub name: Option<&'static str>,
    pub base_pa: usize,
    pub base_ipa: usize,
    pub length: usize,
    pub dma: bool,
    pub irq_list: Vec<usize>,
}

pub struct VmRegion {
    pub ipa_start: usize,
    pub length: usize,
}

impl VmRegion {
    #[allow(dead_code)]
    const fn default() -> VmRegion {
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
    const fn default() -> VmMemoryConfig {
        VmMemoryConfig {
            num: 0,
            region: None,
        }
    }
}

pub struct VmImageConfig {
    pub kernel_name: Option<&'static str>,
    pub kernel_load_ipa: usize,
    pub kernel_entry_point: usize,
    pub device_tree_filename: Option<&'static str>,
    pub device_tree_load_ipa: usize,
    pub ramdisk_filename: Option<&'static str>,
    pub ramdisk_load_ipa: usize,
}

impl VmImageConfig {
    #[allow(dead_code)]
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
    pub num: usize,
    pub allocate_bitmap: u32,
    pub master: i32,
}

impl VmCpuConfig {
    #[allow(dead_code)]
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
    #[allow(dead_code)]
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
    pub entries: Vec<Arc<VmConfigEntry>>,
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

use alloc::sync::Arc;

lazy_static! {
    pub static ref DEF_VM_CONFIG_TABLE: Mutex<VmConfigTable> = Mutex::new(VmConfigTable::default());
}

pub fn config_init() {
    let mut vm_config = DEF_VM_CONFIG_TABLE.lock();
    vm_config.name = Some("qemu-default");
    vm_config.vm_num = 1;

    // vm0 emu
    let mut emu_dev_config: Vec<VmEmulatedDeviceConfig> = Vec::new();
    emu_dev_config.push(VmEmulatedDeviceConfig {
        name: Some("vgicd"),
        base_ipa: 0x8000000,
        length: 0x1000,
        irq_id: 0,
        cfg_list: Vec::new(),
        emu_type: EmuDeviceType::EmuDeviceTGicd,
    });
    emu_dev_config.push(VmEmulatedDeviceConfig {
        name: Some("virtio-blk0"),
        base_ipa: 0xa000000,
        length: 0x1000,
        irq_id: 32 + 0x10,
        cfg_list: vec![DISK_PARTITION_1_SIZE, DISK_PARTITION_1_START],
        emu_type: EmuDeviceType::EmuDeviceTVirtioBlk,
    });
    // emu_dev_config.push(VmEmulatedDeviceConfig {
    //     name: Some("shyper"),
    //     base_ipa: 0,
    //     length: 0,
    //     irq_id: 32 + 0x20,
    //     cfg_list: Vec::new(),
    //     emu_type: EmuDeviceType::EmuDeviceTShyper,
    // });

    // vm0 passthrough
    let mut pt_dev_config: Vec<VmPassthroughDeviceConfig> = Vec::new();
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("serial0"),
        base_pa: UART_1_ADDR,
        base_ipa: 0x9000000,
        length: 0x1000,
        dma: false,
        irq_list: vec![27, UART_1_INT],
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("gicc"),
        base_pa: PLATFORM_GICV_BASE,
        base_ipa: 0x8010000,
        length: 0x2000,
        dma: false,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("nic"),
        base_pa: 0x0a003000,
        base_ipa: 0x0a003000,
        length: 0x1000,
        dma: false,
        irq_list: vec![32 + 0x2e],
    });

    // vm0 vm_region
    let mut vm_region: Vec<VmRegion> = Vec::new();
    vm_region.push(VmRegion {
        ipa_start: 0x50000000,
        length: 0x80000000,
    });

    // vm0 config
    vm_config.entries.push(Arc::new(VmConfigEntry {
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
            num: 1,
            allocate_bitmap: 0b0001,
            master: 0,
        },
        vm_emu_dev_confg: Some(emu_dev_config),
        vm_pt_dev_confg: Some(pt_dev_config),
    }));
}
