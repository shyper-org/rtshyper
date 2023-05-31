use alloc::sync::Arc;

use spin::Mutex;

use crate::arch::PAGE_SIZE;
use crate::device::{VirtioMmio, Virtq};
use crate::kernel::{active_vm, vm_if_set_mem_map_bit};
use crate::kernel::vm;
use crate::kernel::Vm;
use crate::util::round_down;

use super::dev::DevDesc;
use super::iov::VirtioIov;

pub const VIRTQUEUE_CONSOLE_MAX_SIZE: usize = 64;

const VIRTIO_F_VERSION_1: usize = 1 << 32;

const VIRTQUEUE_SERIAL_MAX_SIZE: usize = 64;

const VIRTIO_CONSOLE_F_SIZE: usize = 1 << 0;
const VIRTIO_CONSOLE_F_MULTIPORT: usize = 1 << 1;
const VIRTIO_CONSOLE_F_EMERG_WRITE: usize = 1 << 2;

const VIRTIO_CONSOLE_DEVICE_READY: usize = 0;
const VIRTIO_CONSOLE_DEVICE_ADD: usize = 1;
const VIRTIO_CONSOLE_DEVICE_REMOVE: usize = 2;
const VIRTIO_CONSOLE_PORT_READY: usize = 3;
const VIRTIO_CONSOLE_CONSOLE_PORT: usize = 4;
const VIRTIO_CONSOLE_RESIZE: usize = 5;
const VIRTIO_CONSOLE_PORT_OPEN: usize = 6;
const VIRTIO_CONSOLE_PORT_NAME: usize = 7;

#[derive(Clone)]
pub struct ConsoleDesc {
    inner: Arc<Mutex<ConsoleDescInner>>,
}

impl ConsoleDesc {
    pub fn new(oppo_end_vmid: u16, oppo_end_ipa: u64) -> ConsoleDesc {
        let mut desc = ConsoleDescInner::default();
        desc.oppo_end_vmid = oppo_end_vmid;
        desc.oppo_end_ipa = oppo_end_ipa;
        desc.cols = 80;
        desc.rows = 25;
        ConsoleDesc {
            inner: Arc::new(Mutex::new(desc)),
        }
    }

    fn start_addr(&self) -> usize {
        let inner = self.inner.lock();
        &inner.cols as *const _ as usize
    }

    pub fn offset_data(&self, offset: usize) -> u32 {
        let start_addr = self.start_addr();
        if start_addr + offset < 0x1000 {
            println!("value addr is {}", start_addr + offset);
        }

        unsafe { *((start_addr + offset) as *const u32) }
    }

