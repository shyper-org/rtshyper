use alloc::sync::Arc;

use spin::Mutex;

use crate::config::VmEmulatedDeviceConfig;
// use crate::device::add_mediated_dev;
use crate::device::{net_features, NetDesc};
use crate::device::{console_features, ConsoleDesc};
use crate::device::{BlkDesc, BLOCKIF_IOV_MAX, VirtioBlkReq};
use crate::device::{VIRTIO_BLK_F_SEG_MAX, VIRTIO_BLK_F_SIZE_MAX, VIRTIO_F_VERSION_1};
use crate::device::{BlkStat, NicStat};
use crate::device::DevReq::BlkReq;
use crate::kernel::{ConsoleDescData, DevDescData, mem_pages_alloc, NetDescData, VirtDevData};
use crate::mm::PageFrame;

#[derive(Copy, Clone, Debug)]
pub enum VirtioDeviceType {
    None = 0,
    Net = 1,
    Block = 2,
    Console = 3,
}

#[derive(Clone)]
pub enum DevStat {
    BlkStat(BlkStat),
    NicStat(NicStat),
    None,
}

impl DevStat {
    pub fn copy_from(&mut self, stat: DevStat) {
        match stat {
            DevStat::BlkStat(src_stat) => {
                *self = DevStat::BlkStat(src_stat.back_up());
            }
            DevStat::NicStat(src_stat) => {
                *self = DevStat::NicStat(src_stat.back_up());
            }
            DevStat::None => {
                *self = DevStat::None;
            }
        }
    }
}

#[derive(Clone)]
pub enum DevDesc {
    BlkDesc(BlkDesc),
    NetDesc(NetDesc),
    ConsoleDesc(ConsoleDesc),
    None,
}

impl DevDesc {
    pub fn copy_from(&mut self, desc: DevDesc) {
        match desc {
            DevDesc::BlkDesc(src_desc) => {
                *self = DevDesc::BlkDesc(src_desc);
            }
            DevDesc::NetDesc(src_desc) => {
                *self = DevDesc::NetDesc(src_desc.back_up());
            }
            DevDesc::ConsoleDesc(src_desc) => {
                *self = DevDesc::ConsoleDesc(src_desc.back_up());
            }
            DevDesc::None => *self = DevDesc::None,
        }
    }
}

#[derive(Clone)]
pub enum DevReq {
    BlkReq(VirtioBlkReq),
    None,
}

impl DevReq {
    pub fn copy_from(&mut self, src_req: DevReq) {
        match src_req {
            DevReq::BlkReq(req) => {
                *self = BlkReq(req.back_up());
            }
            DevReq::None => {
                *self = DevReq::None;
            }
        }
    }
}

#[derive(Clone)]
pub struct VirtDev {
    inner: Arc<Mutex<VirtDevInner>>,
}

impl VirtDev {
    pub fn default() -> VirtDev {
        VirtDev {
            inner: Arc::new(Mutex::new(VirtDevInner::default())),
        }
    }

    pub fn init(&self, dev_type: VirtioDeviceType, config: &VmEmulatedDeviceConfig, mediated: bool) {
        let mut inner = self.inner.lock();
        inner.init(dev_type, config, mediated);
    }

    pub fn features(&self) -> usize {
        let inner = self.inner.lock();
        inner.features
    }

    pub fn generation(&self) -> usize {
        let inner = self.inner.lock();
        inner.generation
    }

    pub fn desc(&self) -> DevDesc {
        let inner = self.inner.lock();
        inner.desc.clone()
    }

    pub fn req(&self) -> DevReq {
        let inner = self.inner.lock();
        inner.req.clone()
    }

    pub fn int_id(&self) -> usize {
        let inner = self.inner.lock();
        inner.int_id
    }

    pub fn cache(&self) -> usize {
        let inner = self.inner.lock();
        return inner.cache.as_ref().unwrap().pa();
    }

