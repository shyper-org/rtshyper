use crate::arch::PAGE_SIZE;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

use crate::kernel::{
    active_vm, active_vm_id, add_task, finish_ipi_task, finish_task, io_list_len, ipi_list_len,
    vm_ipa2pa, IoMediatedMsg, IpiMediatedMsg, Task, TaskType, Vm,
};

pub const VIRTQUEUE_BLK_MAX_SIZE: usize = 256;
pub const VIRTQUEUE_NET_MAX_SIZE: usize = 256;

/* VIRTIO_BLK_FEATURES*/
pub const VIRTIO_BLK_F_SIZE_MAX: usize = 1 << 1;
pub const VIRTIO_BLK_F_SEG_MAX: usize = 1 << 2;

/* BLOCK PARAMETERS*/
pub const SECTOR_BSIZE: usize = 512;
pub const BLOCKIF_IOV_MAX: usize = 64;

/* BLOCK REQUEST TYPE*/
pub const VIRTIO_BLK_T_IN: usize = 0;
pub const VIRTIO_BLK_T_OUT: usize = 1;
// pub const VIRTIO_BLK_T_FLUSH: usize = 4;
pub const VIRTIO_BLK_T_GET_ID: usize = 8;

/* BLOCK REQUEST STATUS*/
pub const VIRTIO_BLK_S_OK: usize = 0;
// pub const VIRTIO_BLK_S_IOERR: usize = 1;
pub const VIRTIO_BLK_S_UNSUPP: usize = 2;

#[repr(C)]
struct BlkGeometry {
    cylinders: u16,
    heads: u8,
    sectors: u8,
}

impl BlkGeometry {
    fn default() -> BlkGeometry {
        BlkGeometry {
            cylinders: 0,
            heads: 0,
            sectors: 0,
        }
    }
}

#[repr(C)]
struct BlkTopology {
    // # of logical blocks per physical block (log2)
    physical_block_exp: u8,
    // offset of first aligned logical block
    alignment_offset: u8,
    // suggested minimum I/O size in blocks
    min_io_size: u16,
    // optimal (suggested maximum) I/O size in blocks
    opt_io_size: u32,
}

impl BlkTopology {
    fn default() -> BlkTopology {
        BlkTopology {
            physical_block_exp: 0,
            alignment_offset: 0,
            min_io_size: 0,
            opt_io_size: 0,
        }
    }
}

#[derive(Clone)]
pub struct BlkDesc {
    inner: Arc<Mutex<BlkDescInner>>,
}

impl BlkDesc {
    pub fn default() -> BlkDesc {
        BlkDesc {
            inner: Arc::new(Mutex::new(BlkDescInner::default())),
        }
    }

    pub fn cfg_init(&self, bsize: usize) {
        let mut inner = self.inner.lock();
        inner.cfg_init(bsize);
    }

    pub fn start_addr(&self) -> usize {
        let inner = self.inner.lock();
        &inner.capacity as *const _ as usize
    }

    pub fn offset_data(&self, offset: usize) -> u32 {
        let inner = self.inner.lock();
        let start_addr = &inner.capacity as *const _ as usize;
        let value = unsafe { *((start_addr + offset) as *const u32) };
        return value;
    }
}

#[repr(C)]
pub struct BlkDescInner {
    capacity: usize,
    size_max: u32,
    seg_max: u32,
    geometry: BlkGeometry,
    blk_size: usize,
    topology: BlkTopology,
    writeback: u8,
    unused0: [u8; 3],
    max_discard_sectors: u32,
    max_discard_seg: u32,
    discard_sector_alignment: u32,
    max_write_zeroes_sectors: u32,
    max_write_zeroes_seg: u32,
    write_zeroes_may_unmap: u8,
    unused1: [u8; 3],
}

