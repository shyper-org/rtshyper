use super::{
    AddrRegions, DtbDevType, VmConfigEntry, VmCpuConfig, VmDtbDev, VmEmulatedDeviceConfig,
    VmImageConfig, VmMemoryConfig, VmPassthroughDeviceConfig, VmRegion, DEF_VM_CONFIG_TABLE,
};
use crate::board::*;
use crate::config::PassthroughRegion;
use crate::device::EmuDeviceType;
use crate::kernel::VmType;
use alloc::sync::Arc;
use alloc::vec::Vec;

// pub fn vm_num() -> usize {
//     let vm_config = DEF_VM_CONFIG_TABLE.lock();
//     let vm_num = vm_config.vm_num;
//     drop(vm_config);
//     vm_num()
// }

pub fn config_init() {
    let mut vm_config = DEF_VM_CONFIG_TABLE.lock();
    vm_config.name = Some("tx2-default");
    vm_config.vm_num = 1;

    // vm0 emu
    let mut emu_dev_config: Vec<VmEmulatedDeviceConfig> = Vec::new();
    emu_dev_config.push(VmEmulatedDeviceConfig {
        name: Some("intc@8000000"),
        base_ipa: 0x8000000,
        length: 0x1000,
        irq_id: 0,
        cfg_list: Vec::new(),
        emu_type: EmuDeviceType::EmuDeviceTGicd,
    });
    emu_dev_config.push(VmEmulatedDeviceConfig {
        name: Some("virtio_mmio@a000000"),
        base_ipa: 0xa000000,
        length: 0x1000,
        irq_id: 32 + 0x10,
        cfg_list: vec![DISK_PARTITION_1_START, DISK_PARTITION_1_SIZE],
        emu_type: EmuDeviceType::EmuDeviceTVirtioBlk,
    });
    emu_dev_config.push(VmEmulatedDeviceConfig {
        name: Some("virtio_mmio@a002000"),
        base_ipa: 0xa002000,
        length: 0x1000,
        irq_id: 32 + 0x12,
        cfg_list: vec![DISK_PARTITION_0_START, DISK_PARTITION_0_SIZE],
        emu_type: EmuDeviceType::EmuDeviceTVirtioBlk,
    });
    emu_dev_config.push(VmEmulatedDeviceConfig {
        name: Some("virtio_mmio@a001000"),
        base_ipa: 0xa001000,
        length: 0x1000,
        irq_id: 32 + 0x11,
        cfg_list: vec![0x74, 0x56, 0xaa, 0x0f, 0x47, 0xd0],
        emu_type: EmuDeviceType::EmuDeviceTVirtioNet,
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
    let mut pt_dev_config: VmPassthroughDeviceConfig = VmPassthroughDeviceConfig::default();
    pt_dev_config.regions = vec![
        PassthroughRegion {
            ipa: 0x100000,
            pa: 0x100000,
            length: 0x10000,
        },
        PassthroughRegion {
            ipa: 0x02100000,
            pa: 0x02100000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x02110000,
            pa: 0x02110000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x02120000,
            pa: 0x02120000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x02130000,
            pa: 0x02130000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x02140000,
            pa: 0x02140000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x02150000,
            pa: 0x02150000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x02160000,
            pa: 0x02160000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x02170000,
            pa: 0x02170000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x02180000,
            pa: 0x02180000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x02190000,
            pa: 0x02190000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x02200000,
            pa: 0x02200000,
            length: 0x20000,
        },
        PassthroughRegion {
            ipa: 0x02390000,
            pa: 0x02390000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x023a0000,
            pa: 0x023a0000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x023b0000,
            pa: 0x023b0000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x023c0000,
            pa: 0x023c0000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x023d0000,
            pa: 0x023d0000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x02430000,
            pa: 0x02430000,
            length: 0x15000,
        },
        PassthroughRegion {
            ipa: 0x02490000,
            pa: 0x02490000,
            length: 0x50000,
        },
        PassthroughRegion {
            ipa: 0x02600000,
            pa: 0x02600000,
            length: 0x210000,
        },
        PassthroughRegion {
            ipa: 0x02900000,
            pa: 0x02900000,
            length: 0x200000,
        },
        PassthroughRegion {
            ipa: 0x02c00000,
            pa: 0x02c00000,
            length: 0xb0000,
        },
        PassthroughRegion {
            ipa: 0x03010000,
            pa: 0x03010000,
            length: 0xe0000,
        },
        PassthroughRegion {
            ipa: 0x03110000,
            pa: 0x03110000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x03130000,
            pa: 0x03130000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x03160000,
            pa: 0x03160000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x03180000,
            pa: 0x03180000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x03190000,
            pa: 0x03190000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x031b0000,
            pa: 0x031b0000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x031c0000,
            pa: 0x031c0000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x031e0000,
            pa: 0x031e0000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x03210000,
            pa: 0x03210000,
            length: 0x10000,
        },
        PassthroughRegion {
            ipa: 0x03240000,
            pa: 0x03240000,
            length: 0x10000,
        },
        PassthroughRegion {
            ipa: 0x03280000,
            pa: 0x03280000,
            length: 0x30000,
        },
        PassthroughRegion {
            ipa: 0x03400000,
            pa: 0x03400000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x03440000,
            pa: 0x03440000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x03460000,
            pa: 0x03460000,
            length: 0x140000,
        },
        PassthroughRegion {
            ipa: 0x03500000,
            pa: 0x03500000,
            length: 0x9000,
        },
        PassthroughRegion {
            ipa: 0x03510000,
            pa: 0x03510000,
            length: 0x10000,
        },
        PassthroughRegion {
            ipa: 0x03520000,
            pa: 0x03520000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x03530000,
            pa: 0x03530000,
            length: 0x8000,
        },
        PassthroughRegion {
            ipa: 0x03538000,
            pa: 0x03538000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x03540000,
            pa: 0x03540000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x03550000,
            pa: 0x03550000,
            length: 0x9000,
        },
        PassthroughRegion {
            ipa: 0x03820000,
            pa: 0x03820000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x03830000,
            pa: 0x03830000,
            length: 0x10000,
        },
        PassthroughRegion {
            ipa: 0x03960000,
            pa: 0x03960000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x03990000,
            pa: 0x03990000,
            length: 0x10000,
        },
        PassthroughRegion {
            ipa: 0x039c0000,
            pa: 0x039c0000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x03a90000,
            pa: 0x03a90000,
            length: 0x10000,
        },
        PassthroughRegion {
            ipa: 0x03ad0000,
            pa: 0x03ad0000,
            length: 0x20000,
        },
        PassthroughRegion {
            ipa: 0x03b41000,
            pa: 0x03b41000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x03c00000,
            pa: 0x03c00000,
            length: 0xa0000,
        },
        PassthroughRegion {
            ipa: 0x08010000,
            pa: PLATFORM_GICV_BASE,
            length: 0x2000,
        },
        PassthroughRegion {
            ipa: 0x08030000,
            pa: 0x08030000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x08050000,
            pa: 0x08050000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x08060000,
            pa: 0x08060000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x08070000,
            pa: 0x08070000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x08820000,
            pa: 0x08820000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x08a1c000,
            pa: 0x08a1c000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x09010000,
            pa: 0x09010000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x09840000,
            pa: 0x09840000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x09940000,
            pa: 0x09940000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x09a40000,
            pa: 0x09a40000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x09b40000,
            pa: 0x09b40000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x0b150000,
            pa: 0x0b150000,
            length: 0x90000,
        },
        PassthroughRegion {
            ipa: 0x0b1f0000,
            pa: 0x0b1f0000,
            length: 0x50000,
        },
        PassthroughRegion {
            ipa: 0x0c150000,
            pa: 0x0c150000,
            length: 0x90000,
        },
        PassthroughRegion {
            ipa: 0x0c240000,
            pa: 0x0c240000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x0c250000,
            pa: 0x0c250000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x0c260000,
            pa: 0x0c260000,
            length: 0x10000,
        },
        PassthroughRegion {
            ipa: 0x03100000,
            pa: 0x03100000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x0c2a0000,
            pa: 0x0c2a0000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x0c2f0000,
            pa: 0x0c2f0000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x0c2f1000,
            pa: 0x0c2f1000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x0c300000,
            pa: 0x0c300000,
            length: 0x4000,
        },
        PassthroughRegion {
            ipa: 0x0c340000,
            pa: 0x0c340000,
            length: 0x10000,
        },
        PassthroughRegion {
            ipa: 0x0c360000,
            pa: 0x0c360000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x0c370000,
            pa: 0x0c370000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x0c390000,
            pa: 0x0c390000,
            length: 0x3000,
        },
        PassthroughRegion {
            ipa: 0x0d230000,
            pa: 0x0d230000,
            length: 0x1000,
        },
        PassthroughRegion {
            ipa: 0x0e000000,
            pa: 0x0e000000,
            length: 0x80000,
        },
        PassthroughRegion {
            ipa: 0x10000000,
            pa: 0x10000000,
            length: 0x1000000,
        },
        // smmu
        PassthroughRegion {
            ipa: 0x12000000,
            pa: 0x12000000,
            length: 0x1000000,
        },
        PassthroughRegion {
            ipa: 0x13e00000,
            pa: 0x13e00000,
            length: 0x20000,
        },
        PassthroughRegion {
            ipa: 0x13ec0000,
            pa: 0x13ec0000,
            length: 0x40000,
        },
        PassthroughRegion {
            ipa: 0x150c0000,
            pa: 0x150c0000,
            length: 0x80000,
        },
        PassthroughRegion {
            ipa: 0x15340000,
            pa: 0x15340000,
            length: 0x80000,
        },
        PassthroughRegion {
            ipa: 0x15480000,
            pa: 0x15480000,
            length: 0xc0000,
        },
        PassthroughRegion {
            ipa: 0x15600000,
            pa: 0x15600000,
            length: 0x40000,
        },
        PassthroughRegion {
            ipa: 0x15700000,
            pa: 0x15700000,
            length: 0x100000,
        },
        PassthroughRegion {
            ipa: 0x15810000,
            pa: 0x15810000,
            length: 0x40000,
        },
        PassthroughRegion {
            ipa: 0x17000000,
            pa: 0x17000000,
            length: 0x2000000,
        },
        PassthroughRegion {
            ipa: 0x30000000,
            pa: 0x30000000,
            length: 0x10000000,
        },
        PassthroughRegion {
            ipa: 0x40000000,
            pa: 0x40000000,
            length: 0x40000000,
        },
    ];
    // 146 is UART_INT
    pt_dev_config.irqs = vec![
        27, 32, 33, 34, 35, 36, 37, 38, 39, 40, 48, 49, 56, 57, 58, 59, 60, 62, 63, 64, 65, 67, 68,
        69, 70, 71, 72, 74, 76, 79, 82, 85, 88, 91, 92, 94, 95, 96, 97, 102, 103, 104, 105, 107,
        108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125,
        126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, UART_0_INT, 151, 152,
        153, 154, 155, 156, 157, 158, 159, 165, 166, 167, 168, 173, 174, 175, 176, 177, 178, 179,
        185, 186, 187, 190, 191, 192, 193, 194, 196, 198, 199, 202, 203, 208, 212, 218, 219, 220,
        221, 222, 223, 224, 225, 226, 227, 229, 230, 233, 234, 235, 237, 238, 242, 255, 256, 295,
        297, 315, 322, 328, 329, 330, 331, 352, 353,
    ];
    pt_dev_config.streams_ids = vec![
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 20, 21, 22, 25, 26, 27, 28,
        29, 30, 31, 32, 42, 45, 50, 51, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70,
        71,
    ];

    // vm0 vm_region
    let mut vm_region: Vec<VmRegion> = Vec::new();
    vm_region.push(VmRegion {
        ipa_start: 0x90000000,
        length: 0x60000000,
    });
    // vm_region.push(VmRegion {
    //     ipa_start: 0xf0200000,
    //     length: 0x100000000,
    // });

    // vm0 config
    vm_config.entries.push(Arc::new(VmConfigEntry {
        name: Some("privileged"),
        os_type: VmType::VmTOs,
        memory: VmMemoryConfig {
            num: 1,
            region: Some(vm_region),
        },
        image: VmImageConfig {
            kernel_name: Some("L4T"),
            kernel_load_ipa: 0x90080000,
            kernel_entry_point: 0x90080000,
            device_tree_filename: Some("virt113.bin"),
            device_tree_load_ipa: 0x90000000,
            ramdisk_filename: None,
            ramdisk_load_ipa: 0,
        },
        cpu: VmCpuConfig {
            num: 1,
            allocate_bitmap: 0b0001,
            master: -1,
        },
        vm_emu_dev_confg: Some(emu_dev_config),
        vm_pt_dev_confg: Some(pt_dev_config),
        vm_dtb_devs: None,
        cmdline: "earlycon=uart8250,mmio32,0x3100000 console=ttyS0,115200n8 root=/dev/mmcblk0p1 rw audit=0\0",
    }));

    // vm1 emu
    // let mut emu_dev_config: Vec<VmEmulatedDeviceConfig> = Vec::new();
    // emu_dev_config.push(VmEmulatedDeviceConfig {
    //     name: Some("intc@8000000"),
    //     base_ipa: 0x8000000,
    //     length: 0x1000,
    //     irq_id: 0,
    //     cfg_list: Vec::new(),
    //     emu_type: EmuDeviceType::EmuDeviceTGicd,
    // });
    // emu_dev_config.push(VmEmulatedDeviceConfig {
    //     name: Some("virtio_mmio@a000000"),
    //     base_ipa: 0xa000000,
    //     length: 0x1000,
    //     irq_id: 32 + 0x10,
    //     cfg_list: vec![DISK_PARTITION_2_START, DISK_PARTITION_2_SIZE],
    //     emu_type: EmuDeviceType::EmuDeviceTVirtioBlk,
    // });
    // emu_dev_config.push(VmEmulatedDeviceConfig {
    //     name: Some("virtio_mmio@a001000"),
    //     base_ipa: 0xa001000,
    //     length: 0x1000,
    //     irq_id: 32 + 0x11,
    //     cfg_list: vec![0x74, 0x56, 0xaa, 0x0f, 0x47, 0xd1],
    //     emu_type: EmuDeviceType::EmuDeviceTVirtioNet,
    // });

    // // vm1 passthrough
    // let mut pt_dev_config: Vec<VmPassthroughDeviceConfig> = Vec::new();
    // pt_dev_config.push(VmPassthroughDeviceConfig {
    //     name: Some("serial@3100000"),
    //     base_pa: UART_0_ADDR,
    //     base_ipa: UART_0_ADDR,
    //     length: 0x1000,
    //     irq_list: vec![UART_0_INT],
    // });
    // pt_dev_config.push(VmPassthroughDeviceConfig {
    //     name: Some("intc@8000000"),
    //     base_pa: PLATFORM_GICV_BASE,
    //     base_ipa: 0x8010000,
    //     length: 0x2000,
    //     irq_list: vec![27],
    // });

    // // vm1 vm_region
    // let mut vm_region: Vec<VmRegion> = Vec::new();
    // vm_region.push(VmRegion {
    //     ipa_start: 0x80000000,
    //     length: 0x40000000,
    // });

    // let mut vm_dtb_devs: Vec<VmDtbDev>;
    // vm_dtb_devs.push(VmDtbDev {
    //     dev_type: DtbDevType::DevGicd,
    //     irqs: vec![],
    //     addr_region: AddrRegions {
    //         ipa: 0x8000000,
    //         length: 0x1000,
    //     },
    // });
    // vm_dtb_devs.push(VmDtbDev {
    //     dev_type: DtbDevType::DevGicc,
    //     irqs: vec![],
    //     addr_region: AddrRegions {
    //         ipa: 0x8010000,
    //         length: 0x2000,
    //     },
    // });

    // // vm1 config
    // vm_config.entries.push(Arc::new(VmConfigEntry {
    //     name: Some("guest-os-0"),
    //     os_type: VmType::VmTOs,
    //     memory: VmMemoryConfig {
    //         num: 1,
    //         region: Some(vm_region),
    //     },
    //     image: VmImageConfig {
    //         kernel_name: Some("Vanilla"),
    //         kernel_load_ipa: 0x88080000,
    //         kernel_entry_point: 0x88080000,
    //         device_tree_filename: Some("virt213.bin"),
    //         device_tree_load_ipa: 0x82000000,
    //         ramdisk_filename: None,
    //         ramdisk_load_ipa: 0,
    //     },
    //     cpu: VmCpuConfig {
    //         num: 2,
    //         allocate_bitmap: 0b1100,
    //         master: -1,
    //     },
    //     vm_emu_dev_confg: Some(emu_dev_config),
    //     vm_pt_dev_confg: Some(pt_dev_config),
    // }));
}
