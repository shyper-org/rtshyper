use alloc::sync::Arc;
use alloc::vec::Vec;
use core::mem::size_of;
use spin::Mutex;

use crate::arch::PAGE_SIZE;
use crate::config::{vm_num, vm_type};
use crate::device::{DevDesc, VirtioMmio, Virtq, VIRTQ_DESC_F_NEXT, VIRTQ_DESC_F_WRITE};
use crate::device::EmuDevs;
use crate::device::VirtioIov;
use crate::kernel::{
    active_vm, active_vm_id, current_cpu, NetDescData, vm_if_cmp_mac, vm_if_get_cpu_id, vm_if_set_mem_map_bit,
    vm_ipa2pa, VM_LIST, VM_STATE_FLAG,
};
use crate::kernel::{ipi_send_msg, IpiEthernetMsg, IpiInnerMsg, IpiType};
use crate::kernel::IpiMessage;
use crate::kernel::vm;
use crate::kernel::Vm;
use crate::lib::{round_down, trace};

const VIRTIO_NET_OK: u8 = 0;
const VIRTIO_NET_ERR: u8 = 1;

const VIRTIO_F_VERSION_1: usize = 1 << 32;
const VIRTIO_NET_F_CSUM: usize = 1 << 0;
const VIRTIO_NET_F_GUEST_CSUM: usize = 1 << 1;
const VIRTIO_NET_F_MAC: usize = 1 << 5;
const VIRTIO_NET_F_GSO_DEPREC: usize = 1 << 6;
// deprecated: host handles GSO
const VIRTIO_NET_F_GUEST_TSO4: usize = 1 << 7;
// guest can rcv TSOv4 *
const VIRTIO_NET_F_GUEST_TSO6: usize = 1 << 8;
// guest can rcv TSOv6
const VIRTIO_NET_F_GUEST_ECN: usize = 1 << 9;
// guest can rcv TSO with ECN
const VIRTIO_NET_F_GUEST_UFO: usize = 1 << 10;
// guest can rcv UFO *
const VIRTIO_NET_F_HOST_TSO4: usize = 1 << 11;
// host can rcv TSOv4 *
const VIRTIO_NET_F_HOST_TSO6: usize = 1 << 12;
// host can rcv TSOv6
const VIRTIO_NET_F_HOST_ECN: usize = 1 << 13;
// host can rcv TSO with ECN
const VIRTIO_NET_F_HOST_UFO: usize = 1 << 14;
// host can rcv UFO *
const VIRTIO_NET_F_MRG_RXBUF: usize = 1 << 15;
// host can merge RX buffers *
const VIRTIO_NET_F_STATUS: usize = 1 << 16;
// config status field available *
const VIRTIO_NET_F_CTRL_VQ: usize = 1 << 17;
// control channel available
const VIRTIO_NET_F_CTRL_RX: usize = 1 << 18;
// control channel RX mode support
const VIRTIO_NET_F_CTRL_VLAN: usize = 1 << 19;
// control channel VLAN filtering
const VIRTIO_NET_F_GUEST_ANNOUNCE: usize = 1 << 21; // guest can send gratuitous pkts

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

    // use for live update
    pub fn back_up(&self) -> NetDesc {
        let current_inner = self.inner.lock();
        let inner = NetDescInner {
            mac: current_inner.mac,
            status: current_inner.status,
        };
        NetDesc {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    pub fn set_status(&self, status: u16) {
        let mut inner = self.inner.lock();
        inner.status = status;
    }

    pub fn status(&self) -> u16 {
        let inner = self.inner.lock();
        inner.status
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

    pub fn offset_data(&self, offset: usize) -> u32 {
        let inner = self.inner.lock();
        let start_addr = &inner.mac[0] as *const _ as usize;
        if trace() && start_addr + offset < 0x1000 {
            println!("value addr is {}", start_addr + offset);
        }
        let value = unsafe { *((start_addr + offset) as *const u32) };
        return value;
    }

    // use for migration
    pub fn restore_net_data(&self, desc_data: &NetDescData) {
        let mut inner = self.inner.lock();
        inner.mac = desc_data.mac;
        inner.status = desc_data.status;
    }

    // use for migration
    pub fn save_net_data(&self, desc_data: &mut NetDescData) {
        let inner = self.inner.lock();
        desc_data.mac = inner.mac;
        desc_data.status = inner.status;
    }
}

pub const VIRTIO_NET_S_LINK_UP: u16 = 1;
pub const VIRTIO_NET_S_ANNOUNCE: u16 = 2;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct NetDescInner {
    mac: [u8; 6],
    status: u16,
}

impl NetDescInner {
    pub fn default() -> NetDescInner {
        NetDescInner {
            mac: [0; 6],
            status: VIRTIO_NET_S_LINK_UP,
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct VirtioNetCtrlHdr {
    class: u8,
    command: u8,
}

pub fn net_features() -> usize {
    VIRTIO_F_VERSION_1
        | VIRTIO_NET_F_GUEST_CSUM
        | VIRTIO_NET_F_MAC
        | VIRTIO_NET_F_CSUM
        | VIRTIO_NET_F_GUEST_TSO4
        | VIRTIO_NET_F_GUEST_TSO6
        | VIRTIO_NET_F_GUEST_UFO
        | VIRTIO_NET_F_HOST_TSO4
        | VIRTIO_NET_F_HOST_TSO6
        | VIRTIO_NET_F_HOST_UFO
        | VIRTIO_NET_F_HOST_ECN
        | VIRTIO_NET_F_CTRL_VQ
        | VIRTIO_NET_F_GUEST_ANNOUNCE
        | VIRTIO_NET_F_STATUS
}

const VIRTIO_NET_CTRL_ANNOUNCE: u8 = 3;
const VIRTIO_NET_CTRL_ANNOUNCE_ACK: u8 = 0;

pub fn virtio_net_handle_ctrl(vq: Virtq, nic: VirtioMmio, vm: Vm) -> bool {
    if vq.ready() == 0 {
        println!("virtio net control queue is not ready!");
        return false;
    }

    let out_iov = VirtioIov::default();
    let in_iov = VirtioIov::default();
    let mut next_desc_idx_opt = vq.pop_avail_desc_idx(vq.avail_idx());
    while next_desc_idx_opt.is_some() {
        let mut idx = next_desc_idx_opt.unwrap() as usize;
        let mut len = 0;
        out_iov.clear();
        in_iov.clear();

        loop {
            let addr = vm_ipa2pa(active_vm().unwrap(), vq.desc_addr(idx));
            if addr == 0 {
                println!("virtio_net_handle_ctrl: failed to desc addr");
                return false;
            }
            if vq.desc_flags(idx) & VIRTQ_DESC_F_WRITE != 0 {
                in_iov.push_data(addr, vq.desc_len(idx) as usize);
            } else {
                out_iov.push_data(addr, vq.desc_len(idx) as usize);
            }
            len += vq.desc_len(idx) as usize;
            if vq.desc_flags(idx) != VIRTQ_DESC_F_NEXT {
                break;
            }
            idx = vq.desc_next(idx) as usize;
        }
        let ctrl = VirtioNetCtrlHdr::default();
        out_iov.to_buf(&ctrl as *const _ as usize, size_of::<VirtioNetCtrlHdr>());
        match ctrl.class {
            VIRTIO_NET_CTRL_ANNOUNCE => {
                let status: u8 = if ctrl.command == VIRTIO_NET_CTRL_ANNOUNCE_ACK {
                    match nic.dev().desc() {
                        DevDesc::NetDesc(desc) => {
                            desc.set_status(VIRTIO_NET_S_LINK_UP);
                            VIRTIO_NET_OK
                        }
                        _ => {
                            panic!("illegal dev type for nic");
                        }
                    }
                } else {
                    VIRTIO_NET_ERR
                };
                in_iov.from_buf(&status as *const _ as usize, size_of::<u8>());
            }
            _ => {
                println!("Control queue header class can't match {}", ctrl.class);
            }
        }

        // update ctrl queue used ring
        if vm.id() != 0 {
            let used_addr = vm_ipa2pa(vm.clone(), vq.used_addr());
            if *VM_STATE_FLAG.lock() == 1 {
                println!("vm1 virtio net ctrl write memory in 0x{:x}", used_addr);
            }
            vm_if_set_mem_map_bit(vm.clone(), used_addr);

            for idx in 0..in_iov.num() {
                vm_if_set_mem_map_bit(vm.clone(), in_iov.get_buf(idx));
            }
        }
        if !vq.update_used_ring(len as u32, next_desc_idx_opt.unwrap() as u32) {
            return false;
        }
        next_desc_idx_opt = vq.pop_avail_desc_idx(vq.avail_idx());
    }
    nic.notify(vm.clone());
    true
}

pub fn virtio_net_notify_handler(vq: Virtq, nic: VirtioMmio, vm: Vm) -> bool {
    if vq.ready() == 0 {
        println!("net virt_queue is not ready!");
        return false;
    }

    if vq.vq_indx() != 1 {
        // println!("net rx queue notified!");
        return true;
    }

    let tx_iov = VirtioIov::default();
    let mut vms_to_notify = 0;

    let mut next_desc_idx_opt = vq.pop_avail_desc_idx(vq.avail_idx());

    while next_desc_idx_opt.is_some() {
        let mut idx = next_desc_idx_opt.unwrap() as usize;
        let mut len = 0;
        tx_iov.clear();

        loop {
            let addr = vm_ipa2pa(active_vm().unwrap(), vq.desc_addr(idx));
            if addr == 0 {
                println!("virtio_net_notify_handler: failed to desc addr");
                return false;
            }
            tx_iov.push_data(addr, vq.desc_len(idx) as usize);

            len += vq.desc_len(idx) as usize;
            if vq.desc_flags(idx) == 0 {
                break;
            }
            idx = vq.desc_next(idx) as usize;
        }

        let trgt_vmid_map = ethernet_transmit(tx_iov.clone(), len).1;
        if trgt_vmid_map != 0 {
            vms_to_notify |= trgt_vmid_map;
        }

        if vm.id() != 0 {
            let used_addr = vm_ipa2pa(vm.clone(), vq.used_addr());
            if *VM_STATE_FLAG.lock() == 1 {
                println!("vm1 virtio net write memory in 0x{:x}", used_addr);
            }
            vm_if_set_mem_map_bit(vm.clone(), used_addr);
            vm_if_set_mem_map_bit(vm.clone(), used_addr + PAGE_SIZE);
        }
        if !vq.update_used_ring(
            (len - size_of::<VirtioNetHdr>()) as u32,
            next_desc_idx_opt.unwrap() as u32,
        ) {
            return false;
        }

        next_desc_idx_opt = vq.pop_avail_desc_idx(vq.avail_idx());
    }

    if !vq.avail_is_avail() {
        println!("invalid descriptor table index");
        return false;
    }

    nic.notify(vm.clone());
    // vq.notify(dev.int_id(), vm.clone());
    let mut trgt_vmid = 0;
    while vms_to_notify > 0 {
        if vms_to_notify & 1 != 0 {
            let vm = match crate::kernel::vm(trgt_vmid) {
                None => {
                    println!(
                        "virtio_net_notify_handler: target vm [{}] is not ready or not exist",
                        trgt_vmid
                    );
                    return true;
                }
                Some(_vm) => _vm,
            };
            let vcpu = vm.vcpu(0).unwrap();
            if vcpu.phys_id() == current_cpu().id {
                let nic = match vm.emu_net_dev(0) {
                    EmuDevs::VirtioNet(x) => x,
                    _ => {
                        println!("virtio_net_notify_handler: failed to get virtio net dev");
                        return false;
                    }
                };
                let rx_vq = match nic.vq(0) {
                    Ok(x) => x,
                    Err(_) => {
                        println!(
                            "virtio_net_notify_handler: vm[{}] failed to get virtio net rx virt queue",
                            vm.id()
                        );
                        return false;
                    }
                };
                if rx_vq.ready() != 0 && rx_vq.avail_flags() == 0 {
                    nic.notify(vm.clone());
                    // rx_vq.notify(nic.dev().int_id(), vm.clone());
                }
            } else {
                let msg = IpiEthernetMsg {
                    src_vmid: active_vm_id(),
                    trgt_vmid,
                };
                let cpu_trgt = vm_if_get_cpu_id(trgt_vmid);
                if !ipi_send_msg(cpu_trgt, IpiType::IpiTEthernetMsg, IpiInnerMsg::EnternetMsg(msg)) {
                    println!(
                        "virtio_net_notify_handler: failed to send ipi message, target {}",
                        cpu_trgt
                    );
                }
            }
        }

        trgt_vmid += 1;
        vms_to_notify = vms_to_notify >> 1;
    }
    true
}

pub fn ethernet_ipi_rev_handler(msg: &IpiMessage) {
    match msg.ipi_message {
        IpiInnerMsg::EnternetMsg(ethernet_msg) => {
            let trgt_vmid = ethernet_msg.trgt_vmid;
            let vm = match vm(trgt_vmid) {
                None => {
                    println!(
                        "ethernet_ipi_rev_handler: target vm [{}] is not ready or not exist",
                        trgt_vmid
                    );
                    return;
                }
                Some(_vm) => _vm,
            };
            let nic = match vm.emu_net_dev(0) {
                EmuDevs::VirtioNet(x) => x,
                _ => {
                    // println!(
                    //     "ethernet_ipi_rev_handler: vm[{}] failed to get virtio net dev",
                    //     vm.id()
                    // );
                    return;
                }
            };
            let rx_vq = match nic.vq(0) {
                Ok(x) => x,
                Err(_) => {
                    println!(
                        "ethernet_ipi_rev_handler: vm[{}] failed to get virtio net rx virt queue",
                        vm.id()
                    );
                    return;
                }
            };

            if rx_vq.ready() != 0 && rx_vq.avail_flags() == 0 {
                nic.notify(vm);
                // rx_vq.notify(nic.dev().int_id(), vm);
            }
        }
        _ => {
            panic!("illegal ipi message type in ethernet_ipi_rev_handler");
        }
    }
}

fn ethernet_transmit(tx_iov: VirtioIov, len: usize) -> (bool, usize) {
    // [ destination MAC - 6 ][ source MAC - 6 ][ EtherType - 2 ][ Payload ]
    if len < size_of::<VirtioNetHdr>() || len - size_of::<VirtioNetHdr>() < 6 + 6 + 2 {
        println!(
            "Too short for an ethernet frame, len {}, size of head {}",
            len,
            size_of::<VirtioNetHdr>()
        );
        return (false, 0);
    }

    let frame: &[u8] = tx_iov.get_ptr(size_of::<VirtioNetHdr>());
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
            return (false, 0);
        }
        return ethernet_broadcast(tx_iov.clone(), len);
    }

    if frame[0] == 0x33 && frame[1] == 0x33 {
        if !(frame[12] == 0x86 && frame[13] == 0xdd) {
            // Only IPV6 multicast packet is allowed to be broadcast
            return (false, 0);
        }
        return ethernet_broadcast(tx_iov.clone(), len);
    }

    match ethernet_mac_to_vm_id(frame) {
        Ok(vm_id) => {
            return (ethernet_send_to(vm_id, tx_iov.clone(), len), 1 << vm_id);
        }
        Err(_) => {
            return (false, 0);
        }
    }
}

fn ethernet_broadcast(tx_iov: VirtioIov, len: usize) -> (bool, usize) {
    let vm_num = vm_num();
    let cur_vm_id = active_vm_id();
    let mut trgt_vmid_map = 0;
    for vm_id in 0..vm_num {
        if vm_id == cur_vm_id {
            continue;
        }
        if vm_type(vm_id) as usize != 0 {
            continue;
        }
        if !ethernet_send_to(vm_id, tx_iov.clone(), len) {
            continue;
        }
        trgt_vmid_map |= 1 << vm_id;
    }
    (trgt_vmid_map != 0, trgt_vmid_map)
}

fn ethernet_send_to(vmid: usize, tx_iov: VirtioIov, len: usize) -> bool {
    // println!("ethernet send to vm{}", vmid);
    let vm = match vm(vmid) {
        None => {
            // println!("ethernet_send_to: target vm [{}] is not ready or not exist", vmid);
            return true;
        }
        Some(vm) => vm,
    };
    let nic = match vm.emu_net_dev(0) {
        EmuDevs::VirtioNet(x) => x,
        _ => {
            // println!("ethernet_send_to: vm[{}] failed to get virtio net dev", vmid);
            return true;
        }
    };

    if !nic.dev().activated() {
        // println!("ethernet_send_to: vm[{}] nic dev is not activate", vmid);
        return false;
    }

    let rx_vq = match nic.vq(0) {
        Ok(x) => x,
        Err(_) => {
            println!(
                "ethernet_send_to: vm[{}] failed to get virtio net rx virt queue",
                vm.id()
            );
            return false;
        }
    };

    let desc_header_idx_opt = rx_vq.pop_avail_desc_idx(rx_vq.avail_idx());
    if !rx_vq.avail_is_avail() {
        println!("ethernet_send_to: receive invalid avail desc idx");
        return false;
    } else if desc_header_idx_opt.is_none() {
        // println!("ethernet_send_to: desc_header_idx_opt is none");
        return false;
    }

    let desc_idx_header = desc_header_idx_opt.unwrap();
    let mut desc_idx = desc_header_idx_opt.unwrap() as usize;
    let rx_iov = VirtioIov::default();
    let mut rx_len = 0;

    loop {
        let dst = vm_ipa2pa(vm.clone(), rx_vq.desc_addr(desc_idx));
        if dst == 0 {
            println!(
                "rx_vq desc base table addr 0x{:x}, idx {}, avail table addr 0x{:x}, avail last idx {}",
                rx_vq.desc_table_addr(),
                desc_idx,
                rx_vq.avail_addr(),
                rx_vq.avail_idx()
            );
            println!("ethernet_send_to: failed to get dst {}", vmid);
            return false;
        }
        let desc_len = rx_vq.desc_len(desc_idx) as usize;

        if vmid != 0 {
            let mut addr = round_down(dst, PAGE_SIZE);
            if *VM_STATE_FLAG.lock() == 1 {
                println!("A: vm0 virtio net write vm1 memory in 0x{:x}", addr);
            }
            while addr <= round_down(dst + desc_len, PAGE_SIZE) {
                vm_if_set_mem_map_bit(vm.clone(), addr);
                addr += PAGE_SIZE;
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
        println!("ethernet_send_to: rx_len smaller than tx_len");
        return false;
    }
    if trace() && tx_iov.get_buf(0) < 0x1000 {
        panic!("illegal header addr {}", tx_iov.get_buf(0));
    }
    let header = unsafe { &mut *(tx_iov.get_buf(0) as *mut VirtioNetHdr) };
    header.num_buffers = 1;

    if tx_iov.write_through_iov(rx_iov.clone(), len) > 0 {
        println!(
            "ethernet_send_to: write through iov failed, rx_iov_num {} tx_iov_num {} rx_len {} tx_len {}",
            rx_iov.num(),
            tx_iov.num(),
            rx_len,
            len
        );
        return false;
    }

    if vmid != 0 {
        let used_addr = vm_ipa2pa(vm.clone(), rx_vq.used_addr());
        if *VM_STATE_FLAG.lock() == 1 {
            println!("B: vm0 virtio net write vm1 memory in 0x{:x}", used_addr);
        }
        vm_if_set_mem_map_bit(vm.clone(), used_addr);
        vm_if_set_mem_map_bit(vm.clone(), used_addr + PAGE_SIZE);
    }
    if !rx_vq.update_used_ring(len as u32, desc_idx_header as u32) {
        return false;
    }

    return true;
}

fn ethernet_is_arp(frame: &[u8]) -> bool {
    return frame[12] == 0x8 && frame[13] == 0x6;
}

fn ethernet_mac_to_vm_id(frame: &[u8]) -> Result<usize, ()> {
    for vm in VM_LIST.lock().iter() {
        let vm_id = vm.id();
        if vm_if_cmp_mac(vm_id, frame) {
            return Ok(vm_id);
        }
    }
    return Err(());
}

pub fn virtio_net_announce(vm: Vm) {
    match vm.emu_net_dev(0) {
        EmuDevs::VirtioNet(nic) => match nic.dev().desc() {
            DevDesc::NetDesc(desc) => {
                let status = desc.status();
                desc.set_status(status | VIRTIO_NET_S_ANNOUNCE);
                nic.notify_config(vm);
            }
            _ => {}
        },
        _ => {}
    }
}
