use alloc::sync::Arc;
use alloc::vec::Vec;

use spin::Mutex;

// pub const VIRTIO_MMIO_MAGIC_VALUE: usize = 0x000;
// pub const VIRTIO_MMIO_DEVICE_ID: usize = 0x008;
// pub const VIRTIO_MMIO_VENDOR_ID: usize = 0x00c;
use crate::config::VmEmulatedDeviceConfig;
use crate::device::{EmuContext, virtio_mediated_blk_notify_handler};
use crate::device::{EmuDevs, VirtioDeviceType};
use crate::device::{VirtioQueue, Virtq};
use crate::device::{VIRTQUEUE_BLK_MAX_SIZE, VIRTQUEUE_CONSOLE_MAX_SIZE, VIRTQUEUE_NET_MAX_SIZE};
use crate::device::VirtDev;
use crate::device::VIRTQ_READY;
use crate::driver::VIRTIO_MMIO_MAGIC_VALUE;
use crate::kernel::{current_cpu, vm_ipa2pa};
use crate::kernel::{active_vm, active_vm_id};
use crate::kernel::Vm;

pub const VIRTIO_F_VERSION_1: usize = 1 << 32;
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

    pub fn save_regs(&mut self, src: &VirtMmioRegs) {
        self.magic = src.magic;
        self.version = src.version;
        self.device_id = src.device_id;
        self.vendor_id = src.vendor_id;
        self.dev_feature = src.dev_feature;
        self.dev_feature_sel = src.dev_feature_sel;
        self.drv_feature = src.drv_feature;
        self.drv_feature_sel = src.drv_feature_sel;
        self.q_sel = src.q_sel;
        self.q_num_max = src.q_num_max;
        self.irt_stat = src.irt_stat;
        self.irt_ack = src.irt_ack;
        self.dev_stat = src.dev_stat;
    }
}

#[derive(Clone)]
pub struct VirtioMmio {
    inner: Arc<Mutex<VirtioMmioInner>>,
}

impl VirtioQueue for VirtioMmio {
    fn virtio_queue_init(&self, dev_type: VirtioDeviceType) {
        match dev_type {
            VirtioDeviceType::Block => {
                self.set_q_num_max(VIRTQUEUE_BLK_MAX_SIZE as u32);
                let mut inner = self.inner.lock();
                inner.vq.push(Virtq::default());
                inner.vq[0].reset(0);
                use crate::device::virtio_blk_notify_handler;
                if inner.dev.mediated() {
                    inner.vq[0].set_notify_handler(virtio_mediated_blk_notify_handler);
                } else {
                    inner.vq[0].set_notify_handler(virtio_blk_notify_handler);
                }
            }
            VirtioDeviceType::Net => {
                self.set_q_num_max(VIRTQUEUE_NET_MAX_SIZE as u32);
                let mut inner = self.inner.lock();
                // Not support feature VIRTIO_NET_F_CTRL_VQ (no control queue)
                for i in 0..2 {
                    inner.vq.push(Virtq::default());
                    inner.vq[i].reset(i);
                    use crate::device::virtio_net_notify_handler;
                    inner.vq[i].set_notify_handler(virtio_net_notify_handler);
                }
            }
            VirtioDeviceType::Console => {
                self.set_q_num_max(VIRTQUEUE_CONSOLE_MAX_SIZE as u32);
                let mut inner = self.inner.lock();
                for i in 0..4 {
                    inner.vq.push(Virtq::default());
                    inner.vq[i].reset(i);
                    use crate::device::virtio_console_notify_handler;
                    inner.vq[i].set_notify_handler(virtio_console_notify_handler);
                }
            }
            VirtioDeviceType::None => {
                panic!("virtio_queue_init: unknown emulated device type");
            }
        }
    }

