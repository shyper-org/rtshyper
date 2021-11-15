use alloc::sync::Arc;

use spin::Mutex;

// use crate::arch::PAGE_SIZE;
use crate::config::VmEmulatedDeviceConfig;
// use crate::device::add_mediated_dev;
use crate::device::{net_features, NetDesc};
use crate::device::{BlkDesc, BLOCKIF_IOV_MAX, VirtioBlkReq};
use crate::device::{VIRTIO_BLK_F_SEG_MAX, VIRTIO_BLK_F_SIZE_MAX, VIRTIO_F_VERSION_1};
use crate::device::{BlkStat, NicStat};
use crate::kernel::mem_pages_alloc;
use crate::mm::PageFrame;

#[derive(Copy, Clone)]
pub enum VirtioDeviceType {
    None = 0,
    Net = 1,
    Block = 2,
}

#[derive(Clone)]
pub enum DevStat {
    BlkStat(BlkStat),
    NicStat(NicStat),
    None,
}

#[derive(Clone)]
pub enum DevDesc {
    BlkDesc(BlkDesc),
    NetDesc(NetDesc),
    None,
}

#[derive(Clone)]
pub enum DevReq {
    BlkReq(VirtioBlkReq),
    None,
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

    pub fn init(
        &self,
        dev_type: VirtioDeviceType,
        config: &VmEmulatedDeviceConfig,
        mediated: bool,
    ) {
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
            DevReq::BlkReq(req) => {
                req.mediated()
            }
            DevReq::None => {
                false
            }
        }
    }

    // virtio_dev_init
    pub fn init(
        &mut self,
        dev_type: VirtioDeviceType,
        config: &VmEmulatedDeviceConfig,
        mediated: bool,
    ) {
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
                        self.cache = Some(page_frame.clone());
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
                // if !ipi_register(IpiType::IpiTEthernetMsg, ethernet_ipi_rev_handler) {
                //     panic!(
                //         "virtio_dev_init: failed to register ipi {:?}",
                //         IpiType::IpiTEthernetMsg,
                //     );
                // }
                // if !ipi_register(IpiType::IpiTEthernetAck, ethernet_ipi_ack_handler) {
                //     panic!(
                //         "virtio_dev_init: failed to register ipi {:?}",
                //         IpiType::IpiTEthernetAck,
                //     );
                // }

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
            _ => {
                panic!("ERROR: Wrong virtio device type");
            }
        }
    }
}
