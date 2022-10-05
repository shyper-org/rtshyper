use alloc::sync::Arc;
use alloc::vec::Vec;
use core::slice;

use spin::Mutex;

use crate::device::VirtioDeviceType;
use crate::device::VirtioMmio;
use crate::kernel::{active_vm, current_cpu, ipa2pa, VirtqData, Vm, vm_ipa2pa, VmPa};
use crate::kernel::{ipi_send_msg, IpiInnerMsg, IpiIntInjectMsg, IpiType};
use crate::lib::trace;

pub const VIRTQ_READY: usize = 1;
pub const VIRTQ_DESC_F_NEXT: usize = 1;
pub const VIRTQ_DESC_F_WRITE: usize = 2;

pub const VRING_USED_F_NO_NOTIFY: usize = 1;

pub const DESC_QUEUE_SIZE: usize = 512;

#[repr(C, align(16))]
#[derive(Copy, Clone)]
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
#[derive(Copy, Clone)]
struct VringAvail {
    flags: u16,
    idx: u16,
    ring: [u16; 512],
}

#[repr(C)]
#[derive(Copy, Clone)]
struct VringUsedElem {
    pub id: u32,
    pub len: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct VringUsed {
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
        // Undo: mmio->regs->irt_stat = VIRTIO_MMIO_INT_VRING; Is it necessery?
        let inner = self.inner.lock();
        let trgt_id = vm.vcpu(0).unwrap().phys_id();
        use crate::kernel::interrupt_vm_inject;
        if trgt_id == current_cpu().id {
            drop(inner);
            interrupt_vm_inject(vm.clone(), vm.vcpu(0).unwrap(), int_id, 0);
        } else {
            let m = IpiIntInjectMsg { vm_id: vm.id(), int_id };
            if !ipi_send_msg(trgt_id, IpiType::IpiTIntInject, IpiInnerMsg::IntInjectMsg(m)) {
                println!("notify: failed to send ipi to Core {}", trgt_id);
            }
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
                inner.last_avail_idx = inner.last_avail_idx.wrapping_add(1);
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

    pub fn update_used_ring(&self, len: u32, desc_chain_head_idx: u32) -> bool {
        let mut inner = self.inner.lock();
        let num = inner.num;
        let flag = inner.used_flags;
        match &mut inner.used {
            Some(used) => {
                used.flags = flag;
                used.ring[used.idx as usize % num].id = desc_chain_head_idx;
                used.ring[used.idx as usize % num].len = len;
                used.idx = used.idx.wrapping_add(1);
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

    pub fn desc_table(&self) -> usize {
        let inner = self.inner.lock();
        match &inner.desc_table {
            None => 0,
            Some(desc_table) => &(desc_table[0]) as *const _ as usize,
        }
    }

    pub fn avail(&self) -> usize {
        let inner = self.inner.lock();
        match &inner.avail {
            None => 0,
            Some(avail) => (*avail) as *const _ as usize,
        }
    }

    pub fn used(&self) -> usize {
        let inner = self.inner.lock();
        match &inner.used {
            None => 0,
            Some(used) => (*used) as *const _ as usize,
        }
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

    pub fn last_avail_idx(&self) -> u16 {
        let inner = self.inner.lock();
        inner.last_avail_idx
    }

    pub fn used_idx(&self) -> u16 {
        let inner = self.inner.lock();
        let used = inner.used.as_ref().unwrap();
        used.idx
    }

    // use for migration
    pub fn restore_vq_data(&self, data: &VirtqData, pa_region: &Vec<VmPa>) {
        let mut inner = self.inner.lock();
        inner.ready = data.ready;
        inner.vq_index = data.vq_index;
        inner.num = data.num;
        inner.last_avail_idx = data.last_avail_idx;
        inner.last_used_idx = data.last_used_idx;
        inner.used_flags = data.used_flags;
        inner.desc_table_addr = data.desc_table_ipa;
        inner.avail_addr = data.avail_ipa;
        inner.used_addr = data.used_ipa;
        let desc_table_addr = ipa2pa(pa_region, data.desc_table_ipa);
        let avail_addr = ipa2pa(pa_region, data.avail_ipa);
        let used_addr = ipa2pa(pa_region, data.used_ipa);
        // println!("restore_vq_data: ready {}, vq idx {}, last_avail_idx {}, last_used_idx {}, desc_table_ipa {:x}, avail_ipa {:x}, used_ipa {:x}, desc_table_pa {:x}, avail_pa {:x}, used_pa {:x}",
        //          data.ready, data.vq_index, data.last_avail_idx, data.last_used_idx, data.desc_table_ipa, data.avail_ipa, data.used_ipa, desc_table_addr, avail_addr, used_addr);
        if desc_table_addr != 0 {
            inner.desc_table =
                Some(unsafe { slice::from_raw_parts_mut(desc_table_addr as *mut VringDesc, 16 * DESC_QUEUE_SIZE) });
        }
        if avail_addr != 0 {
            inner.avail = Some(unsafe { &mut *(avail_addr as *mut VringAvail) });
            // println!("restore_vq_data: avail idx {}", inner.avail.as_ref().unwrap().idx);
        }
        if used_addr != 0 {
            inner.used = Some(unsafe { &mut *(used_addr as *mut VringUsed) });
            // println!("restore_vq_data: used idx {}", inner.used.as_ref().unwrap().idx);
        }
    }

    // use for migration
    pub fn save_vq_data(&self, data: &mut VirtqData, _pa_region: &Vec<VmPa>) {
        let inner = self.inner.lock();
        data.ready = inner.ready;
        data.vq_index = inner.vq_index;
        data.num = inner.num;
        data.last_avail_idx = inner.last_avail_idx;
        data.last_used_idx = inner.last_used_idx;
        data.used_flags = inner.used_flags;
        data.desc_table_ipa = inner.desc_table_addr;
        data.avail_ipa = inner.avail_addr;
        data.used_ipa = inner.used_addr;

        // println!("save_vq_data: ready {}, vq idx {}, last_avail_idx {}, last_used_idx {}, desc_table_ipa {:x}, avail_ipa {:x}, used_ipa {:x}",
        //          data.ready, data.vq_index, data.last_avail_idx, data.last_used_idx, data.desc_table_ipa, data.avail_ipa, data.used_ipa);
        // if inner.avail.is_some() {
        //     println!("save_vq_data: avail idx {}", inner.avail.as_ref().unwrap().idx);
        // }
        // if inner.used.is_some() {
        //     println!("save_vq_data: used idx {}", inner.used.as_ref().unwrap().idx);
        // }
    }

    // use for live update
    pub fn save_vq(&self, vq: Virtq, notify_handler: Option<fn(Virtq, VirtioMmio, Vm) -> bool>) {
        let mut dst_inner = self.inner.lock();
        let src_inner = vq.inner.lock();
        dst_inner.ready = src_inner.ready;
        dst_inner.vq_index = src_inner.vq_index;
        dst_inner.num = src_inner.num;

        dst_inner.desc_table = match &src_inner.desc_table {
            None => None,
            Some(desc_table) => {
                let desc_addr = &desc_table[0] as *const _ as usize;
                Some(unsafe { slice::from_raw_parts_mut(desc_addr as *mut VringDesc, 16 * DESC_QUEUE_SIZE) })
            }
        };
        dst_inner.avail = match &src_inner.avail {
            None => None,
            Some(avail) => {
                let avail_addr = *avail as *const _ as usize;
                Some(unsafe { &mut *(avail_addr as *mut VringAvail) })
            }
        };
        dst_inner.used = match &src_inner.used {
            None => None,
            Some(used) => {
                let used_addr = *used as *const _ as usize;
                Some(unsafe { &mut *(used_addr as *mut VringUsed) })
            }
        };

        dst_inner.last_avail_idx = src_inner.last_avail_idx;
        dst_inner.last_used_idx = src_inner.last_used_idx;
        dst_inner.used_flags = src_inner.used_flags;
        dst_inner.desc_table_addr = src_inner.desc_table_addr;
        dst_inner.avail_addr = src_inner.avail_addr;
        dst_inner.used_addr = src_inner.used_addr;
        dst_inner.notify_handler = notify_handler;
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
        self.desc_table_addr = 0;
        self.avail_addr = 0;
        self.used_addr = 0;

        self.desc_table = None;
        self.avail = None;
        self.used = None;
    }
}
