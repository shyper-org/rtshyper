use crate::arch::PTE_S2_NORMAL;
use crate::kernel::{active_vm, active_vm_id, hvc_send_msg_to_vm, HvcGuestMsg, interrupt_vm_inject, IpiInnerMsg, vm, vm_ipa2pa};
use crate::kernel::{ipi_register, IpiMessage, IpiType};
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;
use core::mem::size_of;
use crate::device::{VIRTIO_BLK_T_IN, VIRTIO_BLK_T_OUT};
use crate::lib::memcpy;

const BLK_IRQ: usize = 0x20 + 0x10;
static MEDIATED_BLK_LIST: Mutex<Vec<MediatedBlk>> = Mutex::new(Vec::new());

pub fn mediated_blk_list_push(blk: MediatedBlk) {
    let mut list = MEDIATED_BLK_LIST.lock();
    list.push(blk);
}

pub fn mediated_blk_list_get(idx: usize) -> MediatedBlk {
    let list = MEDIATED_BLK_LIST.lock();
    list[idx].clone()
}

#[derive(Clone)]
pub struct MediatedBlk {
    base_addr: usize,
}

impl MediatedBlk {
    pub fn content(&self) -> &mut MediatedBlkContent {
        unsafe {
            &mut *(self.base_addr as *mut MediatedBlkContent)
        }
    }

    pub fn nreq(&self) -> usize {
        self.content().nreq
    }

    pub fn cache_ipa(&self) -> usize {
        self.content().cfg.cache_ipa
    }

    pub fn cache_pa(&self) -> usize {
        self.content().cfg.cache_ipa
    }

    pub fn set_nreq(&self, nreq: usize) {
        self.content().nreq = nreq;
    }

    pub fn set_type(&self, req_type: usize) {
        self.content().req.req_type = req_type as u32;
    }

    pub fn set_sector(&self, sector: usize) {
        self.content().req.sector = sector;
    }

    pub fn set_count(&self, count: usize) {
        self.content().req.count = count;
    }

    pub fn set_cache_pa(&self, cache_pa: usize) {
        self.content().cfg.cache_pa = cache_pa;
    }
}

#[repr(C)]
pub struct MediatedBlkContent {
    nreq: usize,
    cfg: MediatedBlkCfg,
    req: MediatedBlkReq,
}

#[repr(C)]
pub struct MediatedBlkCfg {
    name: [u8; 32],
    block_dev_path: [u8; 32],
    block_num: usize,
    dma_block_max: usize,
    cache_size: usize,
    idx: u16,
    // TODO: enable page cache
    pcache: bool,
    cache_va: usize,
    cache_ipa: usize,
    cache_pa: usize,
}

#[repr(C)]
pub struct MediatedBlkReq {
    req_type: u32,
    sector: usize,
    count: usize,
}

pub fn mediated_dev_init() {
    println!("register meidated ipi");
    if !ipi_register(IpiType::IpiTMediatedDev, mediated_ipi_handler) {
        panic!("mediated_dev_init: failed to register ipi IpiTMediatedDev", );
    }
}

pub fn mediated_dev_append(class_id: usize, mmio_ipa: usize) -> bool {
    let vm = active_vm().unwrap();
    let blk_pa = vm_ipa2pa(vm.clone(), mmio_ipa);
    let mediated_blk = MediatedBlk { base_addr: blk_pa };
    mediated_blk.set_nreq(0);

    let cache_pa = vm_ipa2pa(vm.clone(), mediated_blk.cache_ipa());
    mediated_blk.set_cache_pa(cache_pa);
    mediated_blk_list_push(mediated_blk);
    true
}

pub fn mediated_blk_notify_handler(dev_ipa_reg: usize) -> bool {
    let vm = vm(0);
    interrupt_vm_inject(vm, BLK_IRQ, active_vm_id());
    true
}

pub fn mediated_ipi_handler(msg: &IpiMessage) {
    println!("vm {} mediated_ipi_handler", active_vm_id());
    match &msg.ipi_message {
        IpiInnerMsg::MediatedMsg(mediated_msg) => {
            match mediated_msg.req_type {
                VIRTIO_BLK_T_IN => {
                    mediated_blk_read(mediated_msg.blk_id, mediated_msg.sector, mediated_msg.count);
                    let blk = mediated_blk_list_get(mediated_msg.blk_id);
                    let mut cache_ptr = blk.cache_pa();
                    for iov in &mediated_msg.iov_list {
                        let data_bg = iov.data_bg;
                        let len = iov.len as usize;
                        unsafe {
                            memcpy(data_bg as *mut u8, cache_ptr as *mut u8, len);
                        }
                        cache_ptr += len;
                    }
                }
                VIRTIO_BLK_T_OUT => {
                    let blk = mediated_blk_list_get(mediated_msg.blk_id);
                    let mut cache_ptr = blk.cache_pa();
                    for iov in &mediated_msg.iov_list {
                        let data_bg = iov.data_bg;
                        let len = iov.len as usize;
                        unsafe {
                            memcpy(cache_ptr as *mut u8, data_bg as *mut u8, len);
                        }
                        cache_ptr += len;
                    }
                    mediated_blk_write(mediated_msg.blk_id, mediated_msg.sector, mediated_msg.count);
                }
                _ => {
                    todo!();
                }
            }
        }
        _ => {}
    }
}

pub fn mediated_blk_read(blk_idx: usize, sector: usize, count: usize) {
    let mediated_blk = mediated_blk_list_get(blk_idx);
    let nreq = mediated_blk.nreq();
    mediated_blk.set_nreq(nreq + 1);
    mediated_blk.set_type(VIRTIO_BLK_T_IN);
    mediated_blk.set_sector(sector);
    mediated_blk.set_count(count);

    let med_read_msg = HvcGuestMsg {
        fid: 3,     // HVC_MEDIATED
        event: 50,  // HVC_MEDIATED_DEV_NOTIFY
    };

    println!("mediated_blk_read send msg to vm0");
    if !hvc_send_msg_to_vm(0, &med_read_msg) {
        println!("mediated_blk_read: failed to notify VM 0");
    }
}


pub fn mediated_blk_write(blk_idx: usize, sector: usize, count: usize) {
    let mediated_blk = mediated_blk_list_get(blk_idx);
    let nreq = mediated_blk.nreq();
    mediated_blk.set_nreq(nreq + 1);
    mediated_blk.set_type(VIRTIO_BLK_T_OUT);
    mediated_blk.set_sector(sector);
    mediated_blk.set_count(count);

    let med_read_msg = HvcGuestMsg {
        fid: 3,     // HVC_MEDIATED
        event: 50,  // HVC_MEDIATED_DRV_NOTIFY
    };

    println!("mediated_blk_write send msg to vm0");
    if !hvc_send_msg_to_vm(0, &med_read_msg) {
        println!("mediated_blk_read: failed to notify VM 0");
    }
}