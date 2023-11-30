use spin::Mutex;

use crate::config::VmEmulatedDeviceConfig;

#[cfg(feature = "balloon")]
use super::balloon::{balloon_features, VirtioBallonConfig};
use super::blk::{blk_features, BlkDesc, VirtioBlkReq};
use super::console::{console_features, ConsoleDesc};
use super::net::{net_features, NetDesc};

#[derive(Copy, Clone, Debug)]
#[allow(dead_code)]
pub enum VirtioDeviceType {
    None = 0,
    Net = 1,
    Block = 2,
    Console = 3,
    #[cfg(feature = "balloon")]
    Balloon = 5,
}

pub enum DevDesc {
    Blk(BlkDesc),
    Net(NetDesc),
    Console(ConsoleDesc),
    #[cfg(feature = "balloon")]
    Balloon(VirtioBallonConfig),
}

#[allow(dead_code)]
pub(super) struct VirtDev {
    dev_type: VirtioDeviceType,
    int_id: usize,
    desc: DevDesc,
    features: usize,
    req: Option<VirtioBlkReq>,
    inner: Mutex<VirtDevInner>,
}

impl VirtDev {
    pub fn new(dev_type: VirtioDeviceType, config: &VmEmulatedDeviceConfig) -> Self {
        let (desc, features, req) = match dev_type {
            VirtioDeviceType::Block => {
                let desc = DevDesc::Blk(BlkDesc::new(config.cfg_list[1]));

                // TODO: blk_features_init & cache init
                let features = blk_features();

                let mut blk_req = VirtioBlkReq::default();
                blk_req.set_start(config.cfg_list[0]);
                blk_req.set_mediated(config.mediated);
                blk_req.set_size(config.cfg_list[1]);
                (desc, features, Some(blk_req))
            }
            VirtioDeviceType::Net => {
                let desc = DevDesc::Net(NetDesc::new(&config.cfg_list));

                let features = net_features();

                (desc, features, None)
            }
            VirtioDeviceType::Console => {
                let desc = DevDesc::Console(ConsoleDesc::new(config.cfg_list[0] as u16, config.cfg_list[1] as u64));
                let features = console_features();

                (desc, features, None)
            }
            #[cfg(feature = "balloon")]
            VirtioDeviceType::Balloon => {
                let config = DevDesc::Balloon(VirtioBallonConfig::new(config.cfg_list[0]));
                let features = balloon_features();
                (config, features, None)
            }
            _ => {
                panic!("ERROR: Wrong virtio device type");
            }
        };
        Self {
            dev_type,
            int_id: config.irq_id,
            desc,
            features,
            req,
            inner: Mutex::new(VirtDevInner::default()),
        }
    }

    pub fn features(&self) -> usize {
        self.features
    }

    pub fn generation(&self) -> usize {
        let inner = self.inner.lock();
        inner.generation
    }

    pub fn desc(&self) -> &DevDesc {
        &self.desc
    }

    pub fn req(&self) -> &Option<VirtioBlkReq> {
        &self.req
    }

    pub fn int_id(&self) -> usize {
        self.int_id
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
        match self.req() {
            Some(req) => req.mediated(),
            None => false,
        }
    }
}

struct VirtDevInner {
    activated: bool,
    generation: usize,
}

impl VirtDevInner {
    pub fn default() -> VirtDevInner {
        VirtDevInner {
            activated: false,
            generation: 0,
        }
    }
}