    fn virtio_queue_reset(&self, index: usize) {
        let inner = self.inner.lock();
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

    pub fn dev_init(&self, dev_type: VirtioDeviceType, config: &VmEmulatedDeviceConfig, mediated: bool) {
        let inner = self.inner.lock();
        inner.dev.init(dev_type, config, mediated)
    }

    // virtio_dev_reset
    pub fn dev_reset(&self) {
        let mut inner = self.inner.lock();
        inner.regs.dev_stat = 0;
        inner.regs.irt_stat = 0;
        let idx = inner.regs.q_sel as usize;
        inner.vq[idx].set_ready(0);
        for (idx, virtq) in inner.vq.iter().enumerate() {
            virtq.reset(idx);
        }
        inner.dev.set_activated(false);
    }

    pub fn set_irt_stat(&self, irt_stat: u32) {
        let mut inner = self.inner.lock();
        inner.regs.irt_stat = irt_stat;
    }

    pub fn set_irt_ack(&self, irt_ack: u32) {
        let mut inner = self.inner.lock();
        inner.regs.irt_ack = irt_ack;
    }

    pub fn set_q_sel(&self, q_sel: u32) {
        let mut inner = self.inner.lock();
        inner.regs.q_sel = q_sel;
    }

    pub fn set_dev_stat(&self, dev_stat: u32) {
        let mut inner = self.inner.lock();
        inner.regs.dev_stat = dev_stat;
    }

    pub fn set_q_num_max(&self, q_num_max: u32) {
        let mut inner = self.inner.lock();
        inner.regs.q_num_max = q_num_max;
    }

    pub fn set_dev_feature(&self, dev_feature: u32) {
        let mut inner = self.inner.lock();
        inner.regs.dev_feature = dev_feature;
    }

    pub fn set_dev_feature_sel(&self, dev_feature_sel: u32) {
        let mut inner = self.inner.lock();
        inner.regs.dev_feature_sel = dev_feature_sel;
    }

    pub fn set_drv_feature(&self, drv_feature: u32) {
        let mut inner = self.inner.lock();
        inner.regs.drv_feature = drv_feature;
    }

    pub fn set_drv_feature_sel(&self, drv_feature_sel: u32) {
        let mut inner = self.inner.lock();
        inner.regs.drv_feature = drv_feature_sel;
    }

    pub fn or_driver_feature(&self, driver_features: usize) {
        let mut inner = self.inner.lock();
        inner.driver_features |= driver_features;
    }

    pub fn dev(&self) -> VirtDev {
        let inner = self.inner.lock();
        inner.dev.clone()
    }

    pub fn q_sel(&self) -> u32 {
        let inner = self.inner.lock();
        inner.regs.q_sel
    }

    pub fn magic(&self) -> u32 {
        let inner = self.inner.lock();
        inner.regs.magic
    }

    pub fn version(&self) -> u32 {
        let inner = self.inner.lock();
        inner.regs.version
    }

    pub fn device_id(&self) -> u32 {
        let inner = self.inner.lock();
        inner.regs.device_id
    }

    pub fn vendor_id(&self) -> u32 {
        let inner = self.inner.lock();
        inner.regs.vendor_id
    }

    pub fn dev_stat(&self) -> u32 {
        let inner = self.inner.lock();
        inner.regs.dev_stat
    }

    pub fn dev_feature_sel(&self) -> u32 {
        let inner = self.inner.lock();
        inner.regs.dev_feature_sel
    }

    pub fn drv_feature_sel(&self) -> u32 {
        let inner = self.inner.lock();
        inner.regs.drv_feature_sel
    }

    pub fn q_num_max(&self) -> u32 {
        let inner = self.inner.lock();
        inner.regs.q_num_max
    }

    pub fn irt_stat(&self) -> u32 {
        let inner = self.inner.lock();
        inner.regs.irt_stat
    }

    pub fn vq(&self, idx: usize) -> Result<Virtq, ()> {
        let inner = self.inner.lock();
        if idx >= inner.vq.len() {
            return Err(());
        }
        return Ok(inner.vq[idx].clone());
    }

    pub fn id(&self) -> usize {
        let inner = self.inner.lock();
        inner.id
    }

    pub fn notify_handler(&self, idx: usize) -> bool {
        let inner = self.inner.lock();
        if idx >= inner.vq.len() {
            return false;
        }
        let vq = inner.vq[idx].clone();
        drop(inner);
        return vq.call_notify_handler(self.clone());
    }

    pub fn save_mmio(&self, virtio_mmio: VirtioMmio, notify_handler: Option<fn(Virtq, VirtioMmio, Vm) -> bool>) {
        let mut dst_dev = self.inner.lock();
        let src_dev = virtio_mmio.inner.lock();
        dst_dev.id = src_dev.id;
        dst_dev.driver_features = src_dev.driver_features;
        dst_dev.regs.save_regs(&src_dev.regs);
        dst_dev.dev.save_virt_dev(src_dev.dev.clone());
        for vq in src_dev.vq.iter() {
            let new_vq = Virtq::default();
            new_vq.save_vq(vq.clone(), notify_handler);
            dst_dev.vq.push(new_vq);
        }
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

fn virtio_mmio_prologue_access(mmio: VirtioMmio, emu_ctx: &EmuContext, offset: usize, write: bool) {
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
                println!("virtio_be_init_handler wrong reg_read, address=0x{:x}", emu_ctx.address);
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
                    println!("VM {} virtio devive {} is reset", active_vm_id(), mmio.id());
                } else if mmio.dev_stat() == 0xf {
                    mmio.dev().set_activated(true);
                    println!("VM {} virtio devive {} init ok", active_vm_id(), mmio.id());
                }
            }
            _ => {
                println!("virtio_mmio_prologue_access: wrong reg write 0x{:x}", emu_ctx.address);
                return;
            }
        }
    }
}

