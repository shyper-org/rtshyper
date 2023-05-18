use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use spin::Mutex;

use crate::board::*;
use crate::device::EmuDeviceType;
use crate::kernel::{HVC_IRQ, VmType, HYPERVISOR_COLORS};

use super::{
    VmConfigEntry, VmCpuConfig, VmEmulatedDeviceConfig, VmImageConfig, VmMemoryConfig, VmPassthroughDeviceConfig,
    VmRegion, PassthroughRegion, vm_cfg_add_vm_entry, VmEmulatedDeviceConfigList, VMDtbDevConfigList,
};

#[rustfmt::skip]
pub fn mvm_config_init() {
    // vm0 emu
    let emu_dev_config = vec![
        VmEmulatedDeviceConfig {
            name: String::from("vgicd"),
            base_ipa: Platform::GICD_BASE,
            length: 0x1000,
            irq_id: 0,
            cfg_list: Vec::new(),
            emu_type: EmuDeviceType::EmuDeviceTGicd,
            mediated: false,
        },
        // VmEmulatedDeviceConfig {
        //     name: String::from("virtio-blk0"),
        //     base_ipa: 0xa000000,
        //     length: 0x1000,
        //     irq_id: 32 + 0x10,
        //     cfg_list: vec![DISK_PARTITION_1_START, DISK_PARTITION_1_SIZE],
        //     emu_type: EmuDeviceType::EmuDeviceTVirtioBlk,
        //     mediated: false,
        // },
        VmEmulatedDeviceConfig {
            name: String::from("virtio-nic0"),
            base_ipa: 0xa001000,
            length: 0x1000,
            irq_id: 32 + 0x11,
            cfg_list: vec![0x74, 0x56, 0xaa, 0x0f, 0x47, 0xd0],
            emu_type: EmuDeviceType::EmuDeviceTVirtioNet,
            mediated: false,
        },
        VmEmulatedDeviceConfig {
            name: String::from("shyper"),
            base_ipa: 0,
            length: 0,
            irq_id: HVC_IRQ,
            cfg_list: Vec::new(),
            emu_type: EmuDeviceType::EmuDeviceTShyper,
            mediated: false,
        }
    ];

    // vm0 passthrough
    let mut pt_dev_config: VmPassthroughDeviceConfig = VmPassthroughDeviceConfig::default();
    pt_dev_config.regions = vec![
        PassthroughRegion { ipa: Platform::UART_0_ADDR, pa: Platform::UART_0_ADDR, length: 0x1000, dev_property: true },
        PassthroughRegion { ipa: Platform::GICC_BASE, pa: Platform::GICV_BASE, length: 0x2000, dev_property: true },
        // pass-througn virtio blk/net
        PassthroughRegion { ipa: 0x0a003000, pa: 0x0a003000, length: 0x1000, dev_property: true },
    ];
    pt_dev_config.irqs = vec![33, 27, 32 + 0x28, 32 + 0x29];
    pt_dev_config.streams_ids = vec![];
    // pt_dev_config.push(VmPassthroughDeviceConfig {
    //     name: String::from("serial0"),
    //     base_pa: UART_1_ADDR,
    //     base_ipa: 0x9000000,
    //     length: 0x1000,
    //     // dma: false,
    //     irq_list: vec![UART_1_INT, 27],
    // });
    // pt_dev_config.push(VmPassthroughDeviceConfig {
    //     name: String::from("gicc"),
    //     base_pa: Platform::GICV_BASE,
    //     base_ipa: 0x8010000,
    //     length: 0x2000,
    //     // dma: false,
    //     irq_list: Vec::new(),
    // });
    // pt_dev_config.push(VmPassthroughDeviceConfig {
    //     name: String::from("nic"),
    //     base_pa: 0x0a003000,
    //     base_ipa: 0x0a003000,
    //     length: 0x1000,
    //     irq_list: vec![32 + 0x2e],
    // });

    // vm0 vm_region
    let vm_region = vec![
        VmRegion {
            ipa_start: 0x50000000,
            length: 0x80000000,
        }
    ];

    // vm0 config
    let mvm_config_entry =VmConfigEntry {
        id: 0,
        name: String::from("supervisor"),
        os_type: VmType::VmTOs,
        cmdline: String::from("earlycon console=ttyAMA0 root=/dev/vda rw audit=0 default_hugepagesz=32M hugepagesz=32M hugepages=4\0"),
        image: Arc::new(VmImageConfig {
            kernel_img_name: Some("Image"),
            kernel_load_ipa: 0x80080000,
            kernel_entry_point: 0x80080000,
            // device_tree_filename: Some("qemu1.bin"),
            device_tree_load_ipa: 0x80000000,
            // ramdisk_filename: Some("initrd.gz"),
            // ramdisk_load_ipa: 0x53000000,
            ramdisk_load_ipa: 0,
        }),
        cpu: Arc::new(Mutex::new(VmCpuConfig {
            num: 4,
            allocate_bitmap: 0b1111,
            master: -1,
        })),
        memory: Arc::new(Mutex::new(VmMemoryConfig {
            region: vm_region,
            colors: HYPERVISOR_COLORS.get().unwrap().clone(),
        })),
        vm_emu_dev_confg: Arc::new(Mutex::new(VmEmulatedDeviceConfigList { emu_dev_list: emu_dev_config })),
        vm_pt_dev_confg: Arc::new(Mutex::new(pt_dev_config)),
        vm_dtb_devs: Arc::new(Mutex::new(VMDtbDevConfigList::default())),
        mediated_block_index: Arc::new(Mutex::new(None)),
    };
    let _ = vm_cfg_add_vm_entry(mvm_config_entry);
}

