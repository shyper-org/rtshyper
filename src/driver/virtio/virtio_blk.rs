const VIRTIO_MMIO_BASE: usize = 0x0a003000;
const QUEUE_SIZE: usize = 8;

// use crate::device::*;
use super::blk::*;
use super::mmio::*;
use super::ring::*;
use super::virtio::*;
use crate::arch::PAGE_SIZE;
use register::mmio::*;
use register::*;
use spin::Mutex;

#[repr(C)]
#[repr(align(4096))]
#[derive(Debug)]
struct VirtioRing {
    desc: [VirtioRingDesc; QUEUE_SIZE],
    driver: VirtioRingDriver,
    device: VirtioRingDevice,
}

static VIRTIO_RING: Mutex<VirtioRing> = Mutex::new(VirtioRing {
    desc: [VirtioRingDesc {
        addr: 0,
        len: 0,
        flags: 0,
        next: 0,
    }; QUEUE_SIZE],
    driver: VirtioRingDriver {
        flags: 0,
        idx: 0,
        ring: [0; QUEUE_SIZE],
    },
    device: VirtioRingDevice {
        flags: 0,
        idx: 0,
        ring: [VirtioRingDeviceElement { id: 0, len: 0 }; QUEUE_SIZE],
    },
});

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct VirtioRingDesc {
    addr: u64,
    len: u32,
    flags: u16,
    next: u16,
}

#[repr(C)]
#[derive(Debug)]
struct VirtioRingDriver {
    flags: u16,
    idx: u16,
    ring: [u16; QUEUE_SIZE],
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
struct VirtioRingDeviceElement {
    id: u32,
    len: u32,
}

#[repr(C)]
#[repr(align(4096))]
#[derive(Debug)]
struct VirtioRingDevice {
    flags: u16,
    idx: u16,
    ring: [VirtioRingDeviceElement; QUEUE_SIZE],
}

register_structs! {
  #[allow(non_snake_case)]
  VirtioMmioBlock {
    (0x000 => MagicValue: ReadOnly<u32>),
    (0x004 => Version: ReadOnly<u32>),
    (0x008 => DeviceID: ReadOnly<u32>),
    (0x00c => VendorID: ReadOnly<u32>),
    (0x010 => DeviceFeatures: ReadOnly<u32>),
    (0x014 => DeviceFeaturesSel: WriteOnly<u32>),
    (0x018 => _reserved_0),
    (0x020 => DriverFeatures: WriteOnly<u32>),
    (0x024 => DriverFeaturesSel: WriteOnly<u32>),
    (0x028 => GuestPageSize: ReadWrite<u32>),
    (0x030 => QueueSel: WriteOnly<u32>),
    (0x034 => QueueNumMax: ReadOnly<u32>),
    (0x038 => QueueNum: WriteOnly<u32>),
    (0x03c => _reserved_2),
    (0x044 => QueueReady: ReadWrite<u32>),
    (0x048 => _reserved_3),
    (0x050 => QueueNotify: WriteOnly<u32>),
    (0x054 => _reserved_4),
    (0x060 => InterruptStatus: ReadOnly<u32>),
    (0x064 => InterruptACK: WriteOnly<u32>),
    (0x068 => _reserved_5),
    (0x070 => Status: ReadWrite<u32>),
    (0x074 => _reserved_6),
    (0x080 => QueueDescLow: WriteOnly<u32>),
    (0x084 => QueueDescHigh: WriteOnly<u32>),
    (0x088 => _reserved_7),
    (0x090 => QueueDriverLow: WriteOnly<u32>),
    (0x094 => QueueDriverHigh: WriteOnly<u32>),
    (0x098 => _reserved_8),
    (0x0a0 => QueueDeviceLow: WriteOnly<u32>),
    (0x0a4 => QueueDeviceHigh: WriteOnly<u32>),
    (0x0a8 => _reserved_9),
    (0x0fc => ConfigGeneration: ReadOnly<u32>),
    (0x0fd => _reserved_10),
    (0x100 => _reserved_config),
    (0x200 => @END),
  }
}

struct VirtioMmio {
    base_addr: usize,
}

impl core::ops::Deref for VirtioMmio {
    type Target = VirtioMmioBlock;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr() }
    }
}

impl VirtioMmio {
    const fn new(base_addr: usize) -> Self {
        VirtioMmio { base_addr }
    }
    fn ptr(&self) -> *const VirtioMmioBlock {
        self.base_addr as *const _
    }
}

trait BaseAddr {
    fn base_addr_u64(&self) -> u64;
    fn base_addr_usize(&self) -> usize;
}

impl<T> BaseAddr for T {
    fn base_addr_u64(&self) -> u64 {
        self as *const T as u64
    }
    fn base_addr_usize(&self) -> usize {
        self as *const T as usize
    }
}

static VIRTIO_MMIO: VirtioMmio = VirtioMmio::new(VIRTIO_MMIO_BASE + 7 * 0x200);

fn virtio_mmio_setup_vq(index: usize) {
    let mmio = &VIRTIO_MMIO;
    mmio.QueueSel.set(index as u32);

    let num = mmio.QueueNumMax.get();
    if num == 0 {
        panic!("queue num max is zero");
    } else if num < QUEUE_SIZE as u32 {
        panic!("queue size not supported");
    }
    mmio.QueueNum.set(QUEUE_SIZE as u32);

    let ring = VIRTIO_RING.lock();

    mmio.QueueDescLow.set(ring.desc.base_addr_usize() as u32);
    mmio.QueueDescHigh
        .set((ring.desc.base_addr_usize() as u32) >> 32);
    mmio.QueueDriverLow
        .set(ring.driver.base_addr_usize() as u32);
    mmio.QueueDriverHigh
        .set((ring.driver.base_addr_usize() as u32) >> 32);
    mmio.QueueDeviceLow
        .set(ring.device.base_addr_usize() as u32);
    mmio.QueueDeviceHigh
        .set((ring.device.base_addr_usize() as u32) >> 32);

    mmio.QueueReady.set(1);
}

pub fn virtio_blk_init() {
    let mmio = &VIRTIO_MMIO;
    if mmio.MagicValue.get() != 0x74726976
        || mmio.Version.get() != 1
        || mmio.DeviceID.get() != 2
        || mmio.VendorID.get() != 0x554d4551
    {
        panic!("could not find virtio blk")
    }

    let mut status = VIRTIO_CONFIG_S_ACKNOWLEDGE as u32;
    mmio.Status.set(status);
    status |= VIRTIO_CONFIG_S_DRIVER as u32;
    mmio.Status.set(status);

    let mut feature = mmio.DeviceFeatures.get();
    feature &= !(1 << VIRTIO_BLK_F_RO);
    feature &= !(1 << VIRTIO_BLK_F_SCSI);
    feature &= !(1 << VIRTIO_BLK_F_CONFIG_WCE);
    feature &= !(1 << VIRTIO_BLK_F_MQ);
    feature &= !(1 << VIRTIO_F_ANY_LAYOUT);
    feature &= !(1 << VIRTIO_RING_F_EVENT_IDX);
    feature &= !(1 << VIRTIO_RING_F_INDIRECT_DESC);
    mmio.DriverFeatures.set(feature);

    status |= VIRTIO_CONFIG_S_FEATURES_OK as u32;
    mmio.Status.set(status);

    status |= VIRTIO_CONFIG_S_DRIVER_OK as u32;
    mmio.Status.set(status);
    mmio.GuestPageSize.set(PAGE_SIZE as u32);

    virtio_mmio_setup_vq(0);
}
