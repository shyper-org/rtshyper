use crate::arch::PAGE_SIZE;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

use crate::kernel::vm_ipa2pa;

pub const VIRTQUEUE_BLK_MAX_SIZE: usize = 256;
pub const VIRTQUEUE_NET_MAX_SIZE: usize = 256;

/* VIRTIO_BLK_FEATURES*/
pub const VIRTIO_BLK_F_SIZE_MAX: usize = 1 << 1;
pub const VIRTIO_BLK_F_SEG_MAX: usize = 1 << 2;
pub const BLOCKIF_IOV_MAX: usize = 64;

/* BLOCK REQUEST TYPE*/
pub const VIRTIO_BLK_T_GET_ID: usize = 8;

/* BLOCK REQUEST STATUS*/
pub const VIRTIO_BLK_S_OK: usize = 0;
pub const VIRTIO_BLK_S_IOERR: usize = 1;
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
struct BlkIov {
    data_bg: usize,
    len: u32,
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

    pub fn reset_blk_iov(&self) {
        let mut inner = self.inner.lock();
        inner.iov_total = 0;
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
}

// TODO: maybe should not use Vec, but [BlkIov; 64]
#[repr(C)]
struct VirtioBlkReqInner {
    req_type: u32,
    reserved: u32,
    sector: usize,
    iov: Vec<BlkIov>,
    iov_total: usize,
    region: BlkReqRegion,
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
        }
    }

    pub fn set_start(&mut self, start: usize) {
        self.region.start = start;
    }

    pub fn set_size(&mut self, size: usize) {
        self.region.size = size;
    }
}

use crate::device::{VirtioMmio, Virtq};
pub fn virtio_blk_notify_handler(vq: Virtq, blk: VirtioMmio) -> bool {
    println!("in virtio_blk_notify_handler");
    if vq.ready() == 0 {
        println!("Virt_queue is not ready!");
        return false;
    }

    let dev = blk.dev();
    let req = match dev.req() {
        super::DevReq::BlkReq(blk_req) => blk_req,
        _ => {
            panic!("virtio_blk_notify_handler: illegal req");
        }
    };

    let process_count: i32 = 0;
    let desc_chain_head_idx;
    let vq_size = vq.num();
    let next_desc_idx_opt = vq.pop_avail_desc_idx();

    while next_desc_idx_opt.is_some() {
        let mut next_desc_idx = next_desc_idx_opt.unwrap() as usize;
        vq.disable_notify();
        if vq.check_avail_idx() {
            vq.enable_notify();
        }

        let mut head = true;
        desc_chain_head_idx = next_desc_idx;
        req.reset_blk_iov();

        loop {
            if vq.desc_has_next(next_desc_idx) {
                if head {
                    if vq.desc_is_writable(next_desc_idx) {
                        println!(
                            "Failed to get virt blk queue desc header, idx = {}",
                            next_desc_idx
                        );
                        vq.notify(dev.int_id());
                        return false;
                    }
                    head = false;
                    let vreq_addr = vm_ipa2pa(vq.desc_addr(next_desc_idx));
                    if vreq_addr == 0 {
                        println!("virtio_blk_notify_handler: failed to get vreq");
                        return false;
                    }
                    let vreq = unsafe { &mut *(vreq_addr as *mut VirtioBlkReqInner) };
                    req.set_type(vreq.req_type);
                    req.set_sector(vreq.sector);
                } else {
                    /*data handler*/
                    if (vq.desc_flags(next_desc_idx) & 0x2) as u32 >> 1 == req.req_type() {
                        println!(
                            "Failed to get virt blk queue desc data, idx = {}, req.type = {}, desc.flags = {}", 
                            next_desc_idx, req.req_type(), vq.desc_flags(next_desc_idx)
                        );
                        vq.notify(dev.int_id());
                        return false;
                    }
                    let data_bg = vm_ipa2pa(vq.desc_addr(next_desc_idx));
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
                if vq.desc_is_writable(next_desc_idx) {
                    println!(
                        "Failed to get virt blk queue desc status, idx = {}",
                        next_desc_idx,
                    );
                    vq.notify(dev.int_id());
                    return false;
                }
                let vstatus_addr = vm_ipa2pa(vq.desc_addr(next_desc_idx));
                if vstatus_addr == 0 {
                    println!("virtio_blk_notify_handler: failed to vstatus");
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
        unimplemented!();
        // TODO: blk_req_handler and update_used_ring

        process_count += 1;
        next_desc_idx_opt = vq.pop_avail_desc_idx();
    }

    if vq.avail_flags() == 0 && process_count > 0 {
        vq.notify(dev.int_id());
    }

    return true;
}