// pub fn config_init() {
//     let mut vm_config = DEF_VM_CONFIG_TABLE.lock();
//     // vm1 emu
//     let mut emu_dev_config: Vec<VmEmulatedDeviceConfig> = Vec::new();
//     emu_dev_config.push(VmEmulatedDeviceConfig {
//         name: String::from("vgicd"),
//         base_ipa: 0x8000000,
//         length: 0x1000,
//         irq_id: 0,
//         cfg_list: Vec::new(),
//         emu_type: EmuDeviceType::EmuDeviceTGicd,
//         mediated: false,
//     });
//     emu_dev_config.push(VmEmulatedDeviceConfig {
//         name: String::from("virtio-blk1"),
//         base_ipa: 0xa000000,
//         length: 0x1000,
//         irq_id: 32 + 0x10,
//         cfg_list: vec![DISK_PARTITION_2_START, DISK_PARTITION_2_SIZE],
//         emu_type: EmuDeviceType::EmuDeviceTVirtioBlk,
//         mediated: false,
//     });

//     // vm1 passthrough
//     let mut pt_dev_config: Vec<VmPassthroughDeviceConfig> = Vec::new();
//     // pt_dev_config.push(VmPassthroughDeviceConfig {
//     //     name: String::from("serial1"),
//     //     base_pa: UART_2_ADDR,
//     //     base_ipa: 0x9000000,
//     //     length: 0x1000,
//     //     // dma: false,
//     //     irq_list: vec![UART_2_INT, 27],
//     // });
//     // pt_dev_config.push(VmPassthroughDeviceConfig {
//     //     name: String::from("gicc"),
//     //     base_pa: Platform::GICV_BASE,
//     //     base_ipa: 0x8010000,
//     //     length: 0x2000,
//     //     // dma: false,
//     //     irq_list: Vec::new(),
//     // });
//     // vm1 vm_region
//     let mut vm_region: Vec<VmRegion> = Vec::new();
//     vm_region.push(VmRegion {
//         ipa_start: 0x80000000,
//         length: 0x80000000,
//     });

//     // vm1 config
//     vm_config.entries.push(Arc::new(VmConfigEntry {
//         id: 1,
//         name: String::from("guest-os-0"),
//         os_type: VmType::VmTOs,
//         memory: VmMemoryConfig {
//             num: 1,
//             region: Some(vm_region),
//         },
//         image: VmImageConfig {
//             kernel_name: Some("Image"),
//             kernel_load_ipa: 0x88080000,
//             kernel_entry_point: 0x88080000,
//             device_tree_filename: Some("qemu2.bin"),
//             device_tree_load_ipa: 0x82000000,
//             ramdisk_filename: Some("initrd.gz"),
//             ramdisk_load_ipa: 0x83000000,
//         },
//         cpu: VmCpuConfig {
//             num: 1,
//             allocate_bitmap: 0b0010,
//             master: -1,
//         },
//         vm_emu_dev_confg: Some(emu_dev_config),
//         vm_pt_dev_confg: Some(pt_dev_config),
//     }));

//     // vm2 BMA emu
//     let mut emu_dev_config: Vec<VmEmulatedDeviceConfig> = Vec::new();
//     emu_dev_config.push(VmEmulatedDeviceConfig {
//         name: String::from("vgicd"),
//         base_ipa: 0x8000000,
//         length: 0x1000,
//         irq_id: 0,
//         cfg_list: Vec::new(),
//         emu_type: EmuDeviceType::EmuDeviceTGicd,
//         mediated: false,
//     });
//     emu_dev_config.push(VmEmulatedDeviceConfig {
//         name: String::from("virtio-blk0"),
//         base_ipa: 0xa000000,
//         length: 0x1000,
//         irq_id: 32 + 0x10,
//         cfg_list: vec![DISK_PARTITION_1_START, DISK_PARTITION_1_SIZE],
//         emu_type: EmuDeviceType::EmuDeviceTVirtioBlk,
//         mediated: false,
//     });

//     // vm2 BMA passthrough
//     let mut pt_dev_config: Vec<VmPassthroughDeviceConfig> = Vec::new();
//     pt_dev_config.push(VmPassthroughDeviceConfig {
//         name: String::from("serial1"),
//         base_pa: UART_2_ADDR,
//         base_ipa: 0x9000000,
//         length: 0x1000,
//         // dma: false,
//         irq_list: vec![27],
//     });
//     pt_dev_config.push(VmPassthroughDeviceConfig {
//         name: String::from("gicc"),
//         base_pa: Platform::GICV_BASE,
//         base_ipa: 0x8010000,
//         length: 0x2000,
//         // dma: false,
//         irq_list: Vec::new(),
//     });

//     // vm2 BMA vm_region
//     let mut vm_region: Vec<VmRegion> = Vec::new();
//     vm_region.push(VmRegion {
//         ipa_start: 0x40000000,
//         length: 0x1000000,
//     });

//     // vm2 BMA config
//     vm_config.entries.push(Arc::new(VmConfigEntry {
//         id: 2,
//         name: String::from("guest-bma-0"),
//         os_type: VmType::VmTBma,
//         memory: VmMemoryConfig {
//             num: 1,
//             region: Some(vm_region),
//         },
//         image: VmImageConfig {
//             kernel_name: Some("sbma1.bin"),
//             kernel_load_ipa: 0x40080000,
//             kernel_entry_point: 0x40080000,
//             device_tree_filename: None,
//             device_tree_load_ipa: 0,
//             ramdisk_filename: None,
//             ramdisk_load_ipa: 0,
//         },
//         cpu: VmCpuConfig {
//             num: 1,
//             allocate_bitmap: 0b0100,
//             master: -1,
//         },
//         vm_emu_dev_confg: Some(emu_dev_config),
//         vm_pt_dev_confg: Some(pt_dev_config),
//     }));
// }
