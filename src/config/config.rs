use alloc::sync::Arc;
use alloc::vec::Vec;

use spin::Mutex;

use crate::board::*;
// use crate::board::*;
use crate::device::EmuDeviceType;
use crate::kernel::{INTERRUPT_IRQ_GUEST_TIMER, VmType};

const NAME_MAX_LEN: usize = 32;
const PASSTHROUGH_DEV_MAX_NUM: usize = 128;
const EMULATED_DEV_MAX_NUM: usize = 16;

pub enum DtbDevType {
    DevSerial,
    DevGicd,
    DevGicc,
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
    pub const fn default() -> VmRegion {
        VmRegion {
            ipa_start: 0,
            length: 0,
        }
    }
}

pub struct VmMemoryConfig {
    pub region: Vec<VmRegion>,
}

impl VmMemoryConfig {
    pub const fn default() -> VmMemoryConfig {
        VmMemoryConfig { region: vec![] }
    }
}

pub struct VmImageConfig {
    pub kernel_img_name: Option<&'static str>,
    pub kernel_load_ipa: usize,
    pub kernel_entry_point: usize,
    // pub device_tree_filename: Option<&'static str>,
    pub device_tree_load_ipa: usize,
    // pub ramdisk_filename: Option<&'static str>,
    pub ramdisk_load_ipa: usize,
}

impl VmImageConfig {
    pub const fn default() -> VmImageConfig {
        VmImageConfig {
            kernel_img_name: None,
            kernel_load_ipa: 0,
            kernel_entry_point: 0,
            // device_tree_filename: None,
            device_tree_load_ipa: 0,
            // ramdisk_filename: None,
            ramdisk_load_ipa: 0,
        }
    }
}

#[derive(Clone)]
pub struct VmCpuConfig {
    pub num: usize,
    pub allocate_bitmap: u32,
    pub master: i32,
}

impl VmCpuConfig {
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
    pub med_blk_idx: Option<usize>,
}

impl VmConfigEntry {
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
            med_blk_idx: None,
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
    vm_config.entries.len()
}

pub fn vm_type(id: usize) -> VmType {
    let vm_config = DEF_VM_CONFIG_TABLE.lock();
    vm_config.entries[id].os_type
}

pub fn vm_cfg_entry(id: usize) -> Arc<VmConfigEntry> {
    let table = DEF_VM_CONFIG_TABLE.lock();
    table.entries[id].clone()
}

// set vm config in DEF_VM_CONFIG_TABLE
pub fn vm_config_add_vm(
    vmtype: usize,
    cmdline_ipa: usize,
    kernel_entry_point: usize,
    kernel_load_ipa: usize,
    device_tree_load_ipa: usize,
) -> bool {
    let mut vm_config = DEF_VM_CONFIG_TABLE.lock();
    let mut newVm = VmConfigEntry {
        name: None,
        os_type: VmType::from_usize(vmtype),
        memory: VmMemoryConfig::default(),
        image: VmImageConfig {
            kernel_img_name: None,
            kernel_load_ipa,
            kernel_entry_point,
            device_tree_load_ipa,
            ramdisk_load_ipa: 0,
        },
        cpu: VmCpuConfig::default(),
        vm_emu_dev_confg: None,
        vm_pt_dev_confg: None,
        vm_dtb_devs: None,
        cmdline: "",
        med_blk_idx: None,
    };
    // memcpy_safe(newVm.cmdline as *mut u8, vm_ipa2pa(active_vm().unwrap(), cmdline_ipa) as *mut u8, NAME_MAX_LEN);

    vm_config.entries.push(Arc::new(newVm));
    true
}

pub fn vm_config_del_vm() -> bool {
    true
}

pub fn vm_config_set_cpu(vmid: usize, num: usize, allocate_bitmap: usize) -> bool {
    true
}

pub fn vm_config_add_emu_dev(vmid: usize) -> bool {
    true
}

pub fn vm_config_add_pt_dev(vmid: usize) -> bool {
    true
}

pub fn vm_config_add_dtb_dev(vmid: usize) -> bool {
    true
}

