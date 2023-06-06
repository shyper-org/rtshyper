use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

use crate::config::VmEmulatedDeviceConfig;
use crate::device::{EmuContext};
use crate::device::{EmuDev, EmuDeviceType};
use crate::device::Virtq;
use crate::kernel::{current_cpu, ipi_send_msg, IpiInnerMsg, IpiIntInjectMsg, IpiType};
use crate::kernel::{active_vm, active_vm_id};
use crate::kernel::Vm;

use super::balloon::virtio_balloon_notify_handler;
use super::queue::VIRTQ_READY;
use super::net::{virtio_net_handle_ctrl, virtio_net_notify_handler, VIRTQUEUE_NET_MAX_SIZE};
use super::dev::{VirtDev, VirtioDeviceType};
use super::console::{VIRTQUEUE_CONSOLE_MAX_SIZE, virtio_console_notify_handler};
use super::blk::{VIRTQUEUE_BLK_MAX_SIZE, virtio_blk_notify_handler, virtio_mediated_blk_notify_handler};

pub const VIRTIO_F_VERSION_1: usize = 1 << 32;
pub const VIRTIO_MMIO_MAGIC_VALUE: usize = 0x000;
pub const VIRTIO_MMIO_VERSION: usize = 0x004;
pub const VIRTIO_MMIO_DEVICE_ID: usize = 0x008;
pub const VIRTIO_MMIO_VENDOR_ID: usize = 0x00c;
pub const VIRTIO_MMIO_HOST_FEATURES: usize = 0x010;
pub const VIRTIO_MMIO_HOST_FEATURES_SEL: usize = 0x014;
pub const VIRTIO_MMIO_GUEST_FEATURES: usize = 0x020;
pub const VIRTIO_MMIO_GUEST_FEATURES_SEL: usize = 0x024;
pub const VIRTIO_MMIO_QUEUE_SEL: usize = 0x030;
pub const VIRTIO_MMIO_QUEUE_NUM_MAX: usize = 0x034;
pub const VIRTIO_MMIO_QUEUE_NUM: usize = 0x038;
pub const VIRTIO_MMIO_QUEUE_READY: usize = 0x044;
pub const VIRTIO_MMIO_QUEUE_NOTIFY: usize = 0x050;
pub const VIRTIO_MMIO_INTERRUPT_STATUS: usize = 0x060;
pub const VIRTIO_MMIO_INTERRUPT_ACK: usize = 0x064;
pub const VIRTIO_MMIO_STATUS: usize = 0x070;
pub const VIRTIO_MMIO_QUEUE_DESC_LOW: usize = 0x080;
pub const VIRTIO_MMIO_QUEUE_DESC_HIGH: usize = 0x084;
pub const VIRTIO_MMIO_QUEUE_AVAIL_LOW: usize = 0x090;
pub const VIRTIO_MMIO_QUEUE_AVAIL_HIGH: usize = 0x094;
pub const VIRTIO_MMIO_QUEUE_USED_LOW: usize = 0x0a0;
pub const VIRTIO_MMIO_QUEUE_USED_HIGH: usize = 0x0a4;
pub const VIRTIO_MMIO_CONFIG_GENERATION: usize = 0x0fc;
pub const VIRTIO_MMIO_CONFIG: usize = 0x100;
pub const VIRTIO_MMIO_REGS_END: usize = 0x200;

pub const VIRTIO_MMIO_INT_VRING: u32 = 1 << 0;
pub const VIRTIO_MMIO_INT_CONFIG: u32 = 1 << 1;

#[repr(C)]
#[derive(Copy, Clone)]
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

struct VirtioInnerConst {
    base: usize,
    length: usize,
    emu_type: EmuDeviceType,
    vq: Vec<Virtq>,
    dev: VirtDev,
}

struct VirtioInner {
    inner_const: VirtioInnerConst,
    inner: Mutex<VirtioMmioInnerMut>,
}

impl VirtioInner {
    fn new(dev_type: VirtioDeviceType, config: &VmEmulatedDeviceConfig) -> Self {
        Self {
            inner_const: VirtioInnerConst {
                base: config.base_ipa,
                length: config.length,
                emu_type: config.emu_type,
                vq: vec![],
                dev: VirtDev::new(dev_type, config),
            },
            inner: Mutex::new(VirtioMmioInnerMut::new()),
        }
    }

    fn init(&mut self, dev_type: VirtioDeviceType) {
        let mut inner = self.inner.lock();
        inner.regs.init(dev_type);
    }