    pub fn stat(&self) -> DevStat {
        let inner = self.inner.lock();
        inner.stat.clone()
    }

    pub fn activated(&self) -> bool {
        let inner = self.inner.lock();
        inner.activated
    }

    pub fn set_activated(&self, activated: bool) {
        let mut inner = self.inner.lock();
        inner.activated = activated;
    }

    pub fn mediated(&self) -> bool {
        let inner = self.inner.lock();
        inner.mediated()
    }

    pub fn is_net(&self) -> bool {
        let inner = self.inner.lock();
        match &inner.desc {
            DevDesc::NetDesc(_) => { true }
            _ => { false }
        }
    }

    // use for migration save
    pub fn restore_virt_dev_data(&self, dev_data: &VirtDevData) {
        let mut inner = self.inner.lock();
        // println!(
        //     "activated {}, type {:#?}, features 0x{:x}, generation {}, int id {}",
        //     dev_data.activated, dev_data.dev_type, dev_data.features, dev_data.generation, dev_data.int_id
        // );
        inner.activated = dev_data.activated;
        inner.dev_type = dev_data.dev_type;
        inner.features = dev_data.features;
        inner.generation = dev_data.generation;
        inner.int_id = dev_data.int_id;
        match &inner.desc {
            DevDesc::BlkDesc(_) => {
                todo!("restore_virt_dev_data: Migrate vm use nfs");
            }
            DevDesc::NetDesc(net_desc) => {
                if let DevDescData::NetDesc(desc_data) = &dev_data.desc {
                    net_desc.restore_net_data(desc_data);
                }
            }
            DevDesc::ConsoleDesc(console_desvc) => {
                if let DevDescData::ConsoleDesc(desc_data) = &dev_data.desc {
                    console_desvc.restore_console_data(desc_data);
                }
            }
            DevDesc::None => {}
        }
    }

    // use for migration save
    pub fn save_virt_dev_data(&self, dev_data: &mut VirtDevData) {
        let mut inner = self.inner.lock();
        dev_data.activated = inner.activated;
        dev_data.dev_type = inner.dev_type;
        dev_data.features = inner.features;
        dev_data.generation = inner.generation;
        dev_data.int_id = inner.int_id;
        match &inner.desc {
            DevDesc::BlkDesc(_) => {
                todo!("save_virt_dev_data: Migrate vm use nfs");
            }
            DevDesc::NetDesc(net_desc) => {
                dev_data.desc = DevDescData::NetDesc(NetDescData { mac: [0; 6], status: 0 });
                if let DevDescData::NetDesc(desc_data) = &mut dev_data.desc {
                    net_desc.save_net_data(desc_data);
                }
            }
            DevDesc::ConsoleDesc(console_desvc) => {
                dev_data.desc = DevDescData::ConsoleDesc(ConsoleDescData {
                    oppo_end_vmid: 0,
                    oppo_end_ipa: 0,
                    cols: 0,
                    rows: 0,
                    max_nr_ports: 0,
                    emerg_wr: 0,
                });
                if let DevDescData::ConsoleDesc(desc_data) = &mut dev_data.desc {
                    console_desvc.save_console_data(desc_data);
                }
            }
            DevDesc::None => {}
        }
        // set activated to false
        inner.activated = false;
    }

    // use for live update
    pub fn save_virt_dev(&self, src_dev: VirtDev) {
        let mut inner = self.inner.lock();
        let src_dev_inner = src_dev.inner.lock();
        inner.activated = src_dev_inner.activated;
        inner.dev_type = src_dev_inner.dev_type;
        inner.features = src_dev_inner.features;
        inner.generation = src_dev_inner.generation;
        inner.int_id = src_dev_inner.int_id;
        inner.desc.copy_from(src_dev_inner.desc.clone());
        inner.req.copy_from(src_dev_inner.req.clone());
        // inner.cache is set by fn dev_init, no need to copy here
        inner.cache = match &src_dev_inner.cache {
            None => None,
            Some(page) => Some(PageFrame::new(page.pa, page.page_num)),
        };
        inner.stat.copy_from(src_dev_inner.stat.clone());
    }
}