pub fn init_tmp_config_for_vm1() {
    println!("init_tmp_config_for_vm1");
    let mut vm_config = DEF_VM_CONFIG_TABLE.lock();
    // #################### vm1 emu ######################
    let mut emu_dev_config: Vec<VmEmulatedDeviceConfig> = Vec::new();
    emu_dev_config.push(VmEmulatedDeviceConfig {
        name: Some("intc@8000000"),
        base_ipa: 0x8000000,
        length: 0x1000,
        irq_id: 0,
        cfg_list: Vec::new(),
        emu_type: EmuDeviceType::EmuDeviceTGicd,
        mediated: false,
    });
    emu_dev_config.push(VmEmulatedDeviceConfig {
        name: Some("virtio_blk@a000000"),
        base_ipa: 0xa000000,
        length: 0x1000,
        irq_id: 32 + 0x10,
        // cfg_list: vec![DISK_PARTITION_2_START, DISK_PARTITION_2_SIZE],
        // cfg_list: vec![0, 8388608],
        // cfg_list: vec![0, 67108864], // 32G
        cfg_list: vec![0, 209715200], // 100G
        emu_type: EmuDeviceType::EmuDeviceTVirtioBlk,
        mediated: true,
    });
    emu_dev_config.push(VmEmulatedDeviceConfig {
        name: Some("virtio_console@a002000"),
        base_ipa: 0xa002000,
        length: 0x1000,
        irq_id: 32 + 0x12,
        cfg_list: vec![0, 0xa002000],
        emu_type: EmuDeviceType::EmuDeviceTVirtioConsole,
        mediated: false,
    });
    emu_dev_config.push(VmEmulatedDeviceConfig {
        name: Some("virtio_net@a001000"),
        base_ipa: 0xa001000,
        length: 0x1000,
        irq_id: 32 + 0x11,
        cfg_list: vec![0x74, 0x56, 0xaa, 0x0f, 0x47, 0xd1],
        emu_type: EmuDeviceType::EmuDeviceTVirtioNet,
        mediated: false,
    });
    // emu_dev_config.push(VmEmulatedDeviceConfig {
    //     name: Some("vm_service"),
    //     base_ipa: 0,
    //     length: 0,
    //     irq_id: HVC_IRQ,
    //     cfg_list: Vec::new(),
    //     emu_type: EmuDeviceType::EmuDeviceTShyper,
    //     mediated: false,
    // });

    // vm1 passthrough
    let mut pt_dev_config: VmPassthroughDeviceConfig = VmPassthroughDeviceConfig::default();
    pt_dev_config.regions = vec![
        // PassthroughRegion { ipa: UART_1_ADDR, pa: UART_1_ADDR, length: 0x1000 },
        PassthroughRegion {
            ipa: 0x8010000,
            pa: PLATFORM_GICV_BASE,
            length: 0x2000,
        },
    ];
    // pt_dev_config.irqs = vec![UART_1_INT, INTERRUPT_IRQ_GUEST_TIMER];
    pt_dev_config.irqs = vec![INTERRUPT_IRQ_GUEST_TIMER];

    // vm1 vm_region
    let mut vm_region: Vec<VmRegion> = Vec::new();
    vm_region.push(VmRegion {
        ipa_start: 0x80000000,
        length: 0x40000000,
    });

    let mut vm_dtb_devs: Vec<VmDtbDev> = vec![];
    vm_dtb_devs.push(VmDtbDev {
        name: "gicd",
        dev_type: DtbDevType::DevGicd,
        irqs: vec![],
        addr_region: AddrRegions {
            ipa: 0x8000000,
            length: 0x1000,
        },
    });
    vm_dtb_devs.push(VmDtbDev {
        name: "gicc",
        dev_type: DtbDevType::DevGicc,
        irqs: vec![],
        addr_region: AddrRegions {
            ipa: 0x8010000,
            length: 0x2000,
        },
    });
    // vm_dtb_devs.push(VmDtbDev {
    //     name: "serial",
    //     dev_type: DtbDevType::DevSerial,
    //     irqs: vec![UART_1_INT],
    //     addr_region: AddrRegions {
    //         ipa: UART_1_ADDR,
    //         length: 0x1000,
    //     },
    // });

    // vm1 config
    vm_config.entries.push(Arc::new(VmConfigEntry {
        name: Some("guest-os-0"),
        os_type: VmType::VmTOs,
        memory: VmMemoryConfig { region: vm_region },
        image: VmImageConfig {
            kernel_img_name: None,
            kernel_load_ipa: 0x80080000,
            kernel_entry_point: 0x80080000,
            device_tree_load_ipa: 0x80000000,
            ramdisk_load_ipa: 0, //0x83000000,
        },
        cpu: VmCpuConfig {
            num: 1,
            allocate_bitmap: 0b0100,
            master: 2,
        },
        vm_emu_dev_confg: Some(emu_dev_config),
        vm_pt_dev_confg: Some(pt_dev_config),
        vm_dtb_devs: Some(vm_dtb_devs),
        med_blk_idx: Some(0),
        // cmdline: "root=/dev/vda rw audit=0",
        cmdline: "earlycon console=hvc0,115200n8 root=/dev/vda rw audit=0",
    }));
}