    pub fn set_q_num_max(&self, q_num_max: u32) {
        let mut inner = self.inner.lock();
        inner.regs.q_num_max = q_num_max;
    }

    fn virtio_queue_init(&mut self, dev_type: VirtioDeviceType) {
        match dev_type {
            VirtioDeviceType::Block => {
                self.set_q_num_max(VIRTQUEUE_BLK_MAX_SIZE as u32);
                let queue = if self.inner_const.dev.mediated() {
                    Virtq::new(0, virtio_mediated_blk_notify_handler)
                } else {
                    Virtq::new(0, virtio_blk_notify_handler)
                };
                self.inner_const.vq.push(queue);
            }
            VirtioDeviceType::Net => {
                self.set_q_num_max(VIRTQUEUE_NET_MAX_SIZE as u32);
                // Not support feature VIRTIO_NET_F_CTRL_VQ (no control queue)
                for i in 0..2 {
                    let queue = Virtq::new(i, virtio_net_notify_handler);
                    self.inner_const.vq.push(queue);
                }
                let queue = Virtq::new(2, virtio_net_handle_ctrl);
                self.inner_const.vq.push(queue);
            }
            VirtioDeviceType::Console => {
                self.set_q_num_max(VIRTQUEUE_CONSOLE_MAX_SIZE as u32);
                for i in 0..4 {
                    let queue = Virtq::new(i, virtio_console_notify_handler);
                    self.inner_const.vq.push(queue);
                }
            }
            VirtioDeviceType::Balloon => {
                self.set_q_num_max(256_u32);
                for i in 0..2 {
                    let queue = Virtq::new(i, virtio_balloon_notify_handler);
                    self.inner_const.vq.push(queue);
                }
            }
            _ => {
                panic!("virtio_queue_init: unknown emulated device type");
            }
        }
    }
}

#[repr(transparent)]
#[derive(Clone)]
pub struct VirtioMmio(Arc<VirtioInner>);

impl VirtioMmio {
    pub fn notify_config(&self, vm: Vm) {
        let mut inner = self.0.inner.lock();
        inner.regs.irt_stat |= VIRTIO_MMIO_INT_CONFIG;
        let int_id = self.dev().int_id();
        let trgt_id = vm.vcpu(0).unwrap().phys_id();
        drop(inner);
        use crate::kernel::interrupt_vm_inject;
        if trgt_id == current_cpu().id {
            interrupt_vm_inject(&vm, vm.vcpu(0).unwrap(), int_id);
        } else {
            let m = IpiIntInjectMsg { vm_id: vm.id(), int_id };
            if !ipi_send_msg(trgt_id, IpiType::IpiTIntInject, IpiInnerMsg::IntInjectMsg(m)) {
                println!("notify_config: failed to send ipi to Core {}", trgt_id);
            }
        }
    }

    pub fn notify(&self, vm: Vm) {
        let mut inner = self.0.inner.lock();
        inner.regs.irt_stat |= VIRTIO_MMIO_INT_VRING;
        let int_id = self.dev().int_id();
        let trgt_id = vm.vcpu(0).unwrap().phys_id();
        drop(inner);
        use crate::kernel::interrupt_vm_inject;
        if trgt_id == current_cpu().id {
            interrupt_vm_inject(&vm, vm.vcpu(0).unwrap(), int_id);
        } else {
            let m = IpiIntInjectMsg { vm_id: vm.id(), int_id };
            if !ipi_send_msg(trgt_id, IpiType::IpiTIntInject, IpiInnerMsg::IntInjectMsg(m)) {
                println!("notify_config: failed to send ipi to Core {}", trgt_id);
            }
        }
    }

    // virtio_dev_reset
    pub fn dev_reset(&self) {
        let mut inner = self.0.inner.lock();
        inner.regs.dev_stat = 0;
        inner.regs.irt_stat = 0;
        let idx = inner.regs.q_sel as usize;
        let vq = &self.0.inner_const.vq;
        vq[idx].set_ready(0);
        for virtq in vq.iter() {
            virtq.reset();
        }
        self.dev().set_activated(false);
    }

    pub fn set_irt_stat(&self, irt_stat: u32) {
        let mut inner = self.0.inner.lock();
        inner.regs.irt_stat = irt_stat;
    }

    pub fn set_irt_ack(&self, irt_ack: u32) {
        let mut inner = self.0.inner.lock();
        inner.regs.irt_ack = irt_ack;
    }

