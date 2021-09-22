use crate::config::{vm_num, vm_type};
use crate::device::DevStat;
use crate::kernel::mem_heap_alloc;
use crate::kernel::{
    active_vm, active_vm_id, cpu_id, mem_pages_free, vm_if_list_cmp_mac, vm_if_list_get_cpu_id,
    vm_ipa2pa,
};
use crate::kernel::{ipi_send_msg, IpiEthernetMsg, IpiInnerMsg, IpiType};
use crate::lib::memcpy;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::mem::size_of;
use core::slice::from_raw_parts;
use spin::Mutex;

const VIRTIO_F_VERSION_1: usize = 1 << 32;
const VIRTIO_NET_F_MAC: usize = 1 << 5;
const VIRTIO_NET_F_GUEST_CSUM: usize = 1 << 1;

const VIRTIO_NET_HDR_F_DATA_VALID: usize = 2;

const VIRTIO_NET_HDR_GSO_NONE: usize = 0;

#[repr(C)]
struct VirtioNetHdr {
    pub flags: u8,
    pub gso_type: u8,
    pub hdr_len: u16,
    pub gso_size: u16,
    pub csum_start: u16,
    pub csum_offset: u16,
    pub num_buffers: u16,
}

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
    if vq.ready() == 0 {
        println!("net virt_queue is not ready!");
        return false;
    }

    if vq.vq_indx() != 1 {
        println!("net rx queue notified!");
        return false;
    }

    let dev = nic.dev();
    let buf = dev.cache();
    let vq_size = vq.num();
    let mut next_desc_idx_opt = vq.pop_avail_desc_idx();
    while next_desc_idx_opt.is_some() {
        let mut idx = next_desc_idx_opt.unwrap() as usize;
        let mut ptr = buf;
        loop {
            let temp = vm_ipa2pa(vq.desc_addr(idx));
            if temp == 0 {
                println!("virtio_net_notify_handler: failed to desc addr");
                return false;
            }

            use crate::lib::memcpy;
            unsafe {
                memcpy(ptr as *mut u8, temp as *mut u8, vq.desc_len(idx) as usize);
            }
            ptr += vq.desc_len(idx) as usize;
            idx = vq.desc_next(idx) as usize;
            if vq.desc_flags(idx) == 0 {
                break;
            }
        }

        let frame = buf + size_of::<VirtioNetHdr>();
        let len = ptr - frame;

        if !vq.update_used_ring(len as u32, next_desc_idx_opt.unwrap() as u32, vq_size) {
            return false;
        }

        if frame == 0 || !ethernet_transmit(unsafe { from_raw_parts(frame as *const u8, len) }, len)
        {
            vq.notify(dev.int_id());
            return true;
        }

        next_desc_idx_opt = vq.pop_avail_desc_idx();
    }

    if !vq.avail_is_avail() {
        println!("invalid descriptor table index");
        return false;
    }

    true
    // unimplemented!();
}