fn virtio_mmio_queue_access(mmio: VirtioMmio, emu_ctx: &EmuContext, offset: usize, write: bool) {
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
                        println!(
                            "VM {} virtio device {} queue {} ready",
                            active_vm_id(),
                            mmio.id(),
                            q_sel
                        );
                    } else {
                        println!(
                            "VM {} virtio device {} queue {} init failed",
                            active_vm_id(),
                            mmio.id(),
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
                    let desc_table_addr = vm_ipa2pa(active_vm().unwrap(), virtq.desc_table_addr());
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
                    let avail_addr = vm_ipa2pa(active_vm().unwrap(), virtq.avail_addr());
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
                    let used_addr = vm_ipa2pa(active_vm().unwrap(), virtq.used_addr());
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
                println!("virtio_mmio_queue_access: wrong reg write 0x{:x}", emu_ctx.address);
            }
        }
    }
}

fn virtio_mmio_cfg_access(mmio: VirtioMmio, emu_ctx: &EmuContext, offset: usize, write: bool) {
    if !write {
        let value;
        match offset {
            VIRTIO_MMIO_CONFIG_GENERATION => {
                value = mmio.dev().generation() as u32;
            }
            VIRTIO_MMIO_CONFIG..=0x1ff => match mmio.dev().desc() {
                super::DevDesc::BlkDesc(blk_desc) => {
                    value = blk_desc.offset_data(offset - VIRTIO_MMIO_CONFIG);
                }
                super::DevDesc::NetDesc(net_desc) => {
                    value = net_desc.offset_data(offset - VIRTIO_MMIO_CONFIG);
                }
                _ => {
                    panic!("unknow desc type");
                }
            },
            _ => {
                println!("virtio_mmio_cfg_access: wrong reg write 0x{:x}", emu_ctx.address);
                return;
            }
        }
        let idx = emu_ctx.reg;
        let val = value as usize;
        current_cpu().set_gpr(idx, val);
    } else {
        println!("virtio_mmio_cfg_access: wrong reg write 0x{:x}", emu_ctx.address);
    }
}