    pub fn set_q_sel(&self, q_sel: u32) {
        let mut inner = self.0.inner.lock();
        inner.regs.q_sel = q_sel;
    }

    pub fn set_dev_stat(&self, dev_stat: u32) {
        let mut inner = self.0.inner.lock();
        inner.regs.dev_stat = dev_stat;
    }

    pub fn set_dev_feature(&self, dev_feature: u32) {
        let mut inner = self.0.inner.lock();
        inner.regs.dev_feature = dev_feature;
    }

    pub fn set_dev_feature_sel(&self, dev_feature_sel: u32) {
        let mut inner = self.0.inner.lock();
        inner.regs.dev_feature_sel = dev_feature_sel;
    }

    pub fn set_drv_feature(&self, drv_feature: u32) {
        let mut inner = self.0.inner.lock();
        inner.regs.drv_feature = drv_feature;
    }

    pub fn set_drv_feature_sel(&self, drv_feature_sel: u32) {
        let mut inner = self.0.inner.lock();
        inner.regs.drv_feature = drv_feature_sel;
    }

    pub fn or_driver_feature(&self, driver_features: usize) {
        let mut inner = self.0.inner.lock();
        inner.driver_features |= driver_features;
    }

    pub(super) fn dev(&self) -> &VirtDev {
        &self.0.inner_const.dev
    }

    pub fn q_sel(&self) -> u32 {
        let inner = self.0.inner.lock();
        inner.regs.q_sel
    }

    pub fn magic(&self) -> u32 {
        let inner = self.0.inner.lock();
        inner.regs.magic
    }

    pub fn version(&self) -> u32 {
        let inner = self.0.inner.lock();
        inner.regs.version
    }

    pub fn device_id(&self) -> u32 {
        let inner = self.0.inner.lock();
        inner.regs.device_id
    }

    pub fn vendor_id(&self) -> u32 {
        let inner = self.0.inner.lock();
        inner.regs.vendor_id
    }

    pub fn dev_stat(&self) -> u32 {
        let inner = self.0.inner.lock();
        inner.regs.dev_stat
    }

    pub fn dev_feature_sel(&self) -> u32 {
        let inner = self.0.inner.lock();
        inner.regs.dev_feature_sel
    }

    pub fn drv_feature_sel(&self) -> u32 {
        let inner = self.0.inner.lock();
        inner.regs.drv_feature_sel
    }

    pub fn q_num_max(&self) -> u32 {
        let inner = self.0.inner.lock();
        inner.regs.q_num_max
    }

    pub fn irt_stat(&self) -> u32 {
        let inner = self.0.inner.lock();
        inner.regs.irt_stat
    }

    pub fn vq(&self, idx: usize) -> Result<&Virtq, ()> {
        match self.0.inner_const.vq.get(idx) {
            Some(vq) => Ok(vq),
            None => Err(()),
        }
    }

    #[inline]
    pub fn base(&self) -> usize {
        self.0.inner_const.base
    }

    pub fn notify_handler(&self, idx: usize) -> bool {
        self.0.inner_const.vq[idx].call_notify_handler(self.clone())
    }

    pub fn vq_num(&self) -> usize {
        self.0.inner_const.vq.len()
    }
}

struct VirtioMmioInnerMut {
    driver_features: usize,
    driver_status: usize,
    regs: VirtMmioRegs,
}

impl VirtioMmioInnerMut {
    fn new() -> VirtioMmioInnerMut {
        VirtioMmioInnerMut {
            driver_features: 0,
            driver_status: 0,
            regs: VirtMmioRegs::default(),
        }
    }
}

