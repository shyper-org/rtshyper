use alloc::sync::Arc;

use spin::Mutex;

pub const ARM_CORTEX_A57: u8 = 0;
pub const ARM_NVIDIA_DENVER: u8 = 1;

#[derive(Clone)]
pub struct BlkStat {
    inner: Arc<Mutex<BlkStatInner>>,
}

impl BlkStat {
    pub fn default() -> BlkStat {
        BlkStat {
            inner: Arc::new(Mutex::new(BlkStatInner::default())),
        }
    }

    pub fn read_req(&self) -> usize {
        let inner = self.inner.lock();
        inner.read_req
    }

    pub fn read_byte(&self) -> usize {
        let inner = self.inner.lock();
        inner.read_byte
    }

    pub fn write_req(&self) -> usize {
        let inner = self.inner.lock();
        inner.write_req
    }

    pub fn write_byte(&self) -> usize {
        let inner = self.inner.lock();
        inner.write_byte
    }

    pub fn set_read_req(&self, read_req: usize) {
        let mut inner = self.inner.lock();
        inner.read_req = read_req;
    }

    pub fn set_read_byte(&self, read_byte: usize) {
        let mut inner = self.inner.lock();
        inner.read_byte = read_byte;
    }

    pub fn set_write_req(&self, write_req: usize) {
        let mut inner = self.inner.lock();
        inner.write_req = write_req;
    }

    pub fn set_write_byte(&self, write_byte: usize) {
        let mut inner = self.inner.lock();
        inner.write_byte = write_byte;
    }
}

struct BlkStatInner {
    read_req: usize,
    write_req: usize,
    read_byte: usize,
    write_byte: usize,
}

impl BlkStatInner {
    fn default() -> BlkStatInner {
        BlkStatInner {
            read_req: 0,
            write_req: 0,
            read_byte: 0,
            write_byte: 0,
        }
    }
}

#[derive(Clone)]
pub struct NicStat {
    inner: Arc<Mutex<NicStatInner>>,
}

impl NicStat {
    pub fn default() -> NicStat {
        NicStat {
            inner: Arc::new(Mutex::new(NicStatInner::default())),
        }
    }

    pub fn send_req(&self) -> usize {
        let inner = self.inner.lock();
        inner.send_req
    }

    pub fn send_byte(&self) -> usize {
        let inner = self.inner.lock();
        inner.send_byte
    }

    pub fn discard(&self) -> usize {
        let inner = self.inner.lock();
        inner.discard
    }

    pub fn receive_req(&self) -> usize {
        let mut inner = self.inner.lock();
        inner.receive_req
    }

    pub fn receive_byte(&self) -> usize {
        let mut inner = self.inner.lock();
        inner.receive_byte
    }

    pub fn set_send_req(&self, req: usize) {
        let mut inner = self.inner.lock();
        inner.send_req = req;
    }

    pub fn set_send_byte(&self, byte: usize) {
        let mut inner = self.inner.lock();
        inner.send_byte = byte;
    }

    pub fn set_discard(&self, discard: usize) {
        let mut inner = self.inner.lock();
        inner.discard = discard;
    }

    pub fn set_receive_req(&self, receive_req: usize) {
        let mut inner = self.inner.lock();
        inner.receive_req = receive_req;
    }

    pub fn set_receive_byte(&self, receive_byte: usize) {
        let mut inner = self.inner.lock();
        inner.receive_byte = receive_byte;
    }
}

struct NicStatInner {
    send_req: usize,
    receive_req: usize,
    send_byte: usize,
    receive_byte: usize,
    discard: usize,
}

impl NicStatInner {
    fn default() -> NicStatInner {
        NicStatInner {
            send_req: 0,
            receive_req: 0,
            send_byte: 0,
            receive_byte: 0,
            discard: 0,
        }
    }
}