impl BlkDescInner {
    pub fn default() -> BlkDescInner {
        BlkDescInner {
            capacity: 0,
            size_max: 0,
            seg_max: 0,
            geometry: BlkGeometry::default(),
            blk_size: 0,
            topology: BlkTopology::default(),
            writeback: 0,
            unused0: [0; 3],
            max_discard_sectors: 0,
            max_discard_seg: 0,
            discard_sector_alignment: 0,
            max_write_zeroes_sectors: 0,
            max_write_zeroes_seg: 0,
            write_zeroes_may_unmap: 0,
            unused1: [0; 3],
        }
    }

    pub fn cfg_init(&mut self, bsize: usize) {
        self.capacity = bsize;
        self.size_max = PAGE_SIZE as u32;
        self.seg_max = BLOCKIF_IOV_MAX as u32;
    }
}

#[repr(C)]
#[derive(Clone)]
pub struct BlkIov {
    pub data_bg: usize,
    pub len: u32,
}

#[repr(C)]
pub struct BlkReqRegion {
    pub start: usize,
    pub size: usize,
}

#[derive(Clone)]
pub struct VirtioBlkReq {
    inner: Arc<Mutex<VirtioBlkReqInner>>,
}

impl VirtioBlkReq {
    pub fn default() -> VirtioBlkReq {
        VirtioBlkReq {
            inner: Arc::new(Mutex::new(VirtioBlkReqInner::default())),
        }
    }

    pub fn set_start(&self, start: usize) {
        let mut inner = self.inner.lock();
        inner.set_start(start);
    }

    pub fn set_size(&self, size: usize) {
        let mut inner = self.inner.lock();
        inner.set_size(size);
    }

    pub fn set_mediated(&self, mediated: bool) {
        let mut inner = self.inner.lock();
        inner.mediated = mediated;
    }

    pub fn mediated(&self) -> bool {
        let inner = self.inner.lock();
        inner.mediated
    }

    pub fn reset_blk_iov(&self) {
        let mut inner = self.inner.lock();
        inner.iov_total = 0;
        inner.iov.clear();
    }

    pub fn set_type(&self, req_type: u32) {
        let mut inner = self.inner.lock();
        inner.req_type = req_type;
    }

    pub fn set_sector(&self, sector: usize) {
        let mut inner = self.inner.lock();
        inner.sector = sector;
    }

    pub fn push_iov(&self, iov: BlkIov) {
        let mut inner = self.inner.lock();
        inner.iov.push(iov);
    }

    pub fn add_iov_total(&self, data: usize) {
        let mut inner = self.inner.lock();
        inner.iov_total += data;
    }

    pub fn req_type(&self) -> u32 {
        let inner = self.inner.lock();
        inner.req_type
    }

    pub fn sector(&self) -> usize {
        let inner = self.inner.lock();
        inner.sector
    }

    pub fn region_start(&self) -> usize {
        let inner = self.inner.lock();
        inner.region.start
    }

    pub fn region_size(&self) -> usize {
        let inner = self.inner.lock();
        inner.region.size
    }

    pub fn iov_total(&self) -> usize {
        let inner = self.inner.lock();
        inner.iov_total
    }

    pub fn iovn(&self) -> usize {
        let inner = self.inner.lock();
        inner.iov.len()
    }

    pub fn iov_data_bg(&self, idx: usize) -> usize {
        let inner = self.inner.lock();
        inner.iov[idx].data_bg
    }

    pub fn iov_len(&self, idx: usize) -> u32 {
        let inner = self.inner.lock();
        inner.iov[idx].len
    }
}

#[derive(Clone)]
pub struct MediatedBlkReqInner {
    pub req_type: usize,
    pub blk_id: usize,
    pub sector: usize,
    pub count: usize,
    pub iov: Vec<BlkIov>,
}

#[repr(C)]
struct VirtioBlkReqInner {
    req_type: u32,
    reserved: u32,
    sector: usize,
    iov: Vec<BlkIov>,
    iov_total: usize,
    region: BlkReqRegion,
    mediated: bool,
    // mediated_req: Vec<MediatedBlkReq>,
    // mediated_notify: Vec<MediatedBlkReq>,
    process_list: Vec<usize>,
}