fn virtio_mmio_prologue_access(mmio: &VirtioMmio, emu_ctx: &EmuContext, offset: usize, write: bool) {
    if !write {
        let value;
        match offset {
            VIRTIO_MMIO_MAGIC_VALUE => {
                value = mmio.magic();
            }
            VIRTIO_MMIO_VERSION => {
                value = mmio.version();
            }
            VIRTIO_MMIO_DEVICE_ID => {
                value = mmio.device_id();
            }
            VIRTIO_MMIO_VENDOR_ID => {
                value = mmio.vendor_id();
            }
            VIRTIO_MMIO_HOST_FEATURES => {
                if mmio.dev_feature_sel() != 0 {
                    value = (mmio.dev().features() >> 32) as u32;
                } else {
                    value = mmio.dev().features() as u32;
                }
                mmio.set_dev_feature(value);
            }
            VIRTIO_MMIO_STATUS => {
                value = mmio.dev_stat();
            }
            _ => {
                println!("virtio_be_init_handler wrong reg_read, address={:#x}", emu_ctx.address);
                return;
            }
        }
        let idx = emu_ctx.reg;
        let val = value as usize;
        current_cpu().set_gpr(idx, val);
    } else {
        let idx = emu_ctx.reg;
        let value = current_cpu().get_gpr(idx) as u32;
        match offset {
            VIRTIO_MMIO_HOST_FEATURES_SEL => {
                mmio.set_dev_feature_sel(value);
            }
            VIRTIO_MMIO_GUEST_FEATURES => {
                mmio.set_drv_feature(value);
                if mmio.drv_feature_sel() != 0 {
                    mmio.or_driver_feature((value as usize) << 32);
                } else {
                    mmio.or_driver_feature(value as usize);
                }
            }
            VIRTIO_MMIO_GUEST_FEATURES_SEL => {
                mmio.set_drv_feature_sel(value);
            }
            VIRTIO_MMIO_STATUS => {
                mmio.set_dev_stat(value);
                if mmio.dev_stat() == 0 {
                    mmio.dev_reset();
                    info!("VM {} virtio device {:x} is reset", active_vm_id(), mmio.base());
                } else if mmio.dev_stat() == 0xf {
                    mmio.dev().set_activated(true);
                    info!("VM {} virtio device {:x} init ok", active_vm_id(), mmio.base());
                }
            }
            _ => {
                println!("virtio_mmio_prologue_access: wrong reg write {:#x}", emu_ctx.address);
            }
        }
    }
}