use crate::kernel::{IpiEthernetAckMsg, IpiMessage};
use crate::lib::byte2page;
pub fn ethernet_ipi_msg_handler(msg: &IpiMessage) {
    let vm = match active_vm() {
        Some(vm) => vm,
        None => {
            panic!("ethernet_ipi_msg_handler: current vcpu.vm is none");
        }
    };
    match msg.ipi_message {
        IpiInnerMsg::EnternetMsg(ethernet_msg) => {
            let npage = byte2page(ethernet_msg.len);
            let mut ack = IpiEthernetAckMsg {
                succeed: true,
                len: ethernet_msg.len,
            };

            let nic = match vm.emu_net_dev(0) {
                EmuDevs::VirtioNet(x) => x,
                _ => {
                    println!("failed to get virtio net dev");
                    ethernet_ipi_send_msg(&ethernet_msg, &ack, npage);
                    return;
                }
            };
            if !nic.dev().activated() {
                ethernet_ipi_send_msg(&ethernet_msg, &ack, npage);
                return;
            }
            let rx_vq = match nic.vq(0) {
                Ok(x) => x,
                Err(_) => {
                    println!("failed to get virtio net rx virt queue");
                    ethernet_ipi_send_msg(&ethernet_msg, &ack, npage);
                    return;
                }
            };

            let mut desc_idx_opt = rx_vq.pop_avail_desc_idx();
            if !rx_vq.avail_is_avail() {
                println!("receive invalid avail desc idx");
                return;
            } else if desc_idx_opt.is_none() {
                ack.succeed = false;
                rx_vq.notify(nic.dev().int_id());
                return;
            }

            let desc_idx_header = desc_idx_opt.unwrap();
            let header = unsafe { &mut *(ethernet_msg.frame as *mut VirtioNetHdr) };
            header.flags = VIRTIO_NET_HDR_F_DATA_VALID as u8;
            header.gso_type = VIRTIO_NET_HDR_GSO_NONE as u8;
            header.num_buffers = 1;

            let mut remain = ethernet_msg.len;
            let mut frame_ptr = ethernet_msg.frame;

            let mut desc_idx = desc_idx_opt.unwrap() as usize;
            while remain > 0 {
                let dst = vm_ipa2pa(rx_vq.desc_addr(desc_idx));
                if dst == 0 {
                    println!("failed to dst");
                    ack.succeed = false;
                    ethernet_ipi_send_msg(&ethernet_msg, &ack, npage);
                    return;
                }
                let desc_len = rx_vq.desc_len(desc_idx) as usize;
                let written_len = if remain > desc_len { desc_len } else { remain };
                use crate::lib::memcpy;
                unsafe {
                    memcpy(dst as *const u8, frame_ptr as *const u8, written_len);
                }
                frame_ptr += written_len;
                remain -= written_len;
                if rx_vq.desc_flags(desc_idx) & 0x1 != 0 {
                    break;
                }
                desc_idx = rx_vq.desc_next(desc_idx) as usize;
            }

            if remain > 0 {
                println!("rx desc entry length is not long enough");
                ethernet_ipi_send_msg(&ethernet_msg, &ack, npage);
                return;
            }

            if !rx_vq.update_used_ring(
                (ethernet_msg.len - remain) as u32,
                desc_idx_header as u32,
                rx_vq.num(),
            ) {
                ethernet_ipi_send_msg(&ethernet_msg, &ack, npage);
                return;
            }

            let nic_stat = nic.dev().stat();
            if let DevStat::NicStat(stat) = nic_stat {
                stat.set_receive_req(stat.receive_req() + 1);
                stat.set_receive_byte(stat.receive_byte() + ethernet_msg.len);
            }

            if rx_vq.avail_flags() == 0 {
                rx_vq.notify(nic.dev().int_id());
            }

            // need ipi and mem_pages_free
            ethernet_ipi_send_msg(&ethernet_msg, &ack, npage);
        }
        _ => {
            panic!("illegal ipi message type in ethernet_ipi_msg_handler");
        }
    }
}

fn ethernet_ipi_send_msg(msg: &IpiEthernetMsg, ack: &IpiEthernetAckMsg, npage: usize) {
    if !ipi_send_msg(
        msg.src,
        IpiType::IpiTEthernetAck,
        IpiInnerMsg::EthernetAck(*ack),
    ) {
        println!(
            "Failed to send ipi message, target {} type {:#?}",
            msg.src,
            IpiType::IpiTEthernetAck
        );
    }
    mem_pages_free(msg.frame, npage);
}

