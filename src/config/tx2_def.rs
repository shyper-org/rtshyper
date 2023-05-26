use alloc::string::String;
use alloc::vec::Vec;

use crate::arch::INTERRUPT_IRQ_GUEST_TIMER;
use crate::board::{PlatOperation, Platform};
use crate::config::vm_cfg_add_vm_entry;
use crate::device::EmuDeviceType;
use crate::kernel::{HVC_IRQ, VmType, HYPERVISOR_COLORS};

use super::{
    PassthroughRegion, VmConfigEntry, VmCpuConfig, VMDtbDevConfigList, VmEmulatedDeviceConfig,
    VmEmulatedDeviceConfigList, VmImageConfig, VmMemoryConfig, VmPassthroughDeviceConfig, VmRegion,
};

#[rustfmt::skip]
pub fn mvm_config_init() {
    println!("mvm_config_init() init config for VM0, which is manager VM");

    // vm0 emu
    let emu_dev_config = vec![
        VmEmulatedDeviceConfig {
            name: String::from("interrupt-controller@3881000"),
            base_ipa: Platform::GICD_BASE,
            length: 0x1000,
            irq_id: 0,
            cfg_list: Vec::new(),
            emu_type: EmuDeviceType::EmuDeviceTGicd,
            mediated: false,
        },
        VmEmulatedDeviceConfig {
            name: String::from("virtio_net@a001000"),
            base_ipa: 0xa001000,
            length: 0x1000,
            irq_id: 32 + 0x100,
            cfg_list: vec![0x74, 0x56, 0xaa, 0x0f, 0x47, 0xd0],
            emu_type: EmuDeviceType::EmuDeviceTVirtioNet,
            mediated: false,
        },
        VmEmulatedDeviceConfig {
            name: String::from("virtio_console@a002000"),
            base_ipa: 0xa002000,
            length: 0x1000,
            irq_id: 32 + 0x101,
            cfg_list: vec![1, 0xa002000],
            emu_type: EmuDeviceType::EmuDeviceTVirtioConsole,
            mediated: false,
        },
        VmEmulatedDeviceConfig {
            name: String::from("virtio_console@a003000"),
            base_ipa: 0xa003000,
            length: 0x1000,
            irq_id: 32 + 0x102,
            cfg_list: vec![2, 0xa002000],
            emu_type: EmuDeviceType::EmuDeviceTVirtioConsole,
            mediated: false,
        },
        VmEmulatedDeviceConfig {
            name: String::from("iommu"),
            base_ipa: 0x12000000,
            length: 0x1000000,
            irq_id: 0,
            cfg_list: Vec::new(),
            emu_type: EmuDeviceType::EmuDeviceTIOMMU,
            mediated: false,
        },
        VmEmulatedDeviceConfig {
            name: String::from("vm_service"),
            base_ipa: 0,
            length: 0,
            irq_id: HVC_IRQ,
            cfg_list: Vec::new(),
            emu_type: EmuDeviceType::EmuDeviceTShyper,
            mediated: false,
        }
    ];

    // vm0 passthrough
    let pt_dev_config: VmPassthroughDeviceConfig = VmPassthroughDeviceConfig {
        regions: vec![
            PassthroughRegion { ipa: 0x100000, pa: 0x100000, length: 0x10000, dev_property: true },
            PassthroughRegion { ipa: 0x02100000, pa: 0x02100000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x02110000, pa: 0x02110000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x02120000, pa: 0x02120000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x02130000, pa: 0x02130000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x02140000, pa: 0x02140000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x02150000, pa: 0x02150000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x02160000, pa: 0x02160000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x02170000, pa: 0x02170000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x02180000, pa: 0x02180000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x02190000, pa: 0x02190000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x02200000, pa: 0x02200000, length: 0x20000, dev_property: true },
            PassthroughRegion { ipa: 0x02390000, pa: 0x02390000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x023a0000, pa: 0x023a0000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x023b0000, pa: 0x023b0000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x023c0000, pa: 0x023c0000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x023d0000, pa: 0x023d0000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x02430000, pa: 0x02430000, length: 0x15000, dev_property: true },
            PassthroughRegion { ipa: 0x02490000, pa: 0x02490000, length: 0x50000, dev_property: true },
            PassthroughRegion { ipa: 0x02600000, pa: 0x02600000, length: 0x210000, dev_property: true },
            PassthroughRegion { ipa: 0x02900000, pa: 0x02900000, length: 0x200000, dev_property: true },
            PassthroughRegion { ipa: 0x02c00000, pa: 0x02c00000, length: 0xb0000, dev_property: true },
            PassthroughRegion { ipa: 0x03010000, pa: 0x03010000, length: 0xe0000, dev_property: true },
            // sata
            PassthroughRegion { ipa: 0x03100000, pa: 0x03100000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x03110000, pa: 0x03110000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x03130000, pa: 0x03130000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x03160000, pa: 0x03160000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x03180000, pa: 0x03180000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x03190000, pa: 0x03190000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x031b0000, pa: 0x031b0000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x031c0000, pa: 0x031c0000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x031e0000, pa: 0x031e0000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x03210000, pa: 0x03210000, length: 0x10000, dev_property: true },
            PassthroughRegion { ipa: 0x03240000, pa: 0x03240000, length: 0x10000, dev_property: true },
            PassthroughRegion { ipa: 0x03280000, pa: 0x03280000, length: 0x30000, dev_property: true },
            PassthroughRegion { ipa: 0x03400000, pa: 0x03400000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x03440000, pa: 0x03440000, length: 0x1000, dev_property: true },
            // emmc blk
            // PassthroughRegion { ipa: 0x03460000, pa: 0x03460000, length: 0x140000 },
            PassthroughRegion { ipa: 0x03460000, pa: 0x03460000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x03500000, pa: 0x03500000, length: 0x9000, dev_property: true },
            PassthroughRegion { ipa: 0x03510000, pa: 0x03510000, length: 0x10000, dev_property: true },
            PassthroughRegion { ipa: 0x03520000, pa: 0x03520000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x03530000, pa: 0x03530000, length: 0x8000, dev_property: true },
            PassthroughRegion { ipa: 0x03538000, pa: 0x03538000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x03540000, pa: 0x03540000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x03550000, pa: 0x03550000, length: 0x9000, dev_property: true },
            PassthroughRegion { ipa: 0x03820000, pa: 0x03820000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x03830000, pa: 0x03830000, length: 0x10000, dev_property: true },
            PassthroughRegion { ipa: 0x03960000, pa: 0x03960000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x03990000, pa: 0x03990000, length: 0x10000, dev_property: true },
            PassthroughRegion { ipa: 0x039c0000, pa: 0x039c0000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x03a90000, pa: 0x03a90000, length: 0x10000, dev_property: true },
            PassthroughRegion { ipa: 0x03ad0000, pa: 0x03ad0000, length: 0x20000, dev_property: true },
            // PassthroughRegion { ipa: 0x03b41000, pa: 0x03b41000, length: 0x1000 },
            PassthroughRegion { ipa: 0x03c00000, pa: 0x03c00000, length: 0xa0000, dev_property: true },
            PassthroughRegion { ipa: Platform::GICC_BASE, pa: Platform::GICV_BASE, length: 0x2000, dev_property: true },
            PassthroughRegion { ipa: 0x8010000, pa: 0x8010000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x08030000, pa: 0x08030000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x08050000, pa: 0x08050000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x08060000, pa: 0x08060000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x08070000, pa: 0x08070000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x08820000, pa: 0x08820000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x08a1c000, pa: 0x08a1c000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x09010000, pa: 0x09010000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x09840000, pa: 0x09840000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x09940000, pa: 0x09940000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x09a40000, pa: 0x09a40000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x09b40000, pa: 0x09b40000, length: 0x1000, dev_property: true },
            // PassthroughRegion { ipa: 0x0b000000, pa: 0x0b000000, length: 0x1000 },
            // PassthroughRegion { ipa: 0x0b040000, pa: 0x0b040000, length: 0x20000},
            PassthroughRegion { ipa: 0x0b150000, pa: 0x0b150000, length: 0x90000, dev_property: true },
            PassthroughRegion { ipa: 0x0b1f0000, pa: 0x0b1f0000, length: 0x50000, dev_property: true },
            PassthroughRegion { ipa: 0x0c150000, pa: 0x0c150000, length: 0x90000, dev_property: true },
            PassthroughRegion { ipa: 0x0c240000, pa: 0x0c240000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x0c250000, pa: 0x0c250000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x0c260000, pa: 0x0c260000, length: 0x10000, dev_property: true },
            // serial
            PassthroughRegion { ipa: 0x0c280000, pa: 0x0c280000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x0c2a0000, pa: 0x0c2a0000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x0c2f0000, pa: 0x0c2f0000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x0c2f1000, pa: 0x0c2f1000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x0c300000, pa: 0x0c300000, length: 0x4000, dev_property: true },
            PassthroughRegion { ipa: 0x0c340000, pa: 0x0c340000, length: 0x10000, dev_property: true },
            PassthroughRegion { ipa: 0x0c360000, pa: 0x0c360000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x0c370000, pa: 0x0c370000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x0c390000, pa: 0x0c390000, length: 0x3000, dev_property: true },
            PassthroughRegion { ipa: 0x0d230000, pa: 0x0d230000, length: 0x1000, dev_property: true },
            PassthroughRegion { ipa: 0x0e000000, pa: 0x0e000000, length: 0x80000, dev_property: true },
            PassthroughRegion { ipa: 0x10000000, pa: 0x10000000, length: 0x1000000, dev_property: true },
            // smmu
            // PassthroughRegion { ipa: 0x12000000, pa: 0x12000000, length: 0x1000000 , dev_property: true},
            PassthroughRegion { ipa: 0x13e00000, pa: 0x13e00000, length: 0x20000, dev_property: true },
            PassthroughRegion { ipa: 0x13ec0000, pa: 0x13ec0000, length: 0x40000, dev_property: true },
            // PassthroughRegion { ipa: 0x15040000, pa: 0x15040000, length: 0x40000 },
            PassthroughRegion { ipa: 0x150c0000, pa: 0x150c0000, length: 0x80000, dev_property: true },
            // PassthroughRegion { ipa: 0x15210000, pa: 0x15210000, length: 0x10000 },
            PassthroughRegion { ipa: 0x15340000, pa: 0x15340000, length: 0x80000, dev_property: true },
            PassthroughRegion { ipa: 0x15480000, pa: 0x15480000, length: 0xc0000, dev_property: true },
            // PassthroughRegion { ipa: 0x15580000, pa: 0x15580000, length: 0x40000 },
            PassthroughRegion { ipa: 0x15600000, pa: 0x15600000, length: 0x40000, dev_property: true },
            PassthroughRegion { ipa: 0x15700000, pa: 0x15700000, length: 0x100000, dev_property: true },
            PassthroughRegion { ipa: 0x15810000, pa: 0x15810000, length: 0x40000, dev_property: true },
            PassthroughRegion { ipa: 0x17000000, pa: 0x17000000, length: 0x2000000, dev_property: true },
            PassthroughRegion { ipa: 0x30000000, pa: 0x30000000, length: 0x10000000, dev_property: true },
            PassthroughRegion { ipa: 0x40000000, pa: 0x40000000, length: 0x40000000, dev_property: true },
        ],
        // 146 is UART_INT
        irqs: vec![
            INTERRUPT_IRQ_GUEST_TIMER, 32, 33, 34, 35, 36, 37, 38, 39, 40, 48, 49, 56, 57, 58, 59, 60, 62, 63, 64, 65, 67, 68,
            69, 70, 71, 72, 74, 76, 79, 82, 85, 88, 91, 92, 94, 95, 96, 97, 102, 103, 104, 105, 107,
            108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125,
            126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, Platform::UART_0_INT, 151, 152,
            153, 154, 155, 156, 157, 158, 159, 165, 166, 167, 168, 173, 174, 175, 176, 177, 178, 179,
            185, 186, 187, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 208,
            212, 218, 219, 220, 221, 222, 223, 224, 225, 226, 227, 229, 230, 233, 234, 235, 237, 238,
            242, 255, 256, 295, 297, 315, 322, 328, 329, 330, 331, 352, 353, 366,
        ],
        streams_ids: vec![
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 20, 21, 22, 25, 26, 27, 28,
            29, 30, 31, 32, 42, 45, 50, 51, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70,
            71,
        ]
    };

    // vm0 vm_region
    let vm_region = vec![
        VmRegion {
            ipa_start: 0xa0000000,
            length: 0x60000000,
        }
    ];
    // vm_region.push(VmRegion {
    //     ipa_start: 0xf0200000,
    //     length: 0xc0000000,
    // });

    // vm0 config
    let mvm_config_entry = VmConfigEntry {
        id: 0,
        name: String::from("privileged"),
        os_type: VmType::VmTOs,
        cmdline:
        String::from("earlycon=uart8250,mmio32,0x3100000 console=ttyS0,115200n8 root=/dev/nvme0n1p1 rw audit=0 rootwait default_hugepagesz=32M hugepagesz=32M hugepages=4\0"),
        // String::from("earlycon=uart8250,mmio32,0x3100000 console=ttyS0,115200n8 root=/dev/sda1 rw audit=0 rootwait default_hugepagesz=32M hugepagesz=32M hugepages=5\0"),

        image: VmImageConfig {
            kernel_img_name: Some("L4T"),
            kernel_load_ipa: 0xa0080000,
            kernel_entry_point: 0xa0080000,
            device_tree_load_ipa: 0xa0000000,
            ramdisk_load_ipa: 0,
        },
        memory: VmMemoryConfig {
            region: vm_region,
            colors: HYPERVISOR_COLORS.get().unwrap().clone(),
        },
        cpu: VmCpuConfig {
            num: 1,
            allocate_bitmap: 0b0001,
            master: Some(0),
        },
        vm_emu_dev_confg: VmEmulatedDeviceConfigList { emu_dev_list: emu_dev_config },
        vm_pt_dev_confg: pt_dev_config,
        vm_dtb_devs: VMDtbDevConfigList::default(),
        mediated_block_index: None,
    };
    let _ = vm_cfg_add_vm_entry(mvm_config_entry);
}

