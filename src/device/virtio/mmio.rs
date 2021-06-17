// pub const VIRTIO_MMIO_MAGIC_VALUE: usize = 0x000;
// pub const VIRTIO_MMIO_VERSION: usize = 0x004;
// pub const VIRTIO_MMIO_DEVICE_ID: usize = 0x008;
// pub const VIRTIO_MMIO_VENDOR_ID: usize = 0x00c;
use crate::config::VmEmulatedDeviceConfig;
use crate::device::EmuContext;
use crate::device::VirtDev;
use crate::kernel::Vm;
use spin::Mutex;

pub const VIRTIO_F_VERSION_1: usize = 1 << 32;

#[repr(C)]
struct VirtMmioRegs {
    magic: u32,
    version: u32,
    device_id: u32,
    vendor_id: u32,
    dev_feature: u32,
    dev_feature_sel: u32,
    drv_feature: u32,
    drv_feature_sel: u32,
    q_sel: u32,
    q_num_max: u32,
    irt_stat: u32,
    irt_ack: u32,
    dev_stat: u32,
}

impl VirtMmioRegs {
    pub fn default() -> VirtMmioRegs {
        VirtMmioRegs {
            magic: 0,
            version: 0,
            device_id: 0,
            vendor_id: 0,
            dev_feature: 0,
            dev_feature_sel: 0,
            drv_feature: 0,
            drv_feature_sel: 0,
            q_sel: 0,
            q_num_max: 0,
            irt_stat: 0,
            irt_ack: 0,
            dev_stat: 0,
        }
    }

    pub fn init(&mut self, id: VirtioDeviceType) {
        self.magic = 0x74726976;
        self.version = 0x2;
        self.vendor_id = 0x8888;
        self.device_id = id as u32;
        self.dev_feature = 0;
        self.drv_feature = 0;
        self.q_sel = 0;
    }
}

pub struct VirtioMmio {
    inner: Mutex<VirtioMmioInner>,
}

impl VirtioMmio {
    pub fn new(id: usize) -> VirtioMmio {
        VirtioMmio {
            inner: Mutex::new(VirtioMmioInner::new(id)),
        }
    }

    pub fn mmio_reg_init(&self, dev_type: VirtioDeviceType) {
        let mut inner = self.inner.lock();
        inner.reg_init(dev_type);
    }

    pub fn dev_init(&self, dev_type: VirtioDeviceType, config: &VmEmulatedDeviceConfig) {
        let inner = self.inner.lock();
        inner.dev.init(dev_type, config);
    }
}

struct VirtioMmioInner {
    id: usize,
    driver_features: usize,
    driver_status: usize,
    regs: VirtMmioRegs,
    vq_num: usize,
    dev: VirtDev,
}

impl VirtioMmioInner {
    fn new(id: usize) -> VirtioMmioInner {
        VirtioMmioInner {
            id,
            driver_features: 0,
            driver_status: 0,
            regs: VirtMmioRegs::default(),
            vq_num: 0,
            dev: VirtDev::default(),
        }
    }

    pub fn reg_init(&mut self, dev_type: VirtioDeviceType) {
        self.regs.init(dev_type);
    }
}

use crate::device::VirtioDeviceType;
pub fn emu_virtio_mmio_init(vm: Vm, emu_dev_id: usize) -> bool {
    // unimplemented!();
    let mut virt_dev_type: VirtioDeviceType = VirtioDeviceType::None;
    let vm_cfg = vm.config();
    match vm_cfg.vm_emu_dev_confg.as_ref().unwrap()[emu_dev_id].emu_type {
        crate::device::EmuDeviceType::EmuDeviceTVirtioBlk => {
            virt_dev_type = VirtioDeviceType::Block;
        }
        crate::device::EmuDeviceType::EmuDeviceTVirtioNet => {
            virt_dev_type = VirtioDeviceType::Net;
        }
        _ => {
            println!("emu_virtio_mmio_init: unknown emulated device type");
            return false;
        }
    }
    let mmio = VirtioMmio::new(emu_dev_id);

    mmio.mmio_reg_init(virt_dev_type);
    mmio.dev_init(
        virt_dev_type,
        &vm_cfg.vm_emu_dev_confg.as_ref().unwrap()[emu_dev_id],
    );
    true
}

pub fn emu_virtio_mmio_handler(emu_dev_id: usize, emu_ctx: &EmuContext) -> bool {
    unimplemented!();
}
