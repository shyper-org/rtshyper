use crate::SYSTEM_FDT;
use alloc::vec::Vec;
use vm_fdt::{Error, FdtWriter};

pub fn init_vm0_dtb(dtb: *mut fdt::myctypes::c_void) {
    unsafe {
        use fdt::*;
        println!("fdt orignal size {}", fdt_size(dtb));
        fdt_pack(dtb);
        fdt_enlarge(dtb);
        let r = fdt_del_mem_rsv(dtb, 0);
        assert_eq!(r, 0);
        // fdt_add_mem_rsv(fdt, 0x80000000, 0x10000000);
        let r = fdt_clear_initrd(dtb);
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
}

// create vm1 fdt demo
fn create_fdt() -> Result<Vec<u8>, Error> {
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

    let int_controller = fdt.begin_node("interrupt-controller@0")?;
    fdt.property_u32("phandle", 0x8001)?;
    fdt.property_array_u64("reg", &[0x0, 0x1000, 0x0, 0x2000])?;
    fdt.property_string("compatible", "arm,cortex-a15-gic")?;
    fdt.property_u32("#interrupt-cells", 0x03)?;
    fdt.property_null("interrupt-controller")?;
    fdt.end_node(int_controller)?;

    let cpus = fdt.begin_node("cpus")?;
    fdt.property_u32("#size-cells", 0)?;
    fdt.property_u32("#address-cells", 0x2)?;
    fdt.end_node(cpus)?;

    let serial = fdt.begin_node("serial@0")?;
    fdt.property_string("compatible", "ns16550")?;
    fdt.property_array_u64("reg", &[0x0, 0x40])?;
    fdt.property_u32("reg-shift", 0x2)?;
    fdt.property_array_u32("interrupts", &[0x0, 0x0, 0x4])?;
    fdt.property_u32("clock-frequency", 408000000)?;
    fdt.property_string("status", "disabled")?;
    fdt.end_node(serial)?;

    let serial = fdt.begin_node("chosen")?;
    fdt.end_node(serial);

    fdt.end_node(root_node)?;
    fdt.finish()
}