#[allow(dead_code)]
pub fn unishyper_config_init() {
    let emu_dev_config = vec![
        VmEmulatedDeviceConfig {
            name: String::from("interrupt-controller@8000000"),
            base_ipa: 0x8000000,
            length: 0x1000,
            irq_id: 0,
            cfg_list: Vec::new(),
            emu_type: EmuDeviceType::EmuDeviceTGicd,
            mediated: false,
        },
        VmEmulatedDeviceConfig {
            name: String::from("virtio_blk@a000000"),
            base_ipa: 0xa000000,
            length: 0x1000,
            irq_id: 32 + 0x10,
            cfg_list: vec![0, 209715200], // 100G
            emu_type: EmuDeviceType::EmuDeviceTVirtioBlk,
            mediated: true,
        },
        VmEmulatedDeviceConfig {
            name: String::from("virtio_net@a001000"),
            base_ipa: 0xa001000,
            length: 0x1000,
            irq_id: 32 + 0x11,
            cfg_list: vec![0x74, 0x56, 0xaa, 0x0f, 0x47, 0xd1],
            emu_type: EmuDeviceType::EmuDeviceTVirtioNet,
            mediated: false,
        },
    ];

    // vm0 passthrough
    let pt_dev_config = VmPassthroughDeviceConfig {
        regions: vec![
            PassthroughRegion {
                ipa: Platform::UART_1_ADDR,
                pa: Platform::UART_1_ADDR,
                length: 0x1000,
                dev_property: true,
            },
            PassthroughRegion {
                ipa: 0x8010000,
                pa: Platform::GICV_BASE,
                length: 0x2000,
                dev_property: true,
            },
        ],
        irqs: vec![INTERRUPT_IRQ_GUEST_TIMER, Platform::UART_1_INT],
        streams_ids: vec![],
    };

    // vm0 vm_region
    let vm_region = vec![VmRegion {
        ipa_start: 0x40000000,
        length: 0x40000000,
    }];

    // vm0 config
    let mvm_config_entry = VmConfigEntry {
        id: 0,
        name: String::from("unishyper"),
        os_type: VmType::VmTOs,
        cmdline: String::from("\0"),

        image: VmImageConfig {
            kernel_img_name: Some("Image_Unishyper"),
            kernel_load_ipa: 0x40080000,
            kernel_entry_point: 0x40080000,
            device_tree_load_ipa: 0,
            ramdisk_load_ipa: 0,
        },
        memory: VmMemoryConfig {
            region: vm_region,
            colors: vec![],
        },
        cpu: VmCpuConfig {
            num: 1,
            allocate_bitmap: 0b0001,
            master: Some(0),
        },
        vm_emu_dev_confg: VmEmulatedDeviceConfigList {
            emu_dev_list: emu_dev_config,
        },
        vm_pt_dev_confg: pt_dev_config,
        vm_dtb_devs: VMDtbDevConfigList::default(),
        mediated_block_index: None,
    };
    let _ = vm_cfg_add_vm_entry(mvm_config_entry);
}
