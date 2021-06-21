// pub const VIRTIO_MMIO_MAGIC_VALUE: usize = 0x000;
// pub const VIRTIO_MMIO_VERSION: usize = 0x004;
// pub const VIRTIO_MMIO_DEVICE_ID: usize = 0x008;
// pub const VIRTIO_MMIO_VENDOR_ID: usize = 0x00c;
use crate::config::VmEmulatedDeviceConfig;
use crate::device::EmuContext;
use crate::device::VirtDev;
use crate::device::{VirtioQueue, Virtq};
use crate::device::{VIRTQUEUE_BLK_MAX_SIZE, VIRTQUEUE_NET_MAX_SIZE};
use crate::driver::VIRTIO_MMIO_MAGIC_VALUE;
use crate::kernel::Vm;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

pub const VIRTIO_F_VERSION_1: usize = 1 << 32;

pub const VIRTIO_MMIO_GUEST_FEATURES_SEL: usize = 0x024;
pub const VIRTIO_MMIO_QUEUE_NOTIFY: usize = 0x050;
pub const VIRTIO_MMIO_INTERRUPT_STATUS: usize = 0x060;
pub const VIRTIO_MMIO_INTERRUPT_ACK: usize = 0x064;
pub const VIRTIO_MMIO_STATUS: usize = 0x070;

pub const VIRTIO_MMIO_INT_VRING: usize = 1 << 0;

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

#[derive(Clone)]
pub struct VirtioMmio {
    inner: Arc<Mutex<VirtioMmioInner>>,
}

impl VirtioQueue for VirtioMmio {
    fn virtio_queue_init(&self, dev_type: VirtioDeviceType) {
        let mut inner = self.inner.lock();
        match dev_type {
            VirtioDeviceType::Block => {
                self.set_q_num_max(VIRTQUEUE_BLK_MAX_SIZE as u32);
                inner.vq.push(Virtq::default());
                inner.vq[0].reset(0);
                use crate::device::virtio_blk_notify_handler;
                inner.vq[0].set_notify_handler(virtio_blk_notify_handler);
            }
            VirtioDeviceType::Net => {
                self.set_q_num_max(VIRTQUEUE_NET_MAX_SIZE as u32);
                for i in 0..2 {
                    inner.vq.push(Virtq::default());
                    inner.vq[i].reset(i);
                    // TODO: queue_notify_handler;
                }
                unimplemented!();
            }
            VirtioDeviceType::None => {
                panic!("virtio_queue_init: unknown emulated device type");
            }
        }
    }

    fn virtio_queue_reset(&self, index: usize) {
        let mut inner = self.inner.lock();
        inner.vq[index].reset(index);
    }
}

impl VirtioMmio {
    pub fn new(id: usize) -> VirtioMmio {
        VirtioMmio {
            inner: Arc::new(Mutex::new(VirtioMmioInner::new(id))),
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

    pub fn set_irt_stat(&self, irt_stat: u32) {
        let mut inner = self.inner.lock();
        inner.regs.irt_stat = irt_stat;
    }

    pub fn set_irt_ack(&self, irt_ack: u32) {
        let mut inner = self.inner.lock();
        inner.regs.irt_ack = irt_ack;
    }

    pub fn set_q_num_max(&self, q_num_max: u32) {
        let mut inner = self.inner.lock();
        inner.regs.q_num_max = q_num_max;
    }

    pub fn q_sel(&self) -> u32 {
        let inner = self.inner.lock();
        inner.regs.q_sel
    }

    pub fn irt_stat(&self) -> u32 {
        let inner = self.inner.lock();
        inner.regs.irt_stat
    }

    pub fn notify_handler(&self, idx: usize) -> bool {
        let inner = self.inner.lock();
        if idx >= inner.vq.len() {
            return false;
        }
        return inner.vq[idx].call_notify_handler(self.clone());
    }
}

struct VirtioMmioInner {
    id: usize,
    driver_features: usize,
    driver_status: usize,
    regs: VirtMmioRegs,
    dev: VirtDev,

    vq: Vec<Virtq>,
}

impl VirtioMmioInner {
    fn new(id: usize) -> VirtioMmioInner {
        VirtioMmioInner {
            id,
            driver_features: 0,
            driver_status: 0,
            regs: VirtMmioRegs::default(),
            dev: VirtDev::default(),
            vq: Vec::new(),
        }
    }

    fn reg_init(&mut self, dev_type: VirtioDeviceType) {
        self.regs.init(dev_type);
    }
}

use crate::device::{EmuDevs, VirtioDeviceType};
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
    vm.set_emu_devs(emu_dev_id, EmuDevs::VirtioBlk(mmio.clone()));

    mmio.mmio_reg_init(virt_dev_type);
    mmio.dev_init(
        virt_dev_type,
        &vm_cfg.vm_emu_dev_confg.as_ref().unwrap()[emu_dev_id],
    );
    // no need to set vm_if_list
    // TODO: virtio_queue_init()
    mmio.virtio_queue_init(virt_dev_type);

    true
}

use crate::kernel::active_vm;
pub fn emu_virtio_mmio_handler(emu_dev_id: usize, emu_ctx: &EmuContext) -> bool {
    use crate::kernel::{context_get_gpr, context_set_gpr};
    let vm = match active_vm() {
        Some(vm) => vm,
        None => {
            panic!("emu_virtio_mmio_handler: current vcpu.vm is none");
        }
    };

    let mmio = match vm.emu_dev(emu_dev_id) {
        EmuDevs::VirtioBlk(blk) => blk,
        _ => {
            panic!("emu_virtio_mmio_handler: illegal mmio dev type")
        }
    };

    let addr = emu_ctx.address;
    let offset = addr - vm.config().vm_emu_dev_confg.as_ref().unwrap()[emu_dev_id].base_ipa;
    let write = emu_ctx.write;

    if offset == VIRTIO_MMIO_QUEUE_NOTIFY && write {
        mmio.set_irt_stat(VIRTIO_MMIO_INT_VRING as u32);
        let q_sel = mmio.q_sel();
        if !mmio.notify_handler(q_sel as usize) {
            println!("Failed to handle virtio mmio request!");
        }
    } else if offset == VIRTIO_MMIO_INTERRUPT_STATUS && !write {
        context_set_gpr(emu_ctx.reg, mmio.irt_stat() as usize);
    } else if offset == VIRTIO_MMIO_INTERRUPT_ACK && write {
        mmio.set_irt_ack(context_get_gpr(emu_ctx.reg));
    } else if (VIRTIO_MMIO_MAGIC_VALUE <= offset && offset <= VIRTIO_MMIO_GUEST_FEATURES_SEL)
        || offset == VIRTIO_MMIO_STATUS
    {
        // TODO: virtio_mmio_prologue_access(mmio, emu_ctx, offset, write);
    }
    unimplemented!();
}
