use alloc::vec::Vec;

use vm_fdt::{Error, FdtWriter, FdtWriterResult};

use crate::board::PLAT_DESC;
use crate::config::{DtbDevType, VmDtbDevConfig};
use crate::config::VmConfigEntry;
use crate::device::EmuDeviceType;
use crate::lib::{bit_num, round_up};
use crate::SYSTEM_FDT;
use crate::vmm::CPIO_RAMDISK;

const PI4_DTB_ADDR: usize = 0xf0000000;

pub fn init_vm0_dtb(dtb: *mut fdt::myctypes::c_void) {
    #[cfg(feature = "tx2")]
    unsafe {
        use fdt::*;
        println!("fdt orignal size {}", fdt_size(dtb));
        fdt_pack(dtb);
        fdt_enlarge(dtb);
        let r = fdt_del_mem_rsv(dtb, 0);
        assert_eq!(r, 0);
        // fdt_add_mem_rsv(fdt, 0x80000000, 0x10000000);
        fdt_clear_initrd(dtb);
        let r = fdt_remove_node(dtb, "/cpus/cpu-map/cluster0/core0\0".as_ptr());
        assert_eq!(r, 0);
        let r = fdt_remove_node(dtb, "/cpus/cpu-map/cluster0/core1\0".as_ptr());
        assert_eq!(r, 0);
        let r = fdt_disable_node(dtb, "/cpus/cpu@0\0".as_ptr());
        assert_eq!(r, 0);
        let r = fdt_disable_node(dtb, "/cpus/cpu@1\0".as_ptr());
        assert_eq!(r, 0);
        let r = fdt_disable_node(dtb, "/sdhci@3460000\0".as_ptr());
        assert_eq!(r, 0);
        // fdt_disable_node(dtb, "iommu@12000000\0".as_ptr());
        // assert_eq!(r, 0);
        let r = fdt_disable_node(dtb, "/sdhci@3440000\0".as_ptr());
        assert_eq!(r, 0);
        let r = fdt_disable_node(dtb, "/serial@c280000\0".as_ptr());
        assert_eq!(r, 0);
        let r = fdt_disable_node(dtb, "/serial@3110000\0".as_ptr());
        assert_eq!(r, 0);
        let r = fdt_disable_node(dtb, "/serial@3130000\0".as_ptr());
        assert_eq!(r, 0);
        let r = fdt_disable_node(dtb, "/combined-uart\0".as_ptr());
        assert_eq!(r, 0);
        let r = fdt_disable_node(dtb, "/trusty\0".as_ptr());
        assert_eq!(r, 0);
        let r = fdt_disable_node(dtb, "/host1x/nvdisplay@15210000\0".as_ptr());
        assert_eq!(r, 0);
        let r = fdt_disable_node(dtb, "/reserved-memory/ramoops_carveout\0".as_ptr());
        assert_eq!(r, 0);
        let r = fdt_disable_node(dtb, "/watchdog@30c0000\0".as_ptr());
        assert_eq!(r, 0);
        let len = fdt_size(dtb);
        println!("fdt after patched size {}", len);
        let slice = core::slice::from_raw_parts(dtb as *const u8, len as usize);

        SYSTEM_FDT.call_once(|| slice.to_vec());
    }
    #[cfg(feature = "pi4")]
    unsafe {
        use fdt::*;
        let pi_fdt = PI4_DTB_ADDR as *mut fdt::myctypes::c_void;
        let len = round_up(fdt_size(pi_fdt) as usize, PAGE_SIZE) + PAGE_SIZE;
        println!("fdt orignal size {}", len);
        let slice = core::slice::from_raw_parts(pi_fdt as *const u8, len as usize);
        SYSTEM_FDT.call_once(|| slice.to_vec());
    }
}

