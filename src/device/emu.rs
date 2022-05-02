use alloc::sync::Arc;
use alloc::vec::Vec;

use spin::Mutex;

use crate::arch::Vgic;
use crate::device::VirtioMmio;
use crate::kernel::{current_cpu, vm};
use crate::lib::in_range;

pub const EMU_DEV_NUM_MAX: usize = 32;
pub static EMU_DEVS_LIST: Mutex<Vec<EmuDevEntry>> = Mutex::new(Vec::new());

#[derive(Clone)]
pub enum EmuDevs {
    Vgic(Arc<Vgic>),
    VirtioBlk(VirtioMmio),
    VirtioNet(VirtioMmio),
    VirtioConsole(VirtioMmio),
    None,
}

impl EmuDevs {
    pub fn migrate_save(&mut self, src_dev: EmuDevs) {
        match self {
            EmuDevs::Vgic(vgic) => {
                // TODO not sure this can work;
                if let EmuDevs::Vgic(src_vgic) = src_dev {
                    *vgic = src_vgic.clone();
                } else {
                    println!("EmuDevs::migrate_save: illegal src dev type for vgic");
                }
            }
            EmuDevs::VirtioBlk(mmio) => {
                if let EmuDevs::VirtioBlk(src_mmio) = src_dev {
                    mmio.migrate_save(src_mmio);
                } else {
                    println!("EmuDevs::migrate_save: illegal src dev type for virtio blk");
                }
            }
            EmuDevs::VirtioNet(mmio) => {
                if let EmuDevs::VirtioNet(src_mmio) = src_dev {
                    mmio.migrate_save(src_mmio);
                } else {
                    println!("EmuDevs::migrate_save: illegal src dev type for virtio net");
                }
            }
            EmuDevs::VirtioConsole(mmio) => {
                if let EmuDevs::VirtioConsole(src_mmio) = src_dev {
                    mmio.migrate_save(src_mmio);
                } else {
                    println!("EmuDevs::migrate_save: illegal src dev type for virtio console");
                }
            }
            EmuDevs::None => {}
        }
    }
}

pub struct EmuContext {
    pub address: usize,
    pub width: usize,
    pub write: bool,
    pub sign_ext: bool,
    pub reg: usize,
    pub reg_width: usize,
}

pub struct EmuDevEntry {
    vm_id: usize,
    id: usize,
    ipa: usize,
    size: usize,
    handler: EmuDevHandler,
}

#[derive(Clone, Copy, PartialEq)]
pub enum EmuDeviceType {
    EmuDeviceTConsole = 0,
    EmuDeviceTGicd = 1,
    EmuDeviceTGPPT = 2,
    EmuDeviceTVirtioBlk = 3,
    EmuDeviceTVirtioNet = 4,
    EmuDeviceTVirtioConsole = 5,
    EmuDeviceTShyper = 6,
    EmuDeviceTVirtioBlkMediated = 7,
}

impl EmuDeviceType {
    pub fn from_usize(value: usize) -> EmuDeviceType {
        match value {
            0 => EmuDeviceType::EmuDeviceTConsole,
            1 => EmuDeviceType::EmuDeviceTGicd,
            2 => EmuDeviceType::EmuDeviceTGPPT,
            3 => EmuDeviceType::EmuDeviceTVirtioBlk,
            4 => EmuDeviceType::EmuDeviceTVirtioNet,
            5 => EmuDeviceType::EmuDeviceTVirtioConsole,
            6 => EmuDeviceType::EmuDeviceTShyper,
            7 => EmuDeviceType::EmuDeviceTVirtioBlkMediated,
            _ => panic!("Unknown  EmuDeviceType value: {}", value),
        }
    }
}

pub type EmuDevHandler = fn(usize, &EmuContext) -> bool;

// TO CHECK
pub fn emu_handler(emu_ctx: &EmuContext) -> bool {
    let ipa = emu_ctx.address;
    let emu_devs_list = EMU_DEVS_LIST.lock();

    for emu_dev in &*emu_devs_list {
        let active_vcpu = current_cpu().active_vcpu.clone().unwrap();
        if active_vcpu.vm_id() == emu_dev.vm_id && in_range(ipa, emu_dev.ipa, emu_dev.size - 1) {
            let handler = emu_dev.handler;
            let id = emu_dev.id;
            drop(emu_devs_list);
            return handler(id, emu_ctx);
        }
    }
    println!(
        "emu_handler: no emul handler for Core {} data abort ipa 0x{:x}",
        current_cpu().id,
        ipa
    );
    return false;
}

pub fn emu_register_dev(vm_id: usize, dev_id: usize, address: usize, size: usize, handler: EmuDevHandler) {
    let mut emu_devs_list = EMU_DEVS_LIST.lock();
    if emu_devs_list.len() >= EMU_DEV_NUM_MAX {
        panic!("emu_register_dev: can't register more devs");
    }

    for emu_dev in &*emu_devs_list {
        if vm_id != emu_dev.vm_id {
            continue;
        }
        if in_range(address, emu_dev.ipa, emu_dev.size - 1) || in_range(emu_dev.ipa, address, size - 1) {
            panic!("emu_register_dev: duplicated emul address region: prev address 0x{:x} size 0x{:x}, next address 0x{:x} size 0x{:x}", emu_dev.ipa, emu_dev.size, address, size);
        }
    }

    emu_devs_list.push(EmuDevEntry {
        vm_id,
        id: dev_id,
        ipa: address,
        size,
        handler,
    });
}

pub fn emu_remove_dev(vm_id: usize, dev_id: usize, address: usize, size: usize) {
    let mut emu_devs_list = EMU_DEVS_LIST.lock();
    for (idx, emu_dev) in emu_devs_list.iter().enumerate() {
        if vm_id == emu_dev.vm_id && emu_dev.ipa == address && emu_dev.id == dev_id && emu_dev.size == size {
            emu_devs_list.remove(idx);
            return;
        }
    }
    panic!(
        "emu_remove_dev: emu dev not exist address 0x{:x} size 0x{:x}",
        address, size
    );
}