impl VirtioBlkReqInner {
    pub fn default() -> VirtioBlkReqInner {
        VirtioBlkReqInner {
            req_type: 0,
            reserved: 0,
            sector: 0,
            iov: Vec::new(),
            iov_total: 0,
            region: BlkReqRegion { start: 0, size: 0 },
            mediated: false,
            // mediated_req: Vec::new(),
            // mediated_notify: Vec::new(),
            process_list: Vec::new(),
        }
    }

    pub fn set_start(&mut self, start: usize) {
        self.region.start = start;
    }

    pub fn set_size(&mut self, size: usize) {
        self.region.size = size;
    }
}

use crate::lib::memcpy;

pub fn blk_req_handler(req: VirtioBlkReq, cache: usize) -> usize {
    // println!("vm[{}] blk req handler", active_vm_id());
    let sector = req.sector();
    let region_start = req.region_start();
    let region_size = req.region_size();
    let mut total_byte = 0;
    let mut cache_ptr = cache;

    if sector + req.iov_total() / SECTOR_BSIZE > region_start + region_size {
        println!(
            "blk_req_handler: {} out of vm range",
            if req.req_type() == VIRTIO_BLK_T_IN as u32 {
                "read"
            } else {
                "write"
            }
        );
        return 0;
    }
    match req.req_type() as usize {
        VIRTIO_BLK_T_IN => {
            if req.mediated() {
                let mut iov_list = vec![];
                for iov_idx in 0..req.iovn() {
                    iov_list.push(BlkIov {
                        data_bg: req.iov_data_bg(iov_idx),
                        len: req.iov_len(iov_idx),
                    });
                }
                // mediated blk read
                add_task(Task {
                    task_type: TaskType::MediatedIoTask(IoMediatedMsg {
                        io_type: VIRTIO_BLK_T_IN,
                        blk_id: 0,
                        sector: sector + region_start,
                        count: req.iov_total() / SECTOR_BSIZE,
                        cache,
                        iov_list: Arc::new(iov_list),
                    }),
                });
            } else {
                todo!();
                // platform_blk_read(sector + region_start, req.iov_total() / SECTOR_BSIZE, cache);
            }
            for iov_idx in 0..req.iovn() {
                let data_bg = req.iov_data_bg(iov_idx);
                let len = req.iov_len(iov_idx) as usize;

                if len < SECTOR_BSIZE {
                    println!("blk_req_handler: read len < SECTOR_BSIZE");
                    return 0;
                }
                if !req.mediated() {
                    unsafe {
                        memcpy(data_bg as *mut u8, cache_ptr as *mut u8, len);
                    }
                }
                cache_ptr += len;
                total_byte += len;
            }
        }
        VIRTIO_BLK_T_OUT => {
            for iov_idx in 0..req.iovn() {
                let data_bg = req.iov_data_bg(iov_idx);
                let len = req.iov_len(iov_idx) as usize;
                if len < SECTOR_BSIZE {
                    println!("blk_req_handler: read len < SECTOR_BSIZE");
                    return 0;
                }
                if !req.mediated() {
                    unsafe {
                        memcpy(cache_ptr as *mut u8, data_bg as *mut u8, len);
                    }
                }
                cache_ptr += len;
                total_byte += len;
            }
            if req.mediated() {
                let mut iov_list = vec![];
                for iov_idx in 0..req.iovn() {
                    iov_list.push(BlkIov {
                        data_bg: req.iov_data_bg(iov_idx),
                        len: req.iov_len(iov_idx),
                    });
                }
                // mediated blk write
                add_task(Task {
                    task_type: TaskType::MediatedIoTask(IoMediatedMsg {
                        io_type: VIRTIO_BLK_T_OUT,
                        blk_id: 0,
                        sector: sector + region_start,
                        count: req.iov_total() / SECTOR_BSIZE,
                        cache,
                        iov_list: Arc::new(iov_list),
                    }),
                });
            } else {
                todo!();
                // platform_blk_write(sector + region_start, req.iov_total() / SECTOR_BSIZE, cache);
            }
        }
        VIRTIO_BLK_T_GET_ID => {
            if req.mediated() {
                add_task(Task {
                    task_type: TaskType::MediatedIoTask(IoMediatedMsg {
                        io_type: VIRTIO_BLK_T_IN,
                        blk_id: 0,
                        sector: region_start,
                        count: 0,
                        cache,
                        iov_list: Arc::new(Vec::new()),
                    }),
                });
            }
            let data_bg = req.iov_data_bg(0);
            let name = "virtio-blk".as_ptr();
            unsafe {
                memcpy(data_bg as *mut u8, name, 20);
            }
            total_byte = 20;
        }
        _ => {
            println!("Wrong block request type {} ", req.req_type());
            return 0;
        }
    }
    return total_byte;
}

