use alloc::sync::Arc;
use core::slice;

use spin::Mutex;

use crate::device::VirtioDeviceType;
use crate::device::VirtioMmio;
use crate::kernel::{active_vm, active_vm_id, Vm};
use crate::lib::trace;

pub const VIRTQ_READY: usize = 1;
pub const VIRTQ_DESC_F_NEXT: usize = 1;
pub const VIRTQ_DESC_F_WRITE: usize = 2;

pub const VRING_USED_F_NO_NOTIFY: usize = 1;

pub const DESC_QUEUE_SIZE: usize = 32768;

#[repr(C, align(16))]
struct VringDesc {
    /*Address (guest-physical)*/
    pub addr: usize,
    /* Length */
    len: u32,
    /* The flags as indicated above */
    flags: u16,
    /* We chain unused descriptors via this, too */
    next: u16,
}

#[repr(C)]
struct VringAvail {
    flags: u16,
    idx: u16,
    ring: [u16; 512],
}

#[repr(C)]
struct VringUsedElem {
    pub id: u32,
    pub len: u32,
}

#[repr(C)]
struct VringUsed {
    flags: u16,
    idx: u16,
    ring: [VringUsedElem; 512],
}

pub trait VirtioQueue {
    fn virtio_queue_init(&self, dev_type: VirtioDeviceType);
    fn virtio_queue_reset(&self, index: usize);
}

#[derive(Clone)]
pub struct Virtq {
    inner: Arc<Mutex<VirtqInner<'static>>>,
}

impl Virtq {
    pub fn default() -> Virtq {
        Virtq {
            inner: Arc::new(Mutex::new(VirtqInner::default())),
        }
    }

    pub fn notify(&self, int_id: usize, vm: Vm) {
        // panic!("should not notify");
        let inner = self.inner.lock();
        use crate::kernel::interrupt_vm_inject;
        if vm.id() != active_vm_id() {
            println!("vm{} notify int {} to vm{}", active_vm_id(), int_id, vm.id());
            // panic!("target vm{} should not notify at vm{}", vm.id(), active_vm_id());
        }
        if inner.to_notify {
            drop(inner);
            interrupt_vm_inject(vm.clone(), vm.vcpu(0), int_id, 0);
        }
    }

    pub fn reset(&self, index: usize) {
        let mut inner = self.inner.lock();
        inner.reset(index);
    }

    pub fn pop_avail_desc_idx(&self, avail_idx: u16) -> Option<u16> {
        let mut inner = self.inner.lock();
        match &inner.avail {
            Some(avail) => {
                if avail_idx == inner.last_avail_idx {
                    return None;
                }
                // unsafe {
                //     llvm_asm!("dsb ish");
                // }
                let idx = inner.last_avail_idx as usize % inner.num;
                let avail_desc_idx = avail.ring[idx];
                inner.last_avail_idx += 1;
                return Some(avail_desc_idx);
            }
            None => {
                println!("pop_avail_desc_idx: failed to avail table");
                return None;
            }
        }
    }

    pub fn put_back_avail_desc_idx(&self) {
        let mut inner = self.inner.lock();
        match &inner.avail {
            Some(_) => {
                inner.last_avail_idx -= 1;
            }
            None => {
                println!("put_back_avail_desc_idx: failed to avail table");
            }
        }
    }

    pub fn avail_is_avail(&self) -> bool {
        let inner = self.inner.lock();
        inner.avail.is_some()
    }

    pub fn disable_notify(&self) {
        let mut inner = self.inner.lock();
        if inner.used_flags & VRING_USED_F_NO_NOTIFY as u16 != 0 {
            return;
        }
        inner.used_flags |= VRING_USED_F_NO_NOTIFY as u16;
    }

    pub fn enable_notify(&self) {
        let mut inner = self.inner.lock();
        if inner.used_flags & VRING_USED_F_NO_NOTIFY as u16 == 0 {
            return;
        }
        inner.used_flags &= !VRING_USED_F_NO_NOTIFY as u16;
    }

    pub fn check_avail_idx(&self, avail_idx: u16) -> bool {
        let inner = self.inner.lock();
        return inner.last_avail_idx == avail_idx;
    }

