const VIRTIO_MMIO_BASE: usize = 0x0a003000;
const QUEUE_SIZE: usize = 8;

// use crate::device::*;
use super::blk::*;
use super::mmio::*;
use super::ring::*;
use super::virtio::*;
use crate::arch::PAGE_SIZE;
use crate::kernel::{readl, writel};

fn virtio_mmio_setup_vq(index: usize) {
    let base: usize = VIRTIO_MMIO_BASE + 7 * 0x200;

    writel(index as u32, base + VIRTIO_MMIO_QUEUE_SEL);
    if readl(base + VIRTIO_MMIO_QUEUE_SEL) != 0 {
        panic!("queue already set up");
    }

    let num = readl(base + VIRTIO_MMIO_QUEUE_NUM_MAX);
    if num == 0 {
        panic!("queue num max is zero");
    } else if num < QUEUE_SIZE as u32 {
        panic!("queue size not supported");
    }
    writel(QUEUE_SIZE as u32, base + VIRTIO_MMIO_QUEUE_NUM);
    // TODO: disk struct init
}

pub fn virtio_blk_init() {
    let base: usize = VIRTIO_MMIO_BASE + 7 * 0x200;

    if (readl(base + VIRTIO_MMIO_MAGIC_VALUE) != 0x74726976
        || readl(base + VIRTIO_MMIO_VERSION) != 1
        || readl(base + VIRTIO_MMIO_DEVICE_ID) != 2
        || readl(base + VIRTIO_MMIO_VENDOR_ID) != 0x554d4551)
    {
        panic!("could not find virtio blk")
    }

    let mut s = VIRTIO_CONFIG_S_ACKNOWLEDGE as u32;
    writel(s, base + VIRTIO_MMIO_STATUS);
    s |= VIRTIO_CONFIG_S_DRIVER as u32;
    writel(s, base + VIRTIO_MMIO_STATUS);

    let mut feature = readl(base + VIRTIO_MMIO_DEVICE_FEATURES);
    feature &= !(1 << VIRTIO_BLK_F_RO);
    feature &= !(1 << VIRTIO_BLK_F_SCSI);
    feature &= !(1 << VIRTIO_BLK_F_CONFIG_WCE);
    feature &= !(1 << VIRTIO_BLK_F_MQ);
    feature &= !(1 << VIRTIO_F_ANY_LAYOUT);
    feature &= !(1 << VIRTIO_RING_F_EVENT_IDX);
    feature &= !(1 << VIRTIO_RING_F_INDIRECT_DESC);
    writel(feature, base + VIRTIO_MMIO_DRIVER_FEATURES);

    s |= VIRTIO_CONFIG_S_FEATURES_OK as u32;
    writel(s, base + VIRTIO_MMIO_STATUS);

    s |= VIRTIO_CONFIG_S_DRIVER_OK as u32;
    writel(s, base + VIRTIO_MMIO_STATUS);

    writel(PAGE_SIZE as u32, base + VIRTIO_MMIO_GUEST_PAGE_SIZE);
    virtio_mmio_setup_vq(0);
}
