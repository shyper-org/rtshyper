use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

use crate::arch::PAGE_SIZE;
use crate::device::{mediated_blk_list_get, VirtioMmio, Virtq};
use crate::kernel::{
    active_vm_id, add_async_task, async_blk_id_req, async_blk_io_req, async_ipi_req, AsyncTask, AsyncTaskData,
    AsyncTaskState, IoAsyncMsg, IoIdAsyncMsg, IpiMediatedMsg, push_used_info, Vm, vm_ipa2hva,
};
use crate::util::{memcpy_safe, trace};

pub const VIRTQUEUE_BLK_MAX_SIZE: usize = 256;

/* VIRTIO_BLK_FEATURES*/
pub const VIRTIO_BLK_F_SIZE_MAX: usize = 1 << 1;
pub const VIRTIO_BLK_F_SEG_MAX: usize = 1 << 2;

/* BLOCK PARAMETERS*/
pub const SECTOR_BSIZE: usize = 512;
pub const BLOCKIF_SIZE_MAX: usize = 128 * PAGE_SIZE;
pub const BLOCKIF_IOV_MAX: usize = 512;

/* BLOCK REQUEST TYPE*/
pub const VIRTIO_BLK_T_IN: usize = 0;
pub const VIRTIO_BLK_T_OUT: usize = 1;
pub const VIRTIO_BLK_T_FLUSH: usize = 4;
pub const VIRTIO_BLK_T_GET_ID: usize = 8;

/* BLOCK REQUEST STATUS*/
pub const VIRTIO_BLK_S_OK: usize = 0;
// pub const VIRTIO_BLK_S_IOERR: usize = 1;
pub const VIRTIO_BLK_S_UNSUPP: usize = 2;