    pub fn desc_is_writable(&self, idx: usize) -> bool {
        let inner = self.inner.lock();
        let desc_table = inner.desc_table.as_ref().unwrap();
        desc_table[idx].flags & VIRTQ_DESC_F_WRITE as u16 != 0
    }

    pub fn desc_has_next(&self, idx: usize) -> bool {
        let inner = self.inner.lock();
        let desc_table = inner.desc_table.as_ref().unwrap();
        desc_table[idx].flags & VIRTQ_DESC_F_NEXT as u16 != 0
    }

    pub fn update_used_ring(&self, len: u32, desc_chain_head_idx: u32, num: usize) -> bool {
        let mut inner = self.inner.lock();
        let flag = inner.used_flags;
        match &mut inner.used {
            Some(used) => {
                used.flags = flag;
                used.ring[used.idx as usize % num].id = desc_chain_head_idx;
                used.ring[used.idx as usize % num].len = len;
                // unsafe {
                //     llvm_asm!("dsb ish");
                // }
                used.idx += 1;
                return true;
            }
            None => {
                println!("update_used_ring: failed to used table");
                return false;
            }
        }
    }

    pub fn set_notify_handler(&self, handler: fn(Virtq, VirtioMmio, Vm) -> bool) {
        let mut inner = self.inner.lock();
        inner.notify_handler = Some(handler);
    }

    pub fn call_notify_handler(&self, mmio: VirtioMmio) -> bool {
        let inner = self.inner.lock();
        match inner.notify_handler {
            Some(handler) => {
                drop(inner);
                return handler(self.clone(), mmio, active_vm().unwrap());
            }
            None => {
                println!("call_notify_handler: virtq notify handler is None");
                return false;
            }
        }
    }

    pub fn show_desc_info(&self, size: usize, vm: Vm) {
        let inner = self.inner.lock();
        let desc = inner.desc_table.as_ref().unwrap();
        println!("[*desc_ring*]");
        for i in 0..size {
            use crate::kernel::vm_ipa2pa;
            let desc_addr = vm_ipa2pa(vm.clone(), desc[i].addr);
            println!(
                "index {}   desc_addr_ipa 0x{:x}   desc_addr_pa 0x{:x}   len 0x{:x}   flags {}  next {}",
                i, desc[i].addr, desc_addr, desc[i].len, desc[i].flags, desc[i].next
            );
        }
    }

    pub fn show_avail_info(&self, size: usize) {
        let inner = self.inner.lock();
        let avail = inner.avail.as_ref().unwrap();
        println!("[*avail_ring*]");
        for i in 0..size {
            println!("index {} ring_idx {}", i, avail.ring[i]);
        }
    }

    pub fn show_used_info(&self, size: usize) {
        let inner = self.inner.lock();
        let used = inner.used.as_ref().unwrap();
        println!("[*used_ring*]");
        for i in 0..size {
            println!(
                "index {} ring_id {} ring_len {:x}",
                i, used.ring[i].id, used.ring[i].len
            );
        }
    }

    pub fn show_addr_info(&self) {
        let inner = self.inner.lock();
        println!(
            "avail_addr {:x}, desc_addr {:x}, used_addr {:x}",
            inner.avail_addr, inner.desc_table_addr, inner.used_addr
        );
    }

    pub fn set_last_used_idx(&self, last_used_idx: u16) {
        let mut inner = self.inner.lock();
        inner.last_used_idx = last_used_idx;
    }

    pub fn set_num(&self, num: usize) {
        let mut inner = self.inner.lock();
        inner.num = num;
    }

    pub fn set_ready(&self, ready: usize) {
        let mut inner = self.inner.lock();
        inner.ready = ready;
    }

    pub fn or_desc_table_addr(&self, addr: usize) {
        let mut inner = self.inner.lock();
        inner.desc_table_addr |= addr;
    }

    pub fn or_avail_addr(&self, addr: usize) {
        let mut inner = self.inner.lock();
        inner.avail_addr |= addr;
    }

    pub fn or_used_addr(&self, addr: usize) {
        let mut inner = self.inner.lock();
        inner.used_addr |= addr;
    }

    pub fn set_desc_table(&self, addr: usize) {
        let mut inner = self.inner.lock();
        inner.desc_table = Some(unsafe {
            if trace() && addr < 0x1000 {
                panic!("illegal desc ring addr {:x}", addr);
            }
            slice::from_raw_parts_mut(addr as *mut VringDesc, 16 * DESC_QUEUE_SIZE)
        });
    }