use crate::device::{mediated_blk_list_get, VirtioMmio, Virtq};

#[no_mangle]
pub fn virtio_mediated_blk_notify_handler(vq: Virtq, blk: VirtioMmio, _vm: Vm, _idx: u16) -> bool {
    // if vq.avail_flags() != 0 {
    let idx = vq.avail_idx();
    let flag = vq.avail_flags();
    // println!(
    //     "vm{} add ipi task, vq avail_idx {}, avail_flag {}",
    //     active_vm_id(),
    //     idx,
    //     flag
    // );
    // }
    add_task(Task {
        task_type: TaskType::MediatedIpiTask(IpiMediatedMsg {
            src_id: active_vm_id(),
            vq: vq.clone(),
            blk: blk.clone(),
            notify: flag == 0,
            avail_idx: idx,
        }),
    });
    true
}

pub fn virtio_blk_notify_handler(vq: Virtq, blk: VirtioMmio, vm: Vm, avail_idx: u16) -> bool {
    // println!("vm[{}] in virtio_blk_notify_handler, avail idx {}", active_vm_id(), avail_idx);
    // use crate::kernel::active_vm;
    // let vm = active_vm().unwrap();

    if vq.ready() == 0 {
        println!("blk virt_queue is not ready!");
        return false;
    }

    // let mediated = blk.mediated();
    let dev = blk.dev();
    let req = match dev.req() {
        super::DevReq::BlkReq(blk_req) => blk_req,
        _ => {
            panic!("virtio_blk_notify_handler: illegal req");
        }
    };

    let vq_size = vq.num();
    let mut next_desc_idx_opt = vq.pop_avail_desc_idx(avail_idx);
    let mut process_count: i32 = 0;
    let mut desc_chain_head_idx;

    while next_desc_idx_opt.is_some() {
        let mut next_desc_idx = next_desc_idx_opt.unwrap() as usize;
        vq.disable_notify();
        if vq.check_avail_idx(avail_idx) {
            vq.enable_notify();
        }

        let mut head = true;
        desc_chain_head_idx = next_desc_idx;
        req.reset_blk_iov();

        // println!("desc_chain_head {}", desc_chain_head_idx);
        // vq.show_desc_info(4);

        loop {
            // println!("next desc idx {}", next_desc_idx);
            if vq.desc_has_next(next_desc_idx) {
                if head {
                    if vq.desc_is_writable(next_desc_idx) {
                        println!(
                            "Failed to get virt blk queue desc header, idx = {}",
                            next_desc_idx
                        );
                        vq.notify(dev.int_id(), vm.clone());
                        return false;
                    }
                    head = false;
                    let vreq_addr = vm_ipa2pa(vm.clone(), vq.desc_addr(next_desc_idx));
                    if vreq_addr == 0 {
                        println!("virtio_blk_notify_handler: failed to get vreq");
                        return false;
                    }
                    let vreq = unsafe { &mut *(vreq_addr as *mut VirtioBlkReqInner) };
                    // println!("type {}", vreq.req_type);
                    // println!("sector {}", vreq.sector);
                    req.set_type(vreq.req_type);
                    req.set_sector(vreq.sector);
                } else {
                    /*data handler*/
                    // println!("data handler");
                    if (vq.desc_flags(next_desc_idx) & 0x2) as u32 >> 1 == req.req_type() {
                        println!(
                            "Failed to get virt blk queue desc data, idx = {}, req.type = {}, desc.flags = {}",
                            next_desc_idx, req.req_type(), vq.desc_flags(next_desc_idx)
                        );
                        vq.notify(dev.int_id(), vm.clone());
                        return false;
                    }
                    let data_bg = vm_ipa2pa(vm.clone(), vq.desc_addr(next_desc_idx));
                    if data_bg == 0 {
                        println!("virtio_blk_notify_handler: failed to get iov data begin");
                        return false;
                    }

                    let iov = BlkIov {
                        data_bg,
                        len: vq.desc_len(next_desc_idx),
                    };
                    req.add_iov_total(iov.len as usize);
                    req.push_iov(iov);
                }
            } else {
                /*state handler*/
                // println!("state handler");
                if !vq.desc_is_writable(next_desc_idx) {
                    println!(
                        "Failed to get virt blk queue desc status, idx = {}",
                        next_desc_idx,
                    );
                    vq.notify(dev.int_id(), vm.clone());
                    return false;
                }
                let vstatus_addr = vm_ipa2pa(vm.clone(), vq.desc_addr(next_desc_idx));
                if vstatus_addr == 0 {
                    println!(
                        "virtio_blk_notify_handler: vm[{}] failed to vstatus",
                        vm.id()
                    );
                    return false;
                }
                let vstatus = unsafe { &mut *(vstatus_addr as *mut u8) };
                if req.req_type() > 1 && req.req_type() != VIRTIO_BLK_T_GET_ID as u32 {
                    *vstatus = VIRTIO_BLK_S_UNSUPP as u8;
                } else {
                    *vstatus = VIRTIO_BLK_S_OK as u8;
                }
                break;
            }
            next_desc_idx = vq.desc_next(next_desc_idx) as usize;
        }

        let total = if !req.mediated() {
            blk_req_handler(req.clone(), dev.cache())
        } else {
            let mediated_blk = mediated_blk_list_get(0);
            let cache = mediated_blk.cache_pa();
            blk_req_handler(req.clone(), cache)
        };
        if !vq.update_used_ring(total as u32, desc_chain_head_idx as u32, vq_size) {
            return false;
        }
        // println!("finish blk req handler");
        // match dev.stat() {
        //     super::DevStat::BlkStat(stat) => match req.req_type() as usize {
        //         VIRTIO_BLK_T_IN => {
        //             let read_req = stat.read_req() + 1;
        //             stat.set_read_req(read_req);
        //             let read_byte = stat.read_byte() + total;
        //             stat.set_read_byte(read_byte);
        //         }
        //         VIRTIO_BLK_T_OUT => {
        //             let write_req = stat.write_req() + 1;
        //             stat.set_write_req(write_req);
        //             let write_byte = stat.write_byte() + total;
        //             stat.set_write_byte(write_byte);
        //         }
        //         _ => {}
        //     },
        //     _ => {
        //         panic!("virtio_blk_notify_handler: illegal dev stat type");
        //     }
        // }

        process_count += 1;
        next_desc_idx_opt = vq.pop_avail_desc_idx(avail_idx);
    }

    if vq.avail_flags() == 0 && process_count > 0 && !req.mediated() {
        panic!("illegal");
        vq.notify(dev.int_id(), vm.clone());
    }
    if req.mediated() && process_count <= 0 {
        finish_ipi_task();
    }

    return true;
}