#[repr(C)]
#[derive(Copy, Clone)]
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
#[derive(Copy, Clone)]
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
    pub fn back_up(&self) -> BlkDesc {
        let current_inner = self.inner.lock();
        let inner = *current_inner;
        BlkDesc {
            inner: Arc::new(Mutex::new(inner)),
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
        let start_addr = self.start_addr();
        if trace() && start_addr + offset < 0x1000 {
            panic!("illegal addr {:x}", start_addr + offset);
        }

        unsafe { *((start_addr + offset) as *const u32) }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
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
        self.size_max = BLOCKIF_SIZE_MAX as u32;
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
    req_list: Arc<Mutex<Vec<VirtioBlkReqNode>>>,
}

impl VirtioBlkReq {
    pub fn default() -> VirtioBlkReq {
        VirtioBlkReq {
            inner: Arc::new(Mutex::new(VirtioBlkReqInner::default())),
            req_list: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn back_up(&self) -> VirtioBlkReq {
        let current_inner = self.inner.lock();
        let current_req_list = self.req_list.lock();
        let inner = VirtioBlkReqInner {
            region: BlkReqRegion {
                start: current_inner.region.start,
                size: current_inner.region.size,
            },
            mediated: current_inner.mediated,
            process_list: {
                let mut list = vec![];
                for process in current_inner.process_list.iter() {
                    list.push(*process)
                }
                list
            },
        };
        let mut req_list = vec![];
        for req in current_req_list.iter() {
            req_list.push(VirtioBlkReqNode {
                req_type: req.req_type,
                reserved: req.reserved,
                sector: req.sector,
                desc_chain_head_idx: req.desc_chain_head_idx,
                iov: {
                    let mut iov = vec![];
                    for io in req.iov.iter() {
                        iov.push(BlkIov {
                            data_bg: io.data_bg,
                            len: io.len,
                        });
                    }
                    iov
                },
                iov_sum_up: req.iov_sum_up,
                iov_total: req.iov_total,
            });
        }
        VirtioBlkReq {
            inner: Arc::new(Mutex::new(inner)),
            req_list: Arc::new(Mutex::new(req_list)),
        }
    }

    pub fn add_req_node(&self, node: VirtioBlkReqNode, _vm: &Vm) {
        let mut list = self.req_list.lock();
        // let mediated_blk = mediated_blk_list_get(vm.med_blk_id());
        // push_used_info(node.desc_chain_head_idx, node.iov_total as u32, vm.id());
        list.push(node);

        // match list.last_mut() {
        //     None => {
        //         list.push(node);
        //     }
        //     Some(prev) => {
        //         if prev.req_type == node.req_type
        //             && (prev.sector + prev.iov_sum_up / SECTOR_BSIZE) == node.sector
        //             && (prev.iov_sum_up + node.iov_sum_up) / SECTOR_BSIZE < mediated_blk.dma_block_max()
        //         {
        //             prev.iov_sum_up += node.iov_sum_up;
        //             prev.iov.append(&mut node.iov);
        //         } else {
        //             list.push(node);
        //         }
        //     }
        // }
    }

    pub fn req_num(&self) -> usize {
        let list = self.req_list.lock();
        list.len()
    }

    pub fn req_node(&self, idx: usize) -> VirtioBlkReqNode {
        let list = self.req_list.lock();
        list[idx].clone()
    }

    pub fn clear_node(&self) {
        let mut list = self.req_list.lock();
        list.clear();
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

    pub fn region_start(&self) -> usize {
        let inner = self.inner.lock();
        inner.region.start
    }

    pub fn region_size(&self) -> usize {
        let inner = self.inner.lock();
        inner.region.size
    }
}

#[repr(C)]
#[derive(Clone)]
pub struct VirtioBlkReqNode {
    req_type: u32,
    reserved: u32,
    sector: usize,
    desc_chain_head_idx: u32,
    iov: Vec<BlkIov>,
    // sum up byte for req
    iov_sum_up: usize,
    // total byte for current req
    iov_total: usize,
}

impl VirtioBlkReqNode {
    pub fn default() -> VirtioBlkReqNode {
        VirtioBlkReqNode {
            req_type: 0,
            reserved: 0,
            sector: 0,
            desc_chain_head_idx: 0,
            iov: vec![],
            iov_sum_up: 0,
            iov_total: 0,
        }
    }
}

#[repr(C)]
struct VirtioBlkReqInner {
    region: BlkReqRegion,
    mediated: bool,
    process_list: Vec<usize>,
}

impl VirtioBlkReqInner {
    pub fn default() -> VirtioBlkReqInner {
        VirtioBlkReqInner {
            region: BlkReqRegion { start: 0, size: 0 },
            mediated: false,
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

pub fn generate_blk_req(req: &VirtioBlkReq, vq: &Virtq, dev: &VirtioMmio, cache: usize, vm: &Vm) {
    let region_start = req.region_start();
    let region_size = req.region_size();
    let mut cache_ptr = cache;
    for idx in 0..req.req_num() {
        let req_node = req.req_node(idx);
        let sector = req_node.sector;
        if sector + req_node.iov_sum_up / SECTOR_BSIZE > region_start + region_size {
            println!(
                "blk_req_handler: {} out of vm range",
                if req_node.req_type == VIRTIO_BLK_T_IN as u32 {
                    "read"
                } else {
                    "write"
                }
            );
            continue;
        }
        match req_node.req_type as usize {
            VIRTIO_BLK_T_IN => {
                if req.mediated() {
                    // mediated blk read
                    let task = AsyncTask::new(
                        AsyncTaskData::Io(IoAsyncMsg {
                            src_vmid: vm.id(),
                            vq: vq.clone(),
                            dev: dev.clone(),
                            io_type: VIRTIO_BLK_T_IN,
                            blk_id: vm.med_blk_id(),
                            sector: sector + region_start,
                            count: req_node.iov_sum_up / SECTOR_BSIZE,
                            cache,
                            iov_list: Arc::new(req_node.iov.clone()),
                        }),
                        vm.id(),
                        async_blk_io_req,
                    );
                    add_async_task(task, false);
                } else {
                    todo!();
                }
                for iov in req_node.iov.iter() {
                    let data_bg = iov.data_bg;
                    let len = iov.len as usize;

                    if len < SECTOR_BSIZE {
                        println!("blk_req_handler: read len < SECTOR_BSIZE");
                        continue;
                    }
                    if !req.mediated() {
                        if trace() && (data_bg < 0x1000 || cache_ptr < 0x1000) {
                            panic!("illegal des addr {:x}, src addr {:x}", data_bg, cache_ptr);
                        }
                        memcpy_safe(data_bg as *mut u8, cache_ptr as *mut u8, len);
                    }
                    cache_ptr += len;
                }
            }
            VIRTIO_BLK_T_OUT => {
                for iov in req_node.iov.iter() {
                    let data_bg = iov.data_bg;
                    let len = iov.len as usize;
                    if len < SECTOR_BSIZE {
                        println!("blk_req_handler: read len < SECTOR_BSIZE");
                        continue;
                    }
                    if !req.mediated() {
                        if trace() && (data_bg < 0x1000 || cache_ptr < 0x1000) {
                            panic!("illegal des addr {:x}, src addr {:x}", cache_ptr, data_bg);
                        }
                        memcpy_safe(cache_ptr as *mut u8, data_bg as *mut u8, len);
                    }
                    cache_ptr += len;
                }
                if req.mediated() {
                    // mediated blk write
                    let task = AsyncTask::new(
                        AsyncTaskData::Io(IoAsyncMsg {
                            src_vmid: vm.id(),
                            vq: vq.clone(),
                            dev: dev.clone(),
                            io_type: VIRTIO_BLK_T_OUT,
                            blk_id: vm.med_blk_id(),
                            sector: sector + region_start,
                            count: req_node.iov_sum_up / SECTOR_BSIZE,
                            cache,
                            iov_list: Arc::new(req_node.iov.clone()),
                        }),
                        vm.id(),
                        async_blk_io_req,
                    );
                    add_async_task(task, false);
                } else {
                    todo!();
                }
            }
            VIRTIO_BLK_T_FLUSH => {
                todo!();
            }
            VIRTIO_BLK_T_GET_ID => {
                let data_bg = req_node.iov[0].data_bg;
                let name = "virtio-blk".as_ptr();
                if trace() && (data_bg < 0x1000) {
                    panic!("illegal des addr {:x}", cache_ptr);
                }
                memcpy_safe(data_bg as *mut u8, name, 20);
                let task = AsyncTask::new(
                    AsyncTaskData::NoneTask(IoIdAsyncMsg {
                        vq: vq.clone(),
                        dev: dev.clone(),
                    }),
                    vm.id(),
                    async_blk_id_req,
                );
                task.set_state(AsyncTaskState::Finish);
                add_async_task(task, false);
            }
            _ => {
                println!("Wrong block request type {} ", req_node.req_type);
                continue;
            }
        }

        // update used ring
        if !req.mediated() {
            todo!("reset num to vq size");
            // if !vq.update_used_ring(req_node.iov_total as u32, req_node.desc_chain_head_idx as u32) {
            //     println!("blk_req_handler: fail to update used ring");
            // }
        } else {
            push_used_info(req_node.desc_chain_head_idx, req_node.iov_total as u32, vm.id());
        }
    }

    req.clear_node();
}

pub fn virtio_mediated_blk_notify_handler(vq: Virtq, blk: VirtioMmio, vm: Vm) -> bool {
    //     add_task_count();
    let task = AsyncTask::new(
        AsyncTaskData::Ipi(IpiMediatedMsg {
            src_id: vm.id(),
            vq,
            blk,
        }),
        vm.id(),
        async_ipi_req,
    );
    add_async_task(task, true);
    true
}

pub fn virtio_blk_notify_handler(vq: Virtq, blk: VirtioMmio, vm: Vm) -> bool {
    if vm.id() == 0 && active_vm_id() == 0 {
        panic!("src vm should not be 0");
    }

    let avail_idx = vq.avail_idx();

    // let begin = time_current_us();
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

    // let vq_size = vq.num();
    let mut next_desc_idx_opt = vq.pop_avail_desc_idx(avail_idx);
    let mut process_count: i32 = 0;
    // let mut desc_chain_head_idx;

    // let time0 = time_current_us();

    while next_desc_idx_opt.is_some() {
        let mut next_desc_idx = next_desc_idx_opt.unwrap() as usize;
        vq.disable_notify();
        if vq.check_avail_idx(avail_idx) {
            vq.enable_notify();
        }

        let mut head = true;
        // desc_chain_head_idx = next_desc_idx;

        // vq.show_desc_info(4, vm.clone());

        let mut req_node = VirtioBlkReqNode::default();
        req_node.desc_chain_head_idx = next_desc_idx as u32;
        // println!(
        //     "avail idx {} desc_chain_head {} avail flag {}",
        //     vq.last_avail_idx() - 1,
        //     req_node.desc_chain_head_idx,
        //     vq.avail_flags()
        // );

        loop {
            if vq.desc_has_next(next_desc_idx) {
                if head {
                    if vq.desc_is_writable(next_desc_idx) {
                        println!(
                            "Failed to get virt blk queue desc header, idx = {}, flag = {:x}",
                            next_desc_idx,
                            vq.desc_flags(next_desc_idx)
                        );
                        blk.notify(vm);
                        // vq.notify(dev.int_id(), vm.clone());
                        return false;
                    }
                    head = false;
                    let vreq_addr = vm_ipa2hva(&vm, vq.desc_addr(next_desc_idx));
                    if vreq_addr == 0 {
                        println!("virtio_blk_notify_handler: failed to get vreq");
                        return false;
                    }
                    let vreq = unsafe { &mut *(vreq_addr as *mut VirtioBlkReqNode) };
                    req_node.req_type = vreq.req_type;
                    req_node.sector = vreq.sector;
                } else {
                    /*data handler*/
                    if (vq.desc_flags(next_desc_idx) & 0x2) as u32 >> 1 == req_node.req_type {
                        println!(
                            "Failed to get virt blk queue desc data, idx = {}, req.type = {}, desc.flags = {}",
                            next_desc_idx,
                            req_node.req_type,
                            vq.desc_flags(next_desc_idx)
                        );
                        blk.notify(vm);
                        // vq.notify(dev.int_id(), vm.clone());
                        return false;
                    }
                    let data_bg = vm_ipa2hva(&vm, vq.desc_addr(next_desc_idx));
                    if data_bg == 0 {
                        println!("virtio_blk_notify_handler: failed to get iov data begin");
                        return false;
                    }

                    let iov = BlkIov {
                        data_bg,
                        len: vq.desc_len(next_desc_idx),
                    };
                    req_node.iov_sum_up += iov.len as usize;
                    req_node.iov.push(iov);
                }
            } else {
                /*state handler*/
                if !vq.desc_is_writable(next_desc_idx) {
                    println!("Failed to get virt blk queue desc status, idx = {}", next_desc_idx);
                    blk.notify(vm);
                    // vq.notify(dev.int_id(), vm.clone());
                    return false;
                }
                let vstatus_addr = vm_ipa2hva(&vm, vq.desc_addr(next_desc_idx));
                if vstatus_addr == 0 {
                    println!("virtio_blk_notify_handler: vm[{}] failed to vstatus", vm.id());
                    return false;
                }
                let vstatus = unsafe { &mut *(vstatus_addr as *mut u8) };
                if req_node.req_type > 1 && req_node.req_type != VIRTIO_BLK_T_GET_ID as u32 {
                    *vstatus = VIRTIO_BLK_S_UNSUPP as u8;
                } else {
                    *vstatus = VIRTIO_BLK_S_OK as u8;
                }
                break;
            }
            next_desc_idx = vq.desc_next(next_desc_idx) as usize;
        }
        req_node.iov_total = req_node.iov_sum_up;
        req.add_req_node(req_node, &vm);

        process_count += 1;
        next_desc_idx_opt = vq.pop_avail_desc_idx(avail_idx);
    }

    if !req.mediated() {
        generate_blk_req(&req, &vq, &blk, dev.cache(), &vm);
    } else {
        let mediated_blk = mediated_blk_list_get(vm.med_blk_id());
        let cache = mediated_blk.cache_pa();
        generate_blk_req(&req, &vq, &blk, cache, &vm);
    };

    // let time1 = time_current_us();

    if vq.avail_flags() == 0 && process_count > 0 && !req.mediated() {
        println!("virtio blk notify");
        blk.notify(vm);
        // vq.notify(dev.int_id(), vm.clone());
    }

    // if req.mediated() {
    // finish_task(true);
    //     finish_async_task(true);
    //     async_task_exe();
    // }

    // let end = time_current_us();
    // println!("init time {}us, while handle desc ring time {}us, finish task {}us", time0 - begin, time1 - time0, end - time1);
    true
}