    pub fn set_avail(&self, addr: usize) {
        if trace() && addr < 0x1000 {
            panic!("illegal avail ring addr {:x}", addr);
        }
        let mut inner = self.inner.lock();
        inner.avail = Some(unsafe { &mut *(addr as *mut VringAvail) });
    }

    pub fn set_used(&self, addr: usize) {
        if trace() && addr < 0x1000 {
            panic!("illegal used ring addr {:x}", addr);
        }
        let mut inner = self.inner.lock();
        inner.used = Some(unsafe { &mut *(addr as *mut VringUsed) });
    }

    pub fn last_used_idx(&self) -> u16 {
        let inner = self.inner.lock();
        inner.last_used_idx
    }

    pub fn desc_table_addr(&self) -> usize {
        let inner = self.inner.lock();
        inner.desc_table_addr
    }

    pub fn avail_addr(&self) -> usize {
        let inner = self.inner.lock();
        inner.avail_addr
    }

    pub fn used_addr(&self) -> usize {
        let inner = self.inner.lock();
        inner.used_addr
    }

    pub fn ready(&self) -> usize {
        let inner = self.inner.lock();
        inner.ready
    }

    pub fn vq_indx(&self) -> usize {
        let inner = self.inner.lock();
        inner.vq_index
    }

    pub fn num(&self) -> usize {
        let inner = self.inner.lock();
        inner.num
    }

    pub fn desc_addr(&self, idx: usize) -> usize {
        let inner = self.inner.lock();
        let desc_table = inner.desc_table.as_ref().unwrap();
        desc_table[idx].addr
    }

    pub fn desc_flags(&self, idx: usize) -> u16 {
        let inner = self.inner.lock();
        let desc_table = inner.desc_table.as_ref().unwrap();
        desc_table[idx].flags
    }

    pub fn desc_next(&self, idx: usize) -> u16 {
        let inner = self.inner.lock();
        let desc_table = inner.desc_table.as_ref().unwrap();
        desc_table[idx].next
    }

    pub fn desc_len(&self, idx: usize) -> u32 {
        let inner = self.inner.lock();
        let desc_table = inner.desc_table.as_ref().unwrap();
        desc_table[idx].len
    }

    pub fn avail_flags(&self) -> u16 {
        let inner = self.inner.lock();
        let avail = inner.avail.as_ref().unwrap();
        avail.flags
    }

    pub fn avail_idx(&self) -> u16 {
        let inner = self.inner.lock();
        let avail = inner.avail.as_ref().unwrap();
        avail.idx
    }

    pub fn used_idx(&self) -> u16 {
        let inner = self.inner.lock();
        let used = inner.used.as_ref().unwrap();
        used.idx
    }
}

pub struct VirtqInner<'a> {
    ready: usize,
    vq_index: usize,
    num: usize,
    desc_table: Option<&'a mut [VringDesc]>,
    avail: Option<&'a mut VringAvail>,
    used: Option<&'a mut VringUsed>,
    last_avail_idx: u16,
    last_used_idx: u16,
    used_flags: u16,
    to_notify: bool,

    desc_table_addr: usize,
    avail_addr: usize,
    used_addr: usize,

    notify_handler: Option<fn(Virtq, VirtioMmio, Vm) -> bool>,
}

impl VirtqInner<'_> {
    pub fn default() -> Self {
        VirtqInner {
            ready: 0,
            vq_index: 0,
            num: 0,
            desc_table: None,
            avail: None,
            used: None,
            last_avail_idx: 0,
            last_used_idx: 0,
            used_flags: 0,
            to_notify: true,

            desc_table_addr: 0,
            avail_addr: 0,
            used_addr: 0,

            notify_handler: None,
        }
    }

    // virtio_queue_reset
    pub fn reset(&mut self, index: usize) {
        self.ready = 0;
        self.vq_index = index;
        self.num = 0;
        self.last_avail_idx = 0;
        self.last_used_idx = 0;
        self.used_flags = 0;
        self.to_notify = true;
        self.desc_table_addr = 0;
        self.avail_addr = 0;
        self.used_addr = 0;

        self.desc_table = None;
        self.avail = None;
        self.used = None;
    }
}
