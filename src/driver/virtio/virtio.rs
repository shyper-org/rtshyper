pub const VIRTIO_CONFIG_S_ACKNOWLEDGE: usize = 1;
pub const VIRTIO_CONFIG_S_DRIVER: usize = 2;
pub const VIRTIO_CONFIG_S_DRIVER_OK: usize = 4;
pub const VIRTIO_CONFIG_S_FEATURES_OK: usize = 8;
pub const VIRTIO_CONFIG_S_NEEDS_RESET: usize = 0x40;
pub const VIRTIO_CONFIG_S_FAILED: usize = 0x80;

pub const VIRTIO_F_ANY_LAYOUT: usize = 27;