pub struct VirtDevInner {
    activated: bool,
    dev_type: VirtioDeviceType,
    features: usize,
    generation: usize,
    int_id: usize,
    desc: DevDesc,
    req: DevReq,
    cache: Option<PageFrame>,
    stat: DevStat,
}

impl VirtDevInner {
    pub fn default() -> VirtDevInner {
        VirtDevInner {
            activated: false,
            dev_type: VirtioDeviceType::None,
            features: 0,
            generation: 0,
            int_id: 0,
            desc: DevDesc::None,
            req: DevReq::None,
            cache: None,
            stat: DevStat::None,
        }
    }

    pub fn mediated(&self) -> bool {
        match &self.req {
            DevReq::BlkReq(req) => req.mediated(),
            DevReq::None => false,
        }
    }

    // virtio_dev_init
    pub fn init(&mut self, dev_type: VirtioDeviceType, config: &VmEmulatedDeviceConfig, mediated: bool) {
        self.dev_type = dev_type;
        self.int_id = config.irq_id;

        match self.dev_type {
            VirtioDeviceType::Block => {
                let blk_desc = BlkDesc::default();
                blk_desc.cfg_init(config.cfg_list[1]);
                self.desc = DevDesc::BlkDesc(blk_desc);

                // TODO: blk_features_init & cache init
                self.features |= VIRTIO_BLK_F_SIZE_MAX | VIRTIO_BLK_F_SEG_MAX | VIRTIO_F_VERSION_1;

                let blk_req = VirtioBlkReq::default();
                blk_req.set_start(config.cfg_list[0]);
                blk_req.set_mediated(mediated);
                blk_req.set_size(config.cfg_list[1]);
                self.req = DevReq::BlkReq(blk_req);

                match mem_pages_alloc(BLOCKIF_IOV_MAX) {
                    Ok(page_frame) => {
                        // println!("PageFrame pa {:x}", page_frame.pa());
                        self.cache = Some(page_frame);
                        // if mediated {
                        //     // todo: change to iov ring
                        //     let cache_size = BLOCKIF_IOV_MAX * PAGE_SIZE;
                        //     add_mediated_dev(0, page_frame.pa(), cache_size);
                        // }
                    }
                    Err(_) => {
                        println!("VirtDevInner::init(): mem_pages_alloc failed");
                    }
                }

                self.stat = DevStat::BlkStat(BlkStat::default());
            }
            VirtioDeviceType::Net => {
                let net_desc = NetDesc::default();
                net_desc.cfg_init(&config.cfg_list);
                self.desc = DevDesc::NetDesc(net_desc);

                self.features |= net_features();

                match mem_pages_alloc(1) {
                    Ok(page_frame) => {
                        // println!("PageFrame pa {:x}", page_frame.pa());
                        self.cache = Some(page_frame);
                    }
                    Err(_) => {
                        println!("VirtDevInner::init(): mem_pages_alloc failed");
                    }
                }

                self.stat = DevStat::NicStat(NicStat::default());
            }
            VirtioDeviceType::Console => {
                let console_desc = ConsoleDesc::default();
                console_desc.cfg_init(config.cfg_list[0] as u16, config.cfg_list[1] as u64);
                self.desc = DevDesc::ConsoleDesc(console_desc);
                self.features |= console_features();

                match mem_pages_alloc(1) {
                    Ok(page_frame) => {
                        // println!("PageFrame pa {:x}", page_frame.pa());
                        self.cache = Some(page_frame);
                    }
                    Err(_) => {
                        println!("VirtDevInner::init(): mem_pages_alloc failed");
                    }
                }
            }
            _ => {
                panic!("ERROR: Wrong virtio device type");
            }
        }
    }
}
