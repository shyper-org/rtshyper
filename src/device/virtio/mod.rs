pub use blk::{virtio_blk_notify_handler, BlkIov, VIRTIO_BLK_T_IN, VIRTIO_BLK_T_OUT};
pub use mac::remove_virtio_nic;
pub use mediated::*;
pub use mmio::{emu_virtio_mmio_init, VirtioMmio};
pub use net::{ethernet_ipi_rev_handler, virtio_net_announce};
pub use queue::Virtq;

#[cfg(feature = "balloon")]
mod balloon;
mod blk;
#[allow(dead_code)]
mod console;
mod dev;
mod iov;
mod mac;
mod mediated;
mod mmio;
#[allow(dead_code)]
mod net;
mod queue;
