use alloc::vec::Vec;

use spin::Mutex;

use crate::device::{BLK_IRQ, virtio_blk_notify_handler, VIRTIO_BLK_T_IN, VIRTIO_BLK_T_OUT};
use crate::kernel::{
    active_vm, active_vm_id, finish_task, hvc_send_msg_to_vm, HvcGuestMsg, interrupt_vm_inject,
    io_task_head, IpiInnerMsg, TaskType, vm, vm_ipa2pa,
};
use crate::kernel::{ipi_register, IpiMessage, IpiType};
use crate::lib::{memcpy_safe, trace};

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
        if trace() && self.base_addr < 0x1000 {
            panic!("illeagal addr {:x}", self.base_addr);
        }
        unsafe { &mut *(self.base_addr as *mut MediatedBlkContent) }
    }

    pub fn dma_block_max(&self) -> usize {
        self.content().cfg.dma_block_max
    }

    pub fn nreq(&self) -> usize {
        self.content().nreq
    }

    pub fn cache_ipa(&self) -> usize {
        self.content().cfg.cache_ipa
    }

    pub fn cache_pa(&self) -> usize {
        self.content().cfg.cache_pa
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
    if !ipi_register(IpiType::IpiTMediatedDev, mediated_ipi_handler) {
        panic!("mediated_dev_init: failed to register ipi IpiTMediatedDev");
    }
    if !ipi_register(IpiType::IpiTMediatedNotify, mediated_notify_ipi_handler) {
        panic!("mediated_dev_init: failed to register ipi IpiTMediatedNotify");
    }
}

pub fn mediated_dev_append(_class_id: usize, mmio_ipa: usize) -> bool {
    let vm = active_vm().unwrap();
    let blk_pa = vm_ipa2pa(vm.clone(), mmio_ipa);
    let mediated_blk = MediatedBlk { base_addr: blk_pa };
    mediated_blk.set_nreq(0);

    let cache_pa = vm_ipa2pa(vm.clone(), mediated_blk.cache_ipa());
    println!(
        "mediated_dev_append: cache ipa {:x}, cache_pa {:x}",
        mediated_blk.cache_ipa(),
        cache_pa
    );
    mediated_blk.set_cache_pa(cache_pa);
    mediated_blk_list_push(mediated_blk);
    true
}

pub fn mediated_blk_notify_handler(_dev_ipa_reg: usize) -> bool {
    // println!("mediated_blk notify");
    let mediated_blk = mediated_blk_list_get(0);
    let mut cache_ptr = mediated_blk.cache_pa();
    let io_task = io_task_head();
    if io_task.is_none() {
        return false;
    }

    match io_task.unwrap().task_type {
        TaskType::MediatedIpiTask(_) => {
            panic!("illegal io task type");
        }
        TaskType::MediatedIoTask(task) => {
            match task.io_type {
                VIRTIO_BLK_T_IN => {
                    // let mut sum = 0;
                    for idx in 0..task.iov_list.len() {
                        let data_bg = task.iov_list[idx].data_bg;
                        let len = task.iov_list[idx].len as usize;
                        if trace() && (data_bg < 0x1000 || cache_ptr < 0x1000) {
                            panic!("illegal des addr {:x}, src addr {:x}", data_bg, cache_ptr);
                        }
                        memcpy_safe(data_bg as *mut u8, cache_ptr as *mut u8, len);
                        // sum |= check_sum(data_bg, len);
                        cache_ptr += len;
                    }
                    // println!("read check_sum is {:x}", sum);
                }
                VIRTIO_BLK_T_OUT => {
                    // println!("notify write");
                }
                _ => {}
            }
        }
    }

    finish_task(false);
    true
}

pub fn mediated_notify_ipi_handler(_msg: &IpiMessage) {
    let vm = active_vm().unwrap();
    // println!("vm[{}] inject blk irq", vm.id());
    interrupt_vm_inject(vm.clone(), vm.vcpu(0), BLK_IRQ, 0);
}

fn check_sum(addr: usize, len: usize) -> usize {
    let slice = unsafe { core::slice::from_raw_parts(addr as *const usize, len / 8) };
    let mut sum = 0;
    for num in slice {
        sum ^= num;
    }
    sum
}

pub fn mediated_ipi_handler(msg: &IpiMessage) {
    // println!("vm {} mediated_ipi_handler", active_vm_id());
    match &msg.ipi_message {
        IpiInnerMsg::MediatedMsg(mediated_msg) => {
            let src_id = mediated_msg.src_id;
            let vm = vm(src_id);
            virtio_blk_notify_handler(mediated_msg.vq.clone(), mediated_msg.blk.clone(), vm, mediated_msg.avail_idx);
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

    // println!("mediated blk read: nreq {}, type {}, sector {}, count {}", nreq + 1, VIRTIO_BLK_T_IN, sector, count);

    let med_read_msg = HvcGuestMsg {
        fid: 3,    // HVC_MEDIATED
        event: 50, // HVC_MEDIATED_DEV_NOTIFY
    };

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
    // println!("mediated blk write: nreq {}, type {}, sector {}, count {}", nreq + 1, VIRTIO_BLK_T_OUT, sector, count);

    let med_read_msg = HvcGuestMsg {
        fid: 3,    // HVC_MEDIATED
        event: 50, // HVC_MEDIATED_DRV_NOTIFY
    };

    // println!("mediated_blk_write send msg to vm0");
    if !hvc_send_msg_to_vm(0, &med_read_msg) {
        println!("mediated_blk_write: failed to notify VM 0");
    }
}