// create vm1 fdt demo
pub fn create_fdt(config: VmConfigEntry) -> Result<Vec<u8>, Error> {
    let mut fdt = FdtWriter::new()?;

    let root_node = fdt.begin_node("root")?;
    fdt.property_string("compatible", "linux,dummy-virt")?;
    fdt.property_u32("#address-cells", 0x2)?;
    fdt.property_u32("#size-cells", 0x2)?;
    fdt.property_u32("interrupt-parent", 0x8001)?;

    let psci = fdt.begin_node("psci")?;
    fdt.property_string("compatible", "arm,psci-1.0")?;
    fdt.property_string("method", "smc")?;
    fdt.property_array_u32("interrupts", &[0x1, 0x7, 0x4])?;
    fdt.end_node(psci)?;

    create_memory_node(&mut fdt, config.clone())?;
    create_timer_node(&mut fdt, 0x8)?;
    // todo: fix create_chosen_node size
    create_chosen_node(&mut fdt, &config.cmdline, config.ramdisk_load_ipa(), CPIO_RAMDISK.len())?;
    create_cpu_node(&mut fdt, config.clone())?;
    if config.dtb_device_list().len() > 0 {
        create_serial_node(&mut fdt, &config.dtb_device_list())?;
    }
    // match &config.vm_dtb_devs {
    //     Some(vm_dtb_devs) => {
    //         create_serial_node(&mut fdt, vm_dtb_devs)?;
    //     }
    //     None => {}
    // }
    create_gic_node(&mut fdt, config.gicc_addr(), config.gicd_addr())?;

    for emu_cfg in config.emulated_device_list() {
        match emu_cfg.emu_type {
            EmuDeviceType::EmuDeviceTVirtioBlk
            | EmuDeviceType::EmuDeviceTVirtioNet
            | EmuDeviceType::EmuDeviceTVirtioConsole => {
                println!(
                    "virtio fdt node init {} {:x}",
                    emu_cfg.name.as_ref().unwrap(),
                    emu_cfg.base_ipa
                );
                create_virtio_node(&mut fdt, &emu_cfg.name.unwrap(), emu_cfg.irq_id, emu_cfg.base_ipa)?;
            }
            EmuDeviceType::EmuDeviceTShyper => {
                println!("shyper fdt node init {:x}", emu_cfg.base_ipa);
                create_shyper_node(
                    &mut fdt,
                    &emu_cfg.name.unwrap(),
                    emu_cfg.irq_id,
                    emu_cfg.base_ipa,
                    emu_cfg.length,
                )?;
            }
            _ => {}
        }
    }

    fdt.end_node(root_node)?;
    fdt.finish()
}

// hard code for tx2 vm1
fn create_memory_node(fdt: &mut FdtWriter, config: VmConfigEntry) -> FdtWriterResult<()> {
    if config.memory_region().len() == 0 {
        panic!("create_memory_node memory region num 0");
    }
    let memory_name = format!("memory@{:x}", config.memory_region()[0].ipa_start);
    let memory = fdt.begin_node(&memory_name)?;
    fdt.property_string("device_type", "memory")?;
    let mut addr = vec![];
    for region in config.memory_region() {
        addr.push(region.ipa_start as u64);
        addr.push(region.length as u64);
    }
    fdt.property_array_u64("reg", addr.as_slice())?;
    fdt.end_node(memory)?;
    Ok(())
}

fn create_timer_node(fdt: &mut FdtWriter, trigger_lvl: u32) -> FdtWriterResult<()> {
    let timer = fdt.begin_node("timer")?;
    fdt.property_string("compatible", "arm,armv8-timer")?;
    fdt.property_array_u32(
        "interrupts",
        &[
            0x1,
            0xd,
            trigger_lvl,
            0x1,
            0xe,
            trigger_lvl,
            0x1,
            0xb,
            trigger_lvl,
            0x1,
            0xa,
            trigger_lvl,
        ],
    )?;
    fdt.end_node(timer)?;
    Ok(())
}