use crate::device::EmuDevs;
pub fn ethernet_ipi_ack_handler(msg: &IpiMessage) {
    match msg.ipi_message {
        crate::kernel::IpiInnerMsg::EthernetAck(ack_msg) => {
            let vm = match crate::kernel::active_vm() {
                None => {
                    panic!("vgic_ipi_handler: vm is None");
                }
                Some(x) => x,
            };
            let nic = match vm.emu_net_dev(0) {
                EmuDevs::VirtioNet(x) => x,
                _ => {
                    println!("failed to get virtio net dev");
                    return;
                }
            };

            let tx_vq = match nic.vq(nic.q_sel() as usize) {
                Ok(x) => x,
                Err(_) => {
                    println!("failed to get virtio tx net virt queue");
                    return;
                }
            };
            let stat = nic.dev().stat();
            match stat {
                super::DevStat::NicStat(nic_stat) => {
                    nic_stat.set_send_req(nic_stat.send_req() + 1);
                    nic_stat.set_send_byte(ack_msg.len);
                    let discard = nic_stat.discard()
                        + match ack_msg.succeed {
                            true => ack_msg.len,
                            _ => 0,
                        };
                    nic_stat.set_discard(discard);
                    tx_vq.set_last_used_idx(tx_vq.last_used_idx() + 1);

                    if tx_vq.avail_flags() == 0 && tx_vq.last_used_idx() == tx_vq.used_idx() {
                        tx_vq.notify(nic.dev().int_id());
                    }
                }
                _ => {
                    panic!("illegal stat for virt net");
                }
            }
        }
        _ => {}
    }
}

fn ethernet_transmit(frame: &[u8], len: usize) -> bool {
    // [ destination MAC - 6 ][ source MAC - 6 ][ EtherType - 2 ][ Payload ]
    if len < 6 + 6 + 2 {
        return false;
    }
    // need to check mac
    // vm_if_list_cmp_mac(active_vm_id(), frame + 6);

    if frame[0] == 0xff
        && frame[1] == 0xff
        && frame[2] == 0xff
        && frame[3] == 0xff
        && frame[4] == 0xff
        && frame[5] == 0xff
    {
        if !ethernet_is_arp(frame) {
            return false;
        }
        return ethernet_broadcast(frame, len);
    }

    if frame[0] == 0x33 && frame[1] == 0x33 {
        if !(frame[12] == 0x86 && frame[13] == 0xdd) {
            // Only IPV6 multicast packet is allowed to be broadcast
            return false;
        }
        return ethernet_broadcast(frame, len);
    }
    // let vm_id = 0;

    match ethernet_mac_to_vm_id(frame) {
        Ok(vm_id) => {
            return ethernet_send_to(vm_id, frame, len);
        }
        Err(_) => {
            return false;
        }
    }
}

fn ethernet_broadcast(frame: &[u8], len: usize) -> bool {
    let vm_num = vm_num();
    let cur_vm_id = active_vm_id();
    for vm_id in 0..vm_num {
        if vm_id == cur_vm_id {
            continue;
        }
        if vm_type(vm_id) as usize == 0 {
            continue;
        }
        if !ethernet_send_to(vm_id, frame, len) {
            return false;
        }
    }
    return true;
}

fn ethernet_send_to(vmid: usize, frame: &[u8], len: usize) -> bool {
    let npage = byte2page(len + size_of::<VirtioNetHdr>());

    let mut m = IpiEthernetMsg {
        src: cpu_id(),
        len: len + size_of::<VirtioNetHdr>(),
        frame: 0,
    };

    match mem_heap_alloc(npage, false) {
        Ok(page_frame) => {
            m.frame = page_frame.pa();
        }
        Err(_) => {
            println!("ethernet_send_to: failed to alloc pages");
            return false;
        }
    }
    unsafe {
        memcpy(m.frame as *const u8, frame.as_ptr(), len);
    }
    let cpu_trgt = vm_if_list_get_cpu_id(vmid);
    if !ipi_send_msg(
        cpu_trgt,
        IpiType::IpiTEthernetMsg,
        IpiInnerMsg::EnternetMsg(m),
    ) {
        println!(
            "ethernet_send_to: Failed to send ipi message, target {} type {:#?}",
            cpu_trgt,
            IpiType::IpiTEthernetMsg
        );
    }

    return true;
}

fn ethernet_is_arp(frame: &[u8]) -> bool {
    return frame[12] == 0x8 && frame[13] == 0x6;
}

fn ethernet_mac_to_vm_id(frame: &[u8]) -> Result<usize, ()> {
    for vm_id in 0..vm_num() {
        if vm_if_list_cmp_mac(vm_id, frame) {
            return Ok(vm_id);
        }
    }
    return Err(());
}