fn virtio_mmio_queue_access(mmio: &VirtioMmio, emu_ctx: &EmuContext, offset: usize, write: bool) {
    if !write {
        let value;
        match offset {
            VIRTIO_MMIO_QUEUE_NUM_MAX => value = mmio.q_num_max(),
            VIRTIO_MMIO_QUEUE_READY => {
                let idx = mmio.q_sel() as usize;
                match mmio.vq(idx) {
                    Ok(virtq) => {
                        value = virtq.ready() as u32;
                    }
                    Err(_) => {
                        panic!(
                            "virtio_mmio_queue_access: wrong q_sel {:x} in read VIRTIO_MMIO_QUEUE_READY",
                            idx
                        );
                        // return;
                    }
                }
            }
            _ => {
                println!(
                    "virtio_mmio_queue_access: wrong reg_read, address {:x}",
                    emu_ctx.address
                );
                return;
            }
        }
        let idx = emu_ctx.reg;
        let val = value as usize;
        current_cpu().set_gpr(idx, val);
    } else {
        let idx = emu_ctx.reg;
        let value = current_cpu().get_gpr(idx);
        let q_sel = mmio.q_sel() as usize;
        match offset {
            VIRTIO_MMIO_QUEUE_SEL => mmio.set_q_sel(value as u32),
            VIRTIO_MMIO_QUEUE_NUM => {
                match mmio.vq(q_sel) {
                    Ok(virtq) => {
                        virtq.set_num(value);
                    }
                    Err(_) => {
                        panic!(
                            "virtio_mmio_queue_access: wrong q_sel {:x} in write VIRTIO_MMIO_QUEUE_NUM",
                            q_sel
                        );
                        // return;
                    }
                }
            }
            VIRTIO_MMIO_QUEUE_READY => match mmio.vq(q_sel) {
                Ok(virtq) => {
                    virtq.set_ready(value);
                    if value == VIRTQ_READY {
                        info!(
                            "VM {} virtio device {:x} queue {} ready",
                            active_vm_id(),
                            mmio.base(),
                            q_sel
                        );
                    } else {
                        warn!(
                            "VM {} virtio device {:x} queue {} init failed",
                            active_vm_id(),
                            mmio.base(),
                            q_sel
                        );
                    }
                }
                Err(_) => {
                    panic!(
                        "virtio_mmio_queue_access: wrong q_sel {:x} in write VIRTIO_MMIO_QUEUE_READY",
                        q_sel
                    );
                }
            },
            VIRTIO_MMIO_QUEUE_DESC_LOW => match mmio.vq(q_sel) {
                Ok(virtq) => {
                    virtq.or_desc_table_addr(value & u32::MAX as usize);
                }
                Err(_) => {
                    panic!(
                        "virtio_mmio_queue_access: wrong q_sel {:x} in write VIRTIO_MMIO_QUEUE_DESC_LOW",
                        q_sel
                    );
                }
            },
            VIRTIO_MMIO_QUEUE_DESC_HIGH => match mmio.vq(q_sel) {
                Ok(virtq) => {
                    virtq.or_desc_table_addr(value << 32);
                    let desc_table_addr = active_vm().unwrap().ipa2hva(virtq.desc_table_addr());
                    if desc_table_addr == 0 {
                        println!("virtio_mmio_queue_access: invalid desc_table_addr");
                        return;
                    }
                    virtq.set_desc_table(desc_table_addr);
                }
                Err(_) => {
                    panic!(
                        "virtio_mmio_queue_access: wrong q_sel {:x} in write VIRTIO_MMIO_QUEUE_DESC_HIGH",
                        q_sel
                    );
                }
            },
            VIRTIO_MMIO_QUEUE_AVAIL_LOW => match mmio.vq(q_sel) {
                Ok(virtq) => {
                    virtq.or_avail_addr(value & u32::MAX as usize);
                }
                Err(_) => {
                    panic!(
                        "virtio_mmio_queue_access: wrong q_sel {:x} in write VIRTIO_MMIO_QUEUE_AVAIL_LOW",
                        q_sel
                    );
                }
            },
            VIRTIO_MMIO_QUEUE_AVAIL_HIGH => match mmio.vq(q_sel) {
                Ok(virtq) => {
                    virtq.or_avail_addr(value << 32);
                    let avail_addr = active_vm().unwrap().ipa2hva(virtq.avail_addr());
                    if avail_addr == 0 {
                        println!("virtio_mmio_queue_access: invalid avail_addr");
                        return;
                    }
                    virtq.set_avail(avail_addr);
                }
                Err(_) => {
                    panic!(
                        "virtio_mmio_queue_access: wrong q_sel {:x} in write VIRTIO_MMIO_QUEUE_AVAIL_HIGH",
                        q_sel
                    );
                }
            },
            VIRTIO_MMIO_QUEUE_USED_LOW => match mmio.vq(q_sel) {
                Ok(virtq) => {
                    virtq.or_used_addr(value & u32::MAX as usize);
                }
                Err(_) => {
                    panic!(
                        "virtio_mmio_queue_access: wrong q_sel {:x} in write VIRTIO_MMIO_QUEUE_USED_LOW",
                        q_sel
                    );
                }
            },
            VIRTIO_MMIO_QUEUE_USED_HIGH => match mmio.vq(q_sel) {
                Ok(virtq) => {
                    virtq.or_used_addr(value << 32);
                    let used_addr = active_vm().unwrap().ipa2hva(virtq.used_addr());
                    if used_addr == 0 {
                        println!("virtio_mmio_queue_access: invalid used_addr");
                        return;
                    }
                    virtq.set_used(used_addr);
                }
                Err(_) => {
                    panic!(
                        "virtio_mmio_queue_access: wrong q_sel {:x} in write VIRTIO_MMIO_QUEUE_USED_HIGH",
                        q_sel
                    );
                }
            },
            _ => {
                println!("virtio_mmio_queue_access: wrong reg write {:#x}", emu_ctx.address);
            }
        }
    }
}

fn virtio_mmio_cfg_access(mmio: &VirtioMmio, emu_ctx: &EmuContext, offset: usize, write: bool) {
    if !write {
        let value = match offset {
            VIRTIO_MMIO_CONFIG_GENERATION => mmio.dev().generation() as u32,
            VIRTIO_MMIO_CONFIG..=0x1ff => match mmio.dev().desc() {
                super::dev::DevDesc::BlkDesc(blk_desc) => blk_desc.offset_data(offset - VIRTIO_MMIO_CONFIG),
                super::dev::DevDesc::NetDesc(net_desc) => net_desc.offset_data(offset - VIRTIO_MMIO_CONFIG),
                super::dev::DevDesc::Balloon(config) => config.read_config(offset - VIRTIO_MMIO_CONFIG),
                _ => {
                    error!("unknow desc type");
                    return;
                }
            },
            _ => {
                println!("virtio_mmio_cfg_access: wrong reg write {:#x}", emu_ctx.address);
                return;
            }
        };
        let idx = emu_ctx.reg;
        let val = value as usize;
        current_cpu().set_gpr(idx, val);
    } else {
        if (VIRTIO_MMIO_CONFIG..=0x1ff).contains(&offset) {
            let val = current_cpu().get_gpr(emu_ctx.reg) as u32;
            match mmio.dev().desc() {
                super::dev::DevDesc::Balloon(config) => config.write_config(offset - VIRTIO_MMIO_CONFIG, val),
                _ => {
                    error!("unknow desc type");
                }
            }
        }
    }
}

