use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use spin::Mutex;

use crate::arch::INTERRUPT_IRQ_GUEST_TIMER;
use crate::board::*;
use crate::config::vm_cfg_add_vm_entry;
use crate::device::EmuDeviceType;
use crate::kernel::{HVC_IRQ, VmType, HYPERVISOR_COLORS};

use super::{
    PassthroughRegion, vm_cfg_set_config_name, VmConfigEntry, VmCpuConfig, VMDtbDevConfigList, VmEmulatedDeviceConfig,
    VmEmulatedDeviceConfigList, VmImageConfig, VmMemoryConfig, VmPassthroughDeviceConfig, VmRegion,
};

#[rustfmt::skip]
pub fn mvm_config_init() {
    println!("mvm_config_init() init config for VM0, which is manager VM");

    vm_cfg_set_config_name("pi4-default");

    // vm0 emu
    let emu_dev_config = vec![
        VmEmulatedDeviceConfig {
            name: Some(String::from("interrupt-controller@fff841000")),
            base_ipa: 0xFFF841000,
            length: 0x1000,
            irq_id: 0,
            cfg_list: Vec::new(),
            emu_type: EmuDeviceType::EmuDeviceTGicd,
            mediated: false,
        },
        VmEmulatedDeviceConfig {
            name: Some(String::from("virtio_net@fa000800")),
            base_ipa: 0xfa000800,
            length: 0x400,
            irq_id: 32 + 0x17,
            cfg_list: vec![0x74, 0x56, 0xaa, 0x0f, 0x47, 0xd0],
            emu_type: EmuDeviceType::EmuDeviceTVirtioNet,
            mediated: false,
        },
        VmEmulatedDeviceConfig {
            name: Some(String::from("virtio_console@fa000c00")),
            base_ipa: 0xfa000c00,
            length: 0x1000,
            irq_id: 32 + 0x20,
            cfg_list: vec![1, 0xa002000],
            emu_type: EmuDeviceType::EmuDeviceTVirtioConsole,
            mediated: false,
        },
        VmEmulatedDeviceConfig {
            name: Some(String::from("virtio_console@fa002000")),
            base_ipa: 0xfa002000,
            length: 0x1000,
            irq_id: 32 + 0x18,
            cfg_list: vec![2, 0xa002000],
            emu_type: EmuDeviceType::EmuDeviceTVirtioConsole,
            mediated: false,
        },
        VmEmulatedDeviceConfig {
            name: Some(String::from("vm_service")),
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
        // all
        PassthroughRegion { ipa: 0xFC000000, pa: 0xFC000000, length: 0x04000000, dev_property: true },
        // pcie@7d500000
        PassthroughRegion { ipa: 0x600000000, pa: 0x600000000, length: 0x4000000, dev_property: true },
        // fb
        PassthroughRegion { ipa: 0x3e000000, pa: 0x3e000000, length: 0x40000000 - 0x3e000000, dev_property: false },
        // gicv
        PassthroughRegion { ipa: Platform::GICC_BASE + 0xF_0000_0000, pa: Platform::GICV_BASE, length: 0x2000, dev_property: true },
    ];
    // 146 is UART_INT
    pt_dev_config.irqs = vec![
        INTERRUPT_IRQ_GUEST_TIMER,        // timer
        32 + 0x21, // mailbox@7e00b880
        32 + 0x28, // usb@7e980000
        32 + 0x40, // timer@7e003000
        32 + 0x41, // timer@7e003000
        32 + 0x42, // timer@7e003000
        32 + 0x43, // timer@7e003000
        32 + 0x4b, // txp@7e004000
        32 + 0x7d, // rng@7e104000
        32 + 0x71, // gpio@7e200000
        32 + 0x72, // gpio@7e200000
        32 + 0x79, // serial@7e201000
        32 + 0x78, // mmc@7e202000
        32 + 0x76, // spi@7e204000
        32 + 0x64, // dsi@7e209000
        32 + 0x5d, // spi@7e215080
        32 + 0x61, // hvs@7e400000
        32 + 0x6c, // dsi@7e700000
        32 + 0x7b, // vec@7e806000
        32 + 0x49, // usb@7e980000
        32 + 0x50, // dma@7e007000
        32 + 0x51, // dma@7e007000
        32 + 0x52, // dma@7e007000
        32 + 0x53, // dma@7e007000
        32 + 0x54, // dma@7e007000
        32 + 0x55, // dma@7e007000
        32 + 0x56, // dma@7e007000
        32 + 0x57, // dma@7e007000
        32 + 0x58, // dma@7e007000
        // spi@7e204800
        // spi@7e204s00
        // spi@7e204v00
        32 + 0x75, // i2c@7e205600
        // i2c@7e205800
        // i2c@7e205a00
        // i2c@7e205c00
        32 + 0x6d, // pixelvalve@7e206000
        32 + 0x6e, // pixelvalve@7e207000
        32 + 0x65, // pixelvalve@7e20a000
        32 + 0x6a, // pixelvalve@7e20a000
        32 + 0x60, // hdmi@7ef00700
        32 + 0x22, // mailbox@7e00b840
        32 + 0x70, // smi@7e600000
        32 + 0x66, // csi@7e800000
        32 + 0x67, // csi@7e801000
        // 32 + 0x10, // arm-pmu
        // 32 + 0x11, // arm-pmu
        // 32 + 0x12, // arm-pmu
        // 32 + 0x13, // arm-pmu
        32 + 0x94, // pcie@7d500000
        32 + 0x9d, // ethernet@7d580000
        32 + 0x9e, // ethernet@7d580000
        32 + 0x59, // dma@7e007b00
        32 + 0x5a, // dma@7e007b00
        32 + 0x5b, // dma@7e007b00
        32 + 0x5c, // dma@7e007b00
        32 + 0xb0, // xhci@7e9c0000
        32 + 0x62, // rpivid-local-intc@7eb10000
    ];
    pt_dev_config.streams_ids = vec![];

    // vm0 vm_region
    let vm_region = vec![
        VmRegion {
            ipa_start: 0x200000,
            length: 0x3e000000 - 0x200000,
        }
    ];
    // vm_region.push(VmRegion {
    //     ipa_start: 0xf0200000,
    //     length: 0xc0000000,
    // });

    // vm0 config

    let mvm_config_entry = VmConfigEntry {
        id: 0,
        // name: Some("privileged"),
        name: Some(String::from("Raspi4")),
        os_type: VmType::VmTOs,
        cmdline:
        // String::from("earlycon=uart8250,mmio32,0x3100000 console=ttyS0,115200n8 root=/dev/nvme0n1p2 rw audit=0 rootwait default_hugepagesz=32M hugepagesz=32M hugepages=4\0"),
        String::from("coherent_pool=1M snd_bcm2835.enable_compat_alsa=0 snd_bcm2835.enable_hdmi=1 snd_bcm2835.enable_headphones=1 console=ttyAMA0,115200n8 root=/dev/sda1 rootfstype=ext4 rw audit=0 rootwait default_hugepagesz=32M hugepagesz=32M hugepages=4\0"),

        image: Arc::new(Mutex::new(VmImageConfig {
            kernel_img_name: Some("Raspi4"),
            kernel_load_ipa: 0x280000,
            kernel_entry_point: 0x280000,
            device_tree_load_ipa: 0x10000000,
            ramdisk_load_ipa: 0,
            mediated_block_index: None,
        })),
        memory: Arc::new(Mutex::new(VmMemoryConfig {
            region: vm_region,
            colors: HYPERVISOR_COLORS.get().unwrap().clone(),
        })),
        cpu: Arc::new(Mutex::new(VmCpuConfig {
            num: 1,
            allocate_bitmap: 0b0001,
            master: 0,
        })),
        vm_emu_dev_confg: Arc::new(Mutex::new(VmEmulatedDeviceConfigList{emu_dev_list: emu_dev_config,})),
        vm_pt_dev_confg: Arc::new(Mutex::new(pt_dev_config)),
        vm_dtb_devs: Arc::new(Mutex::new(VMDtbDevConfigList::default())),
    };
    let _ = vm_cfg_add_vm_entry(mvm_config_entry);
}
