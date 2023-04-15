use alloc::{vec::Vec, collections::BTreeSet};
use fdtrs::{Fdt, node::FdtNode};
use vm_fdt::{FdtWriter, Error, FdtWriterResult};

fn parse_node(node: &FdtNode, writer: &mut FdtWriter, skip_nodes: &BTreeSet<&str>) -> FdtWriterResult<()> {
    if !skip_nodes.contains(node.name) {
        let write_node = writer.begin_node(node.name)?;
        for property in node.properties() {
            writer.property(property.name, property.value)?;
        }
        for child_node in node.children() {
            parse_node(&child_node, writer, skip_nodes)?;
        }
        writer.end_node(write_node)?;
    }
    Ok(())
}

pub(super) fn parse_fdt_to_writable(dtb: *mut fdt::myctypes::c_void) -> Result<FdtWriter, Error> {
    let skip_set = if cfg!(feature = "tx2") {
        BTreeSet::from([
            "cpu@0",
            "cpu@1",
            "sdhci@3460000",
            "sdhci@3440000",
            "serial@c280000",
            "serial@3110000",
            "serial@3130000",
            "combined-uart",
            "trusty",
            "nvdisplay@15210000",
            "ramoops_carveout",
            "watchdog@30c0000",
            "denver-pmu",
        ])
    } else if cfg!(feature = "qemu") {
        BTreeSet::from([
            "platform@c000000",
            "fw-cfg@9020000",
            "memory@40000000",
            "virtio_mmio@a000000",
            "virtio_mmio@a000200",
            "virtio_mmio@a000400",
            "virtio_mmio@a000600",
            "virtio_mmio@a000800",
            "virtio_mmio@a000a00",
            "virtio_mmio@a000c00",
            "virtio_mmio@a000e00",
            "virtio_mmio@a001000",
            "virtio_mmio@a001200",
            "virtio_mmio@a001400",
            "virtio_mmio@a001600",
            "virtio_mmio@a001800",
            "virtio_mmio@a001a00",
            "virtio_mmio@a001c00",
            "virtio_mmio@a001e00",
            "virtio_mmio@a002000",
            "virtio_mmio@a002200",
            "virtio_mmio@a002400",
            "virtio_mmio@a002600",
            "virtio_mmio@a002800",
            "virtio_mmio@a002a00",
            "virtio_mmio@a002c00",
            "virtio_mmio@a002e00",
            "virtio_mmio@a003400",
            "virtio_mmio@a003600",
            "virtio_mmio@a003800",
            "virtio_mmio@a003a00",
            "virtio_mmio@a003c00",
            "virtio_mmio@a003e00",
            "gpio-keys",
            "pl061@9030000",
            "pcie@10000000",
            "pl031@9010000",
            "v2m@8020000",
            "flash@0",
        ])
    } else {
        // feature = "pi4"
        BTreeSet::new()
    };

    let dtb_read = unsafe { Fdt::from_ptr(dtb as *const _).unwrap() };
    // assert_eq!(dtb.total_size(), unsafe { fdt::fdt_size(dtb) } as usize);
    let mut writer = FdtWriter::new().unwrap();
    let root = dtb_read.root();
    writer.begin_node("")?;
    writer.property_string(
        "compatible",
        root.compatible().all().collect::<Vec<_>>().join(",").as_str(),
    )?;
    for property in root.properties() {
        writer.property(property.name, property.value)?;
    }

    for node in dtb_read.all_nodes().into_iter() {
        if node.name == "/" {
            continue;
        }
        parse_node(&node, &mut writer, &skip_set)?;
    }
    // println!("{}", writer.into());
    Ok(writer)
}
