use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

const VIRTIO_F_VERSION_1: usize = 1 << 32;
const VIRTIO_NET_F_MAC: usize = 1 << 5;
const VIRTIO_NET_F_GUEST_CSUM: usize = 1 << 1;

#[derive(Clone)]
pub struct NetDesc {
    inner: Arc<Mutex<NetDescInner>>,
}

impl NetDesc {
    pub fn default() -> NetDesc {
        NetDesc {
            inner: Arc::new(Mutex::new(NetDescInner::default())),
        }
    }

    pub fn cfg_init(&self, mac: &Vec<usize>) {
        let mut inner = self.inner.lock();
        inner.mac[0] = mac[0] as u8;
        inner.mac[1] = mac[1] as u8;
        inner.mac[2] = mac[2] as u8;
        inner.mac[3] = mac[3] as u8;
        inner.mac[4] = mac[4] as u8;
        inner.mac[5] = mac[5] as u8;
    }
}

#[repr(C)]
pub struct NetDescInner {
    mac: [u8; 6],
    status: u16,
}

impl NetDescInner {
    pub fn default() -> NetDescInner {
        NetDescInner {
            mac: [0; 6],
            status: 0,
        }
    }
}

pub fn net_features() -> usize {
    VIRTIO_F_VERSION_1 | VIRTIO_NET_F_GUEST_CSUM | VIRTIO_NET_F_MAC
}

use crate::device::{VirtioMmio, Virtq};
pub fn virtio_net_notify_handler(vq: Virtq, nic: VirtioMmio) -> bool {
    unimplemented!();
}

use crate::kernel::IpiMessage;
pub fn ethernet_ipi_msg_handler(msg: &IpiMessage) {
    unimplemented!();
}

pub fn ethernet_ipi_ack_handler(msg: &IpiMessage) {
    unimplemented!();
}