pub fn init_tmp_config_for_vm2() {
    println!("init_tmp_config_for_vm2");
    let mut vm_config = DEF_VM_CONFIG_TABLE.lock();
    // #################### bare metal app emu (vm2) ######################
    // let mut emu_dev_config: Vec<VmEmulatedDeviceConfig> = Vec::new();
    // emu_dev_config.push(VmEmulatedDeviceConfig {
    //     name: Some("intc@8000000"),
    //     base_ipa: 0x8000000,
    //     length: 0x1000,
    //     irq_id: 0,
    //     cfg_list: Vec::new(),
    //     emu_type: EmuDeviceType::EmuDeviceTGicd,
    //     mediated: false,
    // });
    // emu_dev_config.push(VmEmulatedDeviceConfig {
    //     name: Some("virtio_blk@a000000"),
    //     base_ipa: 0xa000000,
    //     length: 0x1000,
    //     irq_id: 32 + 0x10,
    //     cfg_list: vec![0, 209715200], // 100G
    //     emu_type: EmuDeviceType::EmuDeviceTVirtioBlk,
    //     mediated: true,
    // });
    //
    // // bma passthrough
    // let mut pt_dev_config: VmPassthroughDeviceConfig = VmPassthroughDeviceConfig::default();
    // pt_dev_config.regions = vec![
    //     PassthroughRegion { ipa: 0x9000000, pa: UART_1_ADDR, length: 0x1000 },
    //     PassthroughRegion { ipa: 0x8010000, pa: PLATFORM_GICV_BASE, length: 0x2000 },
    // ];
    // pt_dev_config.irqs = vec![UART_1_INT];
    //
    // // bma vm_region
    // let mut vm_region: Vec<VmRegion> = Vec::new();
    // vm_region.push(VmRegion {
    //     ipa_start: 0x40000000,
    //     length: 0x40000000,
    // });
    //
    // // bma config
    // vm_config.entries.push(Arc::new(VmConfigEntry {
    //     name: Some("guest-bma-0"),
    //     os_type: VmType::VmTBma,
    //     memory: VmMemoryConfig {
    //         region: vm_region,
    //     },
    //     image: VmImageConfig {
    //         kernel_load_ipa: 0x40080000,
    //         kernel_entry_point: 0x40080000,
    //         device_tree_load_ipa: 0,
    //         ramdisk_load_ipa: 0,
    //     },
    //     cpu: VmCpuConfig {
    //         num: 1,
    //         allocate_bitmap: 0b0100,
    //         master: 2,
    //     },
    //     vm_emu_dev_confg: Some(emu_dev_config),
    //     vm_pt_dev_confg: Some(pt_dev_config),
    //     vm_dtb_devs: None,
    //     med_blk_idx: Some(1),
    //     cmdline: "",
    // }));

    // #################### vm2 emu ######################
    let mut emu_dev_config: Vec<VmEmulatedDeviceConfig> = Vec::new();
    emu_dev_config.push(VmEmulatedDeviceConfig {
        name: Some("intc@8000000"),
        base_ipa: 0x8000000,
        length: 0x1000,
        irq_id: 0,
        cfg_list: Vec::new(),
        emu_type: EmuDeviceType::EmuDeviceTGicd,
        mediated: false,
    });
    emu_dev_config.push(VmEmulatedDeviceConfig {
        name: Some("virtio_blk@a000000"),
        base_ipa: 0xa000000,
        length: 0x1000,
        irq_id: 32 + 0x10,
        cfg_list: vec![0, 209715200], // 100G
        emu_type: EmuDeviceType::EmuDeviceTVirtioBlk,
        mediated: true,
    });
    emu_dev_config.push(VmEmulatedDeviceConfig {
        name: Some("virtio_net@a001000"),
        base_ipa: 0xa001000,
        length: 0x1000,
        irq_id: 32 + 0x11,
        cfg_list: vec![0x74, 0x56, 0xaa, 0x0f, 0x47, 0xd2],
        emu_type: EmuDeviceType::EmuDeviceTVirtioNet,
        mediated: false,
    });

    // vm2 passthrough
    let mut pt_dev_config: VmPassthroughDeviceConfig = VmPassthroughDeviceConfig::default();
    pt_dev_config.regions = vec![
        PassthroughRegion {
            ipa: UART_1_ADDR,
            pa: UART_1_ADDR,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x8010000,
            pa: PLATFORM_GICV_BASE,
            length: 0x2000,
        },
    ];
    pt_dev_config.irqs = vec![UART_1_INT, INTERRUPT_IRQ_GUEST_TIMER];
    // pt_dev_config.irqs = vec![INTERRUPT_IRQ_GUEST_TIMER];

    // vm2 vm_region
    let mut vm_region: Vec<VmRegion> = Vec::new();
    vm_region.push(VmRegion {
        ipa_start: 0x80000000,
        length: 0x40000000,
    });

    let mut vm_dtb_devs: Vec<VmDtbDev> = vec![];
    vm_dtb_devs.push(VmDtbDev {
        name: "gicd",
        dev_type: DtbDevType::DevGicd,
        irqs: vec![],
        addr_region: AddrRegions {
            ipa: 0x8000000,
            length: 0x1000,
        },
    });
    vm_dtb_devs.push(VmDtbDev {
        name: "gicc",
        dev_type: DtbDevType::DevGicc,
        irqs: vec![],
        addr_region: AddrRegions {
            ipa: 0x8010000,
            length: 0x2000,
        },
    });
    vm_dtb_devs.push(VmDtbDev {
        name: "serial",
        dev_type: DtbDevType::DevSerial,
        irqs: vec![UART_1_INT],
        addr_region: AddrRegions {
            ipa: UART_1_ADDR,
            length: 0x1000,
        },
    });

    // vm2 config
    vm_config.entries.push(Arc::new(VmConfigEntry {
        name: Some("guest-os-1"),
        os_type: VmType::VmTOs,
        memory: VmMemoryConfig { region: vm_region },
        image: VmImageConfig {
            kernel_img_name: None,
            kernel_load_ipa: 0x80080000,
            kernel_entry_point: 0x80080000,
            device_tree_load_ipa: 0x80000000,
            ramdisk_load_ipa: 0,
        },
        cpu: VmCpuConfig {
            num: 1,
            allocate_bitmap: 0b0100,
            master: 2,
        },
        vm_emu_dev_confg: Some(emu_dev_config),
        vm_pt_dev_confg: Some(pt_dev_config),
        vm_dtb_devs: Some(vm_dtb_devs),
        med_blk_idx: Some(1),
        // cmdline: "root=/dev/vda rw audit=0",
        cmdline: "earlycon console=ttyS0,115200n8 root=/dev/vda rw audit=0",
    }));
}