    pub fn target_console(&self) -> (u16, u64) {
        let inner = self.inner.lock();
        (inner.oppo_end_vmid, inner.oppo_end_ipa)
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
struct ConsoleDescInner {
    oppo_end_vmid: u16,
    oppo_end_ipa: u64,
    // vm access
    cols: u16,
    rows: u16,
    max_nr_ports: u32,
    emerg_wr: u32,
}

impl ConsoleDescInner {
    fn default() -> ConsoleDescInner {
        ConsoleDescInner {
            oppo_end_vmid: 0,
            oppo_end_ipa: 0,
            cols: 0,
            rows: 0,
            max_nr_ports: 0,
            emerg_wr: 0,
        }
    }
}

pub fn console_features() -> usize {
    VIRTIO_F_VERSION_1 | VIRTIO_CONSOLE_F_SIZE
}

pub fn virtio_console_notify_handler(vq: Virtq, console: VirtioMmio, vm: Vm) -> bool {
    if vq.vq_indx() % 4 != 1 {
        // println!("console rx queue notified!");
        return true;
    }

    if vq.ready() == 0 {
        println!("virtio_console_notify_handler: console virt_queue is not ready!");
        return false;
    }

    let tx_iov = VirtioIov::default();
    let dev = console.dev();

    let (trgt_vmid, trgt_console_ipa) = match dev.desc() {
        DevDesc::ConsoleDesc(desc) => desc.target_console(),
        _ => {
            println!("virtio_console_notify_handler: console desc should not be None");
            return false;
        }
    };

    // let buf = dev.cache();
    let mut next_desc_idx_opt = vq.pop_avail_desc_idx(vq.avail_idx());

    while next_desc_idx_opt.is_some() {
        let mut idx = next_desc_idx_opt.unwrap() as usize;
        let mut len = 0;
        tx_iov.clear();

        loop {
            let addr = active_vm().unwrap().ipa2hva(vq.desc_addr(idx));
            if addr == 0 {
                println!("virtio_console_notify_handler: failed to desc addr");
                return false;
            }
            tx_iov.push_data(addr, vq.desc_len(idx) as usize);

            len += vq.desc_len(idx) as usize;
            if vq.desc_flags(idx) == 0 {
                break;
            }
            idx = vq.desc_next(idx) as usize;
        }

        if !virtio_console_recv(trgt_vmid, trgt_console_ipa, tx_iov.clone(), len) {
            println!("virtio_console_notify_handler: failed send");
            // return false;
        }
        if vm.id() != 0 {
            vm_if_set_mem_map_bit(&vm, vq.used_addr());
            vm_if_set_mem_map_bit(&vm, vq.used_addr() + PAGE_SIZE);
        }
        if !vq.update_used_ring(len as u32, next_desc_idx_opt.unwrap() as u32) {
            return false;
        }

        next_desc_idx_opt = vq.pop_avail_desc_idx(vq.avail_idx());
    }

    if !vq.avail_is_avail() {
        println!("invalid descriptor table index");
        return false;
    }

    console.notify(vm);
    // vq.notify(dev.int_id(), vm.clone());

    true
}

fn virtio_console_recv(trgt_vmid: u16, trgt_console_ipa: u64, tx_iov: VirtioIov, len: usize) -> bool {
    let trgt_vm = match vm(trgt_vmid as usize) {
        None => {
            println!("target vm [{}] is not ready or not exist", trgt_vmid);
            return true;
        }
        Some(vm) => vm,
    };

    let console = match trgt_vm.emu_console_dev(trgt_console_ipa as usize) {
        Some(x) => x,
        _ => {
            println!(
                "virtio_console_recv: trgt_vm[{}] failed to get virtio console dev",
                trgt_vmid
            );
            return true;
        }
    };

    if !console.dev().activated() {
        println!(
            "virtio_console_recv: trgt_vm[{}] virtio console dev is not ready",
            trgt_vmid
        );
        return false;
    }

    let rx_vq = match console.vq(0) {
        Ok(x) => x,
        Err(_) => {
            println!(
                "virtio_console_recv: trgt_vm[{}] failed to get virtio console rx virt queue",
                trgt_vmid
            );
            return false;
        }
    };

    let desc_header_idx_opt = rx_vq.pop_avail_desc_idx(rx_vq.avail_idx());
    if !rx_vq.avail_is_avail() {
        println!("virtio_console_recv: receive invalid avail desc idx");
        return false;
    } else if desc_header_idx_opt.is_none() {
        // println!("virtio_console_recv: desc_header_idx_opt.is_none()");
        return true;
    }

    let desc_idx_header = desc_header_idx_opt.unwrap();
    let mut desc_idx = desc_header_idx_opt.unwrap() as usize;
    let rx_iov = VirtioIov::default();
    let mut rx_len = 0;
    loop {
        let dst = trgt_vm.ipa2hva(rx_vq.desc_addr(desc_idx));
        if dst == 0 {
            println!(
                "virtio_console_recv: failed to get dst, desc_idx {}, avail idx {}",
                desc_idx,
                rx_vq.avail_idx()
            );
            return false;
        }
        let desc_len = rx_vq.desc_len(desc_idx) as usize;
        // dirty pages
        if trgt_vmid != 0 {
            let mut ipa_addr = round_down(rx_vq.desc_addr(desc_idx), PAGE_SIZE);
            while ipa_addr <= round_down(rx_vq.desc_addr(desc_idx) + desc_len, PAGE_SIZE) {
                vm_if_set_mem_map_bit(&trgt_vm, ipa_addr);
                ipa_addr += PAGE_SIZE;
            }
        }
        rx_iov.push_data(dst, desc_len);
        rx_len += desc_len;
        if rx_len >= len {
            break;
        }
        if rx_vq.desc_flags(desc_idx) & 0x1 == 0 {
            break;
        }
        desc_idx = rx_vq.desc_next(desc_idx) as usize;
    }

    if rx_len < len {
        rx_vq.put_back_avail_desc_idx();
        println!("virtio_console_recv: rx_len smaller than tx_len");
        return false;
    }

    if tx_iov.write_through_iov(rx_iov.clone(), len) > 0 {
        println!(
            "virtio_console_recv: write through iov failed, rx_iov_num {} tx_iov_num {} rx_len {} tx_len {}",
            rx_iov.num(),
            tx_iov.num(),
            rx_len,
            len
        );
        return false;
    }

    if trgt_vmid != 0 {
        vm_if_set_mem_map_bit(&trgt_vm, rx_vq.used_addr());
        vm_if_set_mem_map_bit(&trgt_vm, rx_vq.used_addr() + PAGE_SIZE);
    }
    if !rx_vq.update_used_ring(len as u32, desc_idx_header as u32) {
        println!(
            "virtio_console_recv: update used ring failed len {} rx_vq num {}",
            len,
            rx_vq.num()
        );
        return false;
    }

    console.notify(trgt_vm);
    // rx_vq.notify(console.dev().int_id(), trgt_vm.clone());
    true
}
