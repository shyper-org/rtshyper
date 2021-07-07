use super::{
    VmConfigEntry, VmConfigTable, VmCpuConfig, VmEmulatedDeviceConfig, VmImageConfig,
    VmMemoryConfig, VmPassthroughDeviceConfig, VmRegion, DEF_VM_CONFIG_TABLE,
};
use crate::board::*;
use crate::device::EmuDeviceType;
use crate::kernel::VmType;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

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
        cfg_list: vec![DISK_PARTITION_1_START, DISK_PARTITION_1_SIZE],
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
    // pt_dev_config.push(VmPassthroughDeviceConfig {
    //     name: Some("nic"),
    //     base_pa: 0x0a003000,
    //     base_ipa: 0x0a003000,
    //     length: 0x1000,
    //     dma: false,
    //     irq_list: vec![32 + 0x2e],
    // });

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
            num: 4,
            allocate_bitmap: 0b1111,
            master: 0,
        },
        vm_emu_dev_confg: Some(emu_dev_config),
        vm_pt_dev_confg: Some(pt_dev_config),
    }));
}