pub fn init_tmp_config_for_vm3() {
    println!("init_tmp_config_for_vm3");
    let mut vm_config = DEF_VM_CONFIG_TABLE.lock();
    // #################### vm3 emu (SRT VM)######################
    let mut emu_dev_config: Vec<VmEmulatedDeviceConfig> = Vec::new();
    emu_dev_config.push(VmEmulatedDeviceConfig {
        name: Some("intc@8000000"),
        base_ipa: 0x8000000,
        length: 0x1000,
        irq_id: 0,
        cfg_list: Vec::new(),
        emu_type: EmuDeviceType::EmuDeviceTGPPT,
        mediated: false,
    });

    // vm3 passthrough
    let mut pt_dev_config: VmPassthroughDeviceConfig = VmPassthroughDeviceConfig::default();
    pt_dev_config.regions = vec![
        PassthroughRegion {
            ipa: UART_1_ADDR,
            pa: UART_1_ADDR,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x8010000,
            pa: PLATFORM_GICC_BASE,
            length: 0x2000,
        },
    ];
    pt_dev_config.irqs = vec![UART_1_INT, INTERRUPT_IRQ_GUEST_TIMER];

    // vm3 vm_region
    let mut vm_region: Vec<VmRegion> = Vec::new();
    vm_region.push(VmRegion {
        ipa_start: 0x80000000,
        length: 0x40000000,
    });

    let mut vm_dtb_devs: Vec<VmDtbDev> = vec![];
    vm_dtb_devs.push(VmDtbDev {
        name: "gicd",
        dev_type: DtbDevType::DevGicd,
        irqs: vec![],
        addr_region: AddrRegions {
            ipa: 0x8000000,
            length: 0x1000,
        },
    });
    vm_dtb_devs.push(VmDtbDev {
        name: "gicc",
        dev_type: DtbDevType::DevGicc,
        irqs: vec![],
        addr_region: AddrRegions {
            ipa: 0x8010000,
            length: 0x2000,
        },
    });
    vm_dtb_devs.push(VmDtbDev {
        name: "serial",
        dev_type: DtbDevType::DevSerial,
        irqs: vec![UART_1_INT],
        addr_region: AddrRegions {
            ipa: UART_1_ADDR,
            length: 0x1000,
        },
    });

    // vm3 config
    vm_config.entries.push(Arc::new(VmConfigEntry {
        name: Some("guest-os-2"),
        os_type: VmType::VmTOs,
        memory: VmMemoryConfig { region: vm_region },
        image: VmImageConfig {
            kernel_img_name: None,
            kernel_load_ipa: 0x80080000,
            kernel_entry_point: 0x80080000,
            device_tree_load_ipa: 0x80000000,
            ramdisk_load_ipa: 0x83000000,
        },
        cpu: VmCpuConfig {
            num: 1,
            allocate_bitmap: 0b1000,
            master: -1,
        },
        vm_emu_dev_confg: Some(emu_dev_config),
        vm_pt_dev_confg: Some(pt_dev_config),
        vm_dtb_devs: Some(vm_dtb_devs),
        med_blk_idx: None,
        cmdline: "earlycon console=ttyS0,115200n8 audit=0",
    }));
}
