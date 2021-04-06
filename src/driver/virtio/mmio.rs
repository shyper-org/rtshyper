pub const VIRTIO_MMIO_MAGIC_VALUE: usize = 0x000;
pub const VIRTIO_MMIO_VERSION: usize = 0x004;
pub const VIRTIO_MMIO_DEVICE_ID: usize = 0x008;
pub const VIRTIO_MMIO_VENDOR_ID: usize = 0x00c;
pub const VIRTIO_MMIO_DEVICE_FEATURES: usize = 0x010;

pub const VIRTIO_MMIO_DRIVER_FEATURES: usize = 0x020;

pub const VIRTIO_MMIO_GUEST_PAGE_SIZE: usize = 0x028;

pub const VIRTIO_MMIO_QUEUE_SEL: usize = 0x030;
pub const VIRTIO_MMIO_QUEUE_NUM_MAX: usize = 0x034;
pub const VIRTIO_MMIO_QUEUE_NUM: usize = 0x038;

pub const VIRTIO_MMIO_QUEUE_PIN: usize = 0x040;

pub const VIRTIO_MMIO_STATUS: usize = 0x070;
