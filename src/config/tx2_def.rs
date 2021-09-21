use super::{
    VmConfigEntry, VmCpuConfig, VmEmulatedDeviceConfig, VmImageConfig, VmMemoryConfig,
    VmPassthroughDeviceConfig, VmRegion, DEF_VM_CONFIG_TABLE,
};
use crate::board::*;
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
        name: Some("serial@c280000"),
        base_pa: UART_1_ADDR,
        base_ipa: UART_1_ADDR,
        length: 0x1000,
        irq_list: vec![UART_1_INT],
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("intc@8000000"),
        base_pa: PLATFORM_GICV_BASE,
        base_ipa: 0x8010000,
        length: 0x2000,
        irq_list: vec![27],
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("clock@5000000"),
        base_pa: 0x5000000,
        base_ipa: 0x5000000,
        length: 0x1000000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("tegra-hsp@b150000"),
        base_pa: 0xb150000,
        base_ipa: 0xb150000,
        length: 0x90000,
        irq_list: vec![32 + 0x8d, 32 + 0x8e, 32 + 0x8f, 32 + 0x90],
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("tegra-hsp@c150000"),
        base_pa: 0xc150000,
        base_ipa: 0xc150000,
        length: 0x90000,
        irq_list: vec![32 + 0x85, 32 + 0x86, 32 + 0x87, 32 + 0x88],
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("tegra-hsp@3c00000"),
        base_pa: 0x3c00000,
        base_ipa: 0x3c00000,
        length: 0xa0000,
        irq_list: vec![
            32 + 0xb0,
            32 + 0x78,
            32 + 0x79,
            32 + 0x7a,
            32 + 0x7b,
            32 + 0x7c,
            32 + 0x7d,
            32 + 0x7e,
            32 + 0x7f,
        ],
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("miscreg@100000"),
        base_pa: 0x100000,
        base_ipa: 0x100000,
        length: 0x10000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("efuse@3820000"),
        base_pa: 0x3820000,
        base_ipa: 0x3820000,
        length: 0x1000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("bpmp#1"),
        base_pa: 0xd000000,
        base_ipa: 0xd000000,
        length: 0x800000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("bpmp#2"),
        base_pa: 0x3004e000,
        base_ipa: 0x3004e000,
        length: 0x1000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("bpmp#3"),
        base_pa: 0x3004f000,
        base_ipa: 0x3004f000,
        length: 0x1000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("pinmux@2430000#1"),
        base_pa: 0x2430000,
        base_ipa: 0x2430000,
        length: 0x15000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("pinmux@2430000#2"),
        base_pa: 0xc300000,
        base_ipa: 0xc300000,
        length: 0x4000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("gpio@2200000"),
        base_pa: 0x2200000,
        base_ipa: 0x2200000,
        length: 0x20000,
        irq_list: vec![32 + 47, 32 + 50, 32 + 53, 32 + 56, 32 + 59, 32 + 180],
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("ether_qos@2490000"),
        base_pa: 0x2490000,
        base_ipa: 0x2490000,
        length: 0x10000,
        irq_list: vec![
            32 + 194,
            32 + 195,
            32 + 190,
            32 + 186,
            32 + 191,
            32 + 187,
            32 + 192,
            32 + 188,
            32 + 193,
            32 + 189,
        ],
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("host1x#1"),
        base_pa: 0x13e10000,
        base_ipa: 0x13e10000,
        length: 0x10000,
        irq_list: vec![32 + 0x109, 32 + 0x107],
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("host1x#2"),
        base_pa: 0x13e00000,
        base_ipa: 0x13e00000,
        length: 0x10000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("host1x#3"),
        base_pa: 0x13ec0000,
        base_ipa: 0x13ec0000,
        length: 0x40000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("nvcsi@150c0000"),
        base_pa: 0x150c0000,
        base_ipa: 0x150c0000,
        length: 0x40000,
        irq_list: vec![32 + 0x77],
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("vi@15700000"),
        base_pa: 0x15700000,
        base_ipa: 0x15700000,
        length: 0x100000,
        irq_list: vec![32 + 0xc9, 32 + 0xca, 32 + 0xcb],
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("isp@15600000"),
        base_pa: 0x15600000,
        base_ipa: 0x15600000,
        length: 0x40000,
        irq_list: vec![32 + 0xcd],
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("dc_common"),
        base_pa: 0x15200000,
        base_ipa: 0x15200000,
        length: 0x30000,
        irq_list: vec![32 + 0x99, 32 + 0x9a, 32 + 0x9b],
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("dsi#1"),
        base_pa: 0x15300000,
        base_ipa: 0x15300000,
        length: 0x40000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("dsi#2"),
        base_pa: 0x15400000,
        base_ipa: 0x15400000,
        length: 0x40000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("dsi#3"),
        base_pa: 0x15900000,
        base_ipa: 0x15900000,
        length: 0x40000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("dsi#4"),
        base_pa: 0x15940000,
        base_ipa: 0x15940000,
        length: 0x40000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("dsi#5"),
        base_pa: 0x15880000,
        base_ipa: 0x15880000,
        length: 0x10000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("vic@15340000"),
        base_pa: 0x15340000,
        base_ipa: 0x15340000,
        length: 0x40000,
        irq_list: vec![32 + 0xce],
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("nvenc@154c0000"),
        base_pa: 0x154c0000,
        base_ipa: 0x154c0000,
        length: 0x40000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("nvdec@15480000"),
        base_pa: 0x15480000,
        base_ipa: 0x15480000,
        length: 0x40000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("nvjpg@15380000"),
        base_pa: 0x15380000,
        base_ipa: 0x15380000,
        length: 0x40000,
        irq_list: vec![32 + 0xc6],
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("tsec@15500000"),
        base_pa: 0x15500000,
        base_ipa: 0x15500000,
        length: 0x40000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("tsecb@15100000"),
        base_pa: 0x15100000,
        base_ipa: 0x15100000,
        length: 0x40000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("sor"),
        base_pa: 0x15540000,
        base_ipa: 0x15540000,
        length: 0x40000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("sor1"),
        base_pa: 0x15580000,
        base_ipa: 0x15580000,
        length: 0x40000,
        irq_list: vec![32 + 0x9e],
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("dpaux@155c0000"),
        base_pa: 0x155c0000,
        base_ipa: 0x155c0000,
        length: 0x40000,
        irq_list: vec![32 + 0x9f],
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("dpaux@15040000"),
        base_pa: 0x15040000,
        base_ipa: 0x15040000,
        length: 0x40000,
        irq_list: vec![32 + 0xa0],
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("se@15810000"),
        base_pa: 0x15810000,
        base_ipa: 0x15810000,
        length: 0x10000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("se@15820000"),
        base_pa: 0x15820000,
        base_ipa: 0x15820000,
        length: 0x10000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("se@15830000"),
        base_pa: 0x15830000,
        base_ipa: 0x15830000,
        length: 0x10000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("se@15840000"),
        base_pa: 0x15840000,
        base_ipa: 0x15840000,
        length: 0x10000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("dma@2600000"),
        base_pa: 0x2600000,
        base_ipa: 0x2600000,
        length: 0x210000,
        irq_list: vec![
            32 + 0x4b,
            32 + 0x4c,
            32 + 0x4d,
            32 + 0x4e,
            32 + 0x4f,
            32 + 0x50,
            32 + 0x51,
            32 + 0x52,
            32 + 0x53,
            32 + 0x54,
            32 + 0x55,
            32 + 0x56,
            32 + 0x57,
            32 + 0x58,
            32 + 0x59,
            32 + 0x5a,
            32 + 0x5b,
            32 + 0x5c,
            32 + 0x5d,
            32 + 0x5e,
            32 + 0x5f,
            32 + 0x60,
            32 + 0x61,
            32 + 0x62,
            32 + 0x63,
            32 + 0x64,
            32 + 0x65,
            32 + 0x66,
            32 + 0x67,
            32 + 0x68,
            32 + 0x69,
            32 + 0x6a,
            32 + 0x6b,
        ],
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("i2c@3180000"),
        base_pa: 0x3180000,
        base_ipa: 0x3180000,
        length: 0x1000,
        irq_list: vec![32 + 0x1b],
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("gpio@c2f0000"),
        base_pa: 0xc2f0000,
        base_ipa: 0xc2f0000,
        length: 0x2000,
        irq_list: vec![32 + 0x3c],
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("i2c@3160000"),
        base_pa: 0x3160000,
        base_ipa: 0x3160000,
        length: 0x1000,
        irq_list: vec![32 + 0x19],
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("pwm@3280000"),
        base_pa: 0x3280000,
        base_ipa: 0x3280000,
        length: 0x10000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("pmc@c360000#1"),
        base_pa: 0xc360000,
        base_ipa: 0xc360000,
        length: 0x1000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("pmc@c360000#2"),
        base_pa: 0xc390000,
        base_ipa: 0xc390000,
        length: 0x3000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("i2c@31b0000"),
        base_pa: 0x31b0000,
        base_ipa: 0x31b0000,
        length: 0x1000,
        irq_list: vec![32 + 0x1e],
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("i2c@3190000"),
        base_pa: 0x3190000,
        base_ipa: 0x3190000,
        length: 0x1000,
        irq_list: vec![32 + 0x1c],
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("gp10b#1"),
        base_pa: 0x17000000,
        base_ipa: 0x17000000,
        length: 0x2000000,
        irq_list: vec![32 + 0x46, 32 + 0x47],
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("gp10b#2"),
        base_pa: 0x3b41000,
        base_ipa: 0x3b41000,
        length: 0x1000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("axi2apb@2390000"),
        base_pa: 0x2390000,
        base_ipa: 0x2390000,
        length: 0x1000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("axi2apb@23a0000"),
        base_pa: 0x23a0000,
        base_ipa: 0x23a0000,
        length: 0x1000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("axi2apb@23b0000"),
        base_pa: 0x23b0000,
        base_ipa: 0x23b0000,
        length: 0x1000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("axi2apb@23c0000"),
        base_pa: 0x23c0000,
        base_ipa: 0x23c0000,
        length: 0x1000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("axi2apb@23d0000"),
        base_pa: 0x23d0000,
        base_ipa: 0x23d0000,
        length: 0x1000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("axi2apb@2100000"),
        base_pa: 0x2100000,
        base_ipa: 0x2100000,
        length: 0x1000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("axi2apb@2110000"),
        base_pa: 0x2110000,
        base_ipa: 0x2110000,
        length: 0x1000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("axi2apb@2120000"),
        base_pa: 0x2120000,
        base_ipa: 0x2120000,
        length: 0x1000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("axi2apb@2130000"),
        base_pa: 0x2130000,
        base_ipa: 0x2130000,
        length: 0x1000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("axi2apb@2140000"),
        base_pa: 0x2140000,
        base_ipa: 0x2140000,
        length: 0x1000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("axi2apb@2150000"),
        base_pa: 0x2150000,
        base_ipa: 0x2150000,
        length: 0x1000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("axi2apb@2160000"),
        base_pa: 0x2160000,
        base_ipa: 0x2160000,
        length: 0x1000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("axi2apb@2170000"),
        base_pa: 0x2170000,
        base_ipa: 0x2170000,
        length: 0x1000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("axi2apb@2180000"),
        base_pa: 0x2180000,
        base_ipa: 0x2180000,
        length: 0x1000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("axi2apb@2190000"),
        base_pa: 0x2190000,
        base_ipa: 0x2190000,
        length: 0x1000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("mc"),
        base_pa: 0x2c00000,
        base_ipa: 0x2c00000,
        length: 0xb0000,
        irq_list: vec![32 + 223, 32 + 224],
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("iommu@12000000"),
        base_pa: 0x12000000,
        base_ipa: 0x12000000,
        length: 0x1000000,
        irq_list: vec![32 + 0xaa, 32 + 0xab],
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("iommu@12000000#suspend-save-reg"),
        base_pa: 0xc390000,
        base_ipa: 0xc390000,
        length: 0x1000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("xhci@3530000"),
        base_pa: 0x3530000,
        base_ipa: 0x3530000,
        length: 0x9000,
        irq_list: vec![32 + 0xa3, 32 + 0xa4, 32 + 0xa7],
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("xusb_padctl@3520000#1"),
        base_pa: 0x3520000,
        base_ipa: 0x3520000,
        length: 0x1000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("xusb_padctl@3520000#2"),
        base_pa: 0x3540000,
        base_ipa: 0x3540000,
        length: 0x1000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("cpufreq@e070000"),
        base_pa: 0xe000000,
        base_ipa: 0xe000000,
        length: 0x80000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("cluster_clk_priv@e090000#1"),
        base_pa: 0xe090000,
        base_ipa: 0xe090000,
        length: 0x10000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("cluster_clk_priv@e090000#2"),
        base_pa: 0xe0a0000,
        base_ipa: 0xe0a0000,
        length: 0x10000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("cluster_clk_priv@e090000#3"),
        base_pa: 0xe0b0000,
        base_ipa: 0xe0b0000,
        length: 0x10000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("cluster_clk_priv@e090000#4"),
        base_pa: 0xe0c0000,
        base_ipa: 0xe0c0000,
        length: 0x10000,
        irq_list: Vec::new(),
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("cluster_clk_priv@e090000#5"),
        base_pa: 0xe0d0000,
        base_ipa: 0xe0d0000,
        length: 0x10000,
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
        ipa_start: 0x90000000,
        length: 0x60000000,
    });
    vm_region.push(VmRegion {
        ipa_start: 0xf0200000,
        length: 0x100000000,
    });

    // vm0 config
    vm_config.entries.push(Arc::new(VmConfigEntry {
        name: Some("privileged"),
        os_type: VmType::VmTOs,
        memory: VmMemoryConfig {
            num: 2,
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
    }));

    // vm1 emu
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
        cfg_list: vec![DISK_PARTITION_2_START, DISK_PARTITION_2_SIZE],
        emu_type: EmuDeviceType::EmuDeviceTVirtioBlk,
    });
    emu_dev_config.push(VmEmulatedDeviceConfig {
        name: Some("virtio_mmio@a000000"),
        base_ipa: 0xa001000,
        length: 0x1000,
        irq_id: 32 + 0x11,
        cfg_list: vec![0x74, 0x56, 0xaa, 0x0f, 0x47, 0xd0],
        emu_type: EmuDeviceType::EmuDeviceTVirtioNet,
    });

    // vm1 passthrough
    let mut pt_dev_config: Vec<VmPassthroughDeviceConfig> = Vec::new();
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("serial@3100000"),
        base_pa: UART_0_ADDR,
        base_ipa: UART_0_ADDR,
        length: 0x1000,
        irq_list: vec![UART_0_INT],
    });
    pt_dev_config.push(VmPassthroughDeviceConfig {
        name: Some("intc@8000000"),
        base_pa: PLATFORM_GICV_BASE,
        base_ipa: 0x8010000,
        length: 0x2000,
        irq_list: vec![27],
    });

    // vm1 vm_region
    let mut vm_region: Vec<VmRegion> = Vec::new();
    vm_region.push(VmRegion {
        ipa_start: 0x80000000,
        length: 0x40000000,
    });

    // vm1 config
    vm_config.entries.push(Arc::new(VmConfigEntry {
        name: Some("guest-os-0"),
        os_type: VmType::VmTOs,
        memory: VmMemoryConfig {
            num: 1,
            region: Some(vm_region),
        },
        image: VmImageConfig {
            kernel_name: Some("Vanilla"),
            kernel_load_ipa: 0x88080000,
            kernel_entry_point: 0x88080000,
            device_tree_filename: Some("virt213.bin"),
            device_tree_load_ipa: 0x82000000,
            ramdisk_filename: None,
            ramdisk_load_ipa: 0,
        },
        cpu: VmCpuConfig {
            num: 2,
            allocate_bitmap: 0b1100,
            master: -1,
        },
        vm_emu_dev_confg: Some(emu_dev_config),
        vm_pt_dev_confg: Some(pt_dev_config),
    }));
}