pub fn emu_virtio_mmio_init(vm: Vm, emu_dev_id: usize, mediated: bool) -> bool {
    let virt_dev_type: VirtioDeviceType;
    let vm_cfg = vm.config();
    let mmio = VirtioMmio::new(emu_dev_id);
    match vm_cfg.emulated_device_list()[emu_dev_id].emu_type {
        crate::device::EmuDeviceType::EmuDeviceTVirtioBlk => {
            virt_dev_type = VirtioDeviceType::Block;
            vm.set_emu_devs(emu_dev_id, EmuDevs::VirtioBlk(mmio.clone()));
        }
        crate::device::EmuDeviceType::EmuDeviceTVirtioNet => {
            virt_dev_type = VirtioDeviceType::Net;
            vm.set_emu_devs(emu_dev_id, EmuDevs::VirtioNet(mmio.clone()));
        }
        crate::device::EmuDeviceType::EmuDeviceTVirtioConsole => {
            virt_dev_type = VirtioDeviceType::Console;
            vm.set_emu_devs(emu_dev_id, EmuDevs::VirtioConsole(mmio.clone()));
        }
        _ => {
            println!("emu_virtio_mmio_init: unknown emulated device type");
            return false;
        }
    }

    mmio.mmio_reg_init(virt_dev_type);
    mmio.dev_init(virt_dev_type, &vm_cfg.emulated_device_list()[emu_dev_id], mediated);
    // no need to set vm_if_list
    mmio.virtio_queue_init(virt_dev_type);

    true
}

pub fn emu_virtio_mmio_handler(emu_dev_id: usize, emu_ctx: &EmuContext) -> bool {
    let vm = match active_vm() {
        Some(vm) => vm,
        None => {
            panic!("emu_virtio_mmio_handler: current vcpu.vm is none");
        }
    };

    let mmio = match vm.emu_dev(emu_dev_id) {
        EmuDevs::VirtioBlk(blk) => blk,
        EmuDevs::VirtioNet(net) => net,
        EmuDevs::VirtioConsole(console) => console,
        _ => {
            panic!("emu_virtio_mmio_handler: illegal mmio dev type")
        }
    };

    let addr = emu_ctx.address;
    let offset = addr - vm.config().emulated_device_list()[emu_dev_id].base_ipa;
    let write = emu_ctx.write;

    // if vm.vm_id() == 1 && emu_dev_id == 2 {
    //     println!("### emu_virtio_mmio_handler offset {:x} ###", offset);
    // }
    if offset == VIRTIO_MMIO_QUEUE_NOTIFY && write {
        mmio.set_irt_stat(VIRTIO_MMIO_INT_VRING as u32);
        let q_sel = mmio.q_sel();
        // println!("in VIRTIO_MMIO_QUEUE_NOTIFY");

        if !mmio.notify_handler(q_sel as usize) {
            println!("Failed to handle virtio mmio request!");
        }
    } else if offset == VIRTIO_MMIO_INTERRUPT_STATUS && !write {
        // println!("in VIRTIO_MMIO_INTERRUPT_STATUS");
        let idx = emu_ctx.reg;
        let val = mmio.irt_stat() as usize;
        current_cpu().set_gpr(idx, val);
    } else if offset == VIRTIO_MMIO_INTERRUPT_ACK && write {
        // println!("in VIRTIO_MMIO_INTERRUPT_ACK");
        let idx = emu_ctx.reg;
        mmio.set_irt_ack(current_cpu().get_gpr(idx) as u32);
    } else if (VIRTIO_MMIO_MAGIC_VALUE <= offset && offset <= VIRTIO_MMIO_GUEST_FEATURES_SEL)
        || offset == VIRTIO_MMIO_STATUS
    {
        // println!("in virtio_mmio_prologue_access");
        virtio_mmio_prologue_access(mmio.clone(), emu_ctx, offset, write);
    } else if VIRTIO_MMIO_QUEUE_SEL <= offset && offset <= VIRTIO_MMIO_QUEUE_USED_HIGH {
        // println!("in virtio_mmio_queue_access");
        virtio_mmio_queue_access(mmio.clone(), emu_ctx, offset, write);
    } else if VIRTIO_MMIO_CONFIG_GENERATION <= offset && offset <= VIRTIO_MMIO_REGS_END {
        // println!("in virtio_mmio_cfg_access");
        virtio_mmio_cfg_access(mmio.clone(), emu_ctx, offset, write);
    } else {
        println!(
            "emu_virtio_mmio_handler: regs wrong {}, address 0x{:x}, offset 0x{:x}",
            if write { "write" } else { "read" },
            addr,
            offset
        );
        return false;
    }
    return true;
}