fn create_cpu_node(fdt: &mut FdtWriter, config: VmConfigEntry) -> FdtWriterResult<()> {
    let cpus = fdt.begin_node("cpus")?;
    fdt.property_u32("#size-cells", 0)?;
    fdt.property_u32("#address-cells", 0x2)?;

    let cpu_num = bit_num(config.cpu_allocated_bitmap() as usize, PLAT_DESC.cpu_desc.num);
    for cpu_id in 0..cpu_num {
        let cpu_name = format!("cpu@{:x}", cpu_id);
        let cpu_node = fdt.begin_node(&cpu_name)?;
        fdt.property_string("compatible", "arm,cortex-a57")?;
        fdt.property_string("device_type", "cpu")?;
        fdt.property_string("enable-method", "psci")?;
        fdt.property_array_u32("reg", &[0, cpu_id as u32])?;
        fdt.end_node(cpu_node)?;
    }

    fdt.end_node(cpus)?;

    Ok(())
}

fn create_serial_node(fdt: &mut FdtWriter, devs_config: &Vec<VmDtbDevConfig>) -> FdtWriterResult<()> {
    for dev in devs_config {
        match dev.dev_type {
            DtbDevType::DevSerial => {
                let serial_name = format!("serial@{:x}", dev.addr_region.ipa);
                let serial = fdt.begin_node(&serial_name)?;
                fdt.property_string("compatible", "ns16550")?;
                fdt.property_array_u64("reg", &[dev.addr_region.ipa as u64, 0x1000])?;
                fdt.property_u32("reg-shift", 0x2)?;
                fdt.property_array_u32("interrupts", &[0x0, (dev.irqs[0] - 32) as u32, 0x4])?;
                fdt.property_u32("clock-frequency", 408000000)?;
                // fdt.property_string("status", "disabled")?;
                fdt.end_node(serial)?;
            }
            _ => {}
        }
    }

    Ok(())
}

fn create_chosen_node(fdt: &mut FdtWriter, cmdline: &str, ipa: usize, size: usize) -> FdtWriterResult<()> {
    let chosen = fdt.begin_node("chosen")?;
    fdt.property_string("bootargs", cmdline)?;
    fdt.property_u32("linux,initrd-start", ipa as u32)?;
    fdt.property_u32("linux,initrd-end", (ipa + size) as u32)?;
    fdt.end_node(chosen)?;
    Ok(())
}

fn create_gic_node(fdt: &mut FdtWriter, gicc_addr: usize, gicd_addr: usize) -> FdtWriterResult<()> {
    let gic_name = format!("interrupt-controller@{:x}", gicd_addr);
    let gic = fdt.begin_node(&gic_name)?;

    fdt.property_u32("phandle", 0x8001)?;
    fdt.property_array_u64("reg", &[gicd_addr as u64, 0x1000, gicc_addr as u64, 0x2000])?;
    fdt.property_string("compatible", "arm,gic-400")?;
    fdt.property_u32("#interrupt-cells", 0x03)?;
    fdt.property_null("interrupt-controller")?;
    fdt.end_node(gic)?;

    Ok(())
}

fn create_virtio_node(fdt: &mut FdtWriter, name: &str, irq: usize, address: usize) -> FdtWriterResult<()> {
    let virtio = fdt.begin_node(name)?;
    fdt.property_null("dma-coherent")?;
    fdt.property_string("compatible", "virtio,mmio")?;
    fdt.property_array_u32("interrupts", &[0, irq as u32 - 32, 0x1])?;
    fdt.property_array_u64("reg", &[address as u64, 0x400])?;
    fdt.end_node(virtio)?;

    Ok(())
}

fn create_shyper_node(fdt: &mut FdtWriter, name: &str, irq: usize, address: usize, len: usize) -> FdtWriterResult<()> {
    let shyper = fdt.begin_node(name)?;
    fdt.property_string("compatible", "shyper")?;
    fdt.property_array_u32("interrupts", &[0, irq as u32 - 32, 0x1])?;
    if address != 0 && len != 0 {
        fdt.property_array_u64("reg", &[address as u64, len as u64])?;
    }
    fdt.end_node(shyper)?;

    Ok(())
}