pub fn emu_virtio_mmio_init(vmid: usize, emu_cfg: &VmEmulatedDeviceConfig) -> Result<Box<dyn EmuDev>, ()> {
    let virt_dev_type = match emu_cfg.emu_type {
        EmuDeviceType::EmuDeviceTVirtioBlk => VirtioDeviceType::Block,
        EmuDeviceType::EmuDeviceTVirtioNet => VirtioDeviceType::Net,
        EmuDeviceType::EmuDeviceTVirtioConsole => VirtioDeviceType::Console,
        EmuDeviceType::VirtioBalloon => VirtioDeviceType::Balloon,
        _ => {
            println!("emu_virtio_mmio_init: unknown emulated device type");
            return Err(());
        }
    };
    let mut mmio = VirtioInner::new(virt_dev_type, emu_cfg);
    mmio.init(virt_dev_type);
    mmio.virtio_queue_init(virt_dev_type);
    let mmio = VirtioMmio(Arc::new(mmio));
    if emu_cfg.emu_type == EmuDeviceType::EmuDeviceTVirtioNet {
        let nic = mmio.clone();
        let mac = emu_cfg.cfg_list.iter().take(6).map(|x| *x as u8).collect::<Vec<_>>();
        super::mac::set_mac_info(&mac, vmid, nic);
    }
    Ok(Box::new(mmio))
}

impl EmuDev for VirtioMmio {
    fn emu_type(&self) -> EmuDeviceType {
        self.0.inner_const.emu_type
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }

    fn address_range(&self) -> core::ops::Range<usize> {
        self.0.inner_const.base..self.0.inner_const.base + self.0.inner_const.length
    }

    fn handler(&self, emu_ctx: &EmuContext) -> bool {
        let addr = emu_ctx.address;
        let offset = addr - self.base();
        let write = emu_ctx.write;

        if offset == VIRTIO_MMIO_QUEUE_NOTIFY && write {
            self.set_irt_stat(VIRTIO_MMIO_INT_VRING);
            // let q_sel = mmio.q_sel();
            // if q_sel as usize != current_cpu().get_gpr(emu_ctx.reg) {
            // println!("{} {}", q_sel as usize, current_cpu().get_gpr(emu_ctx.reg));
            // }
            // println!("in VIRTIO_MMIO_QUEUE_NOTIFY");

            if !self.notify_handler(current_cpu().get_gpr(emu_ctx.reg)) {
                println!("Failed to handle virtio mmio request!");
            }
        } else if offset == VIRTIO_MMIO_INTERRUPT_STATUS && !write {
            // println!("in VIRTIO_MMIO_INTERRUPT_STATUS");
            let idx = emu_ctx.reg;
            let val = self.irt_stat() as usize;
            current_cpu().set_gpr(idx, val);
        } else if offset == VIRTIO_MMIO_INTERRUPT_ACK && write {
            let idx = emu_ctx.reg;
            let val = self.irt_stat();
            self.set_irt_stat(val & !(current_cpu().get_gpr(idx) as u32));
            self.set_irt_ack(current_cpu().get_gpr(idx) as u32);
        } else if (VIRTIO_MMIO_MAGIC_VALUE..=VIRTIO_MMIO_GUEST_FEATURES_SEL).contains(&offset)
            || offset == VIRTIO_MMIO_STATUS
        {
            // println!("in virtio_mmio_prologue_access");
            virtio_mmio_prologue_access(self, emu_ctx, offset, write);
        } else if (VIRTIO_MMIO_QUEUE_SEL..=VIRTIO_MMIO_QUEUE_USED_HIGH).contains(&offset) {
            // println!("in virtio_mmio_queue_access");
            virtio_mmio_queue_access(self, emu_ctx, offset, write);
        } else if (VIRTIO_MMIO_CONFIG_GENERATION..=VIRTIO_MMIO_REGS_END).contains(&offset) {
            // println!("in virtio_mmio_cfg_access");
            virtio_mmio_cfg_access(self, emu_ctx, offset, write);
        } else {
            println!(
                "emu_virtio_mmio_handler: regs wrong {}, address {:#x}, offset {:#x}",
                if write { "write" } else { "read" },
                addr,
                offset
            );
            return false;
        }
        true
    }
}
