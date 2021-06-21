use crate::device::VirtioDeviceType;
use crate::device::VirtioMmio;
use alloc::sync::Arc;
use spin::Mutex;

pub trait VirtioQueue {
    fn virtio_queue_init(&self, dev_type: VirtioDeviceType);
    fn virtio_queue_reset(&self, index: usize);
}

#[derive(Clone)]
pub struct Virtq {
    inner: Arc<Mutex<VirtqInner>>,
}

impl Virtq {
    pub fn default() -> Virtq {
        Virtq {
            inner: Arc::new(Mutex::new(VirtqInner::default())),
        }
    }

    pub fn reset(&self, index: usize) {
        let mut inner = self.inner.lock();
        inner.reset(index);
    }

    pub fn set_notify_handler(&self, handler: fn(Virtq, VirtioMmio) -> bool) {
        let mut inner = self.inner.lock();
        inner.notify_handler = Some(handler);
    }

    pub fn call_notify_handler(&self, mmio: VirtioMmio) -> bool {
        let mut inner = self.inner.lock();
        match inner.notify_handler {
            Some(handler) => {
                return handler(self.clone(), mmio);
            }
            None => {
                println!("call_notify_handler: virtq notify handler is None");
                return false;
            }
        }
    }
}

pub struct VirtqInner {
    ready: usize,
    vq_index: usize,
    notify_handler: Option<fn(Virtq, VirtioMmio) -> bool>,
}

impl VirtqInner {
    pub fn default() -> VirtqInner {
        VirtqInner {
            ready: 0,
            vq_index: 0,
            notify_handler: None,
        }
    }

    // TODO: fix this reset fn
    pub fn reset(&mut self, index: usize) {
        self.ready = 0;
        self.vq_index = index;
        self.notify_handler = None;
    }
}
