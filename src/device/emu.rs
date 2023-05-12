use core::ops::Range;

use alloc::sync::Arc;
use alloc::vec::Vec;

use spin::Mutex;

use crate::arch::Vgic;
use crate::device::VirtioMmio;
use crate::kernel::current_cpu;

static EMU_DEVS_LIST: Mutex<Vec<EmuDevEntry>> = Mutex::new(Vec::new());

#[derive(Clone)]
pub enum EmuDevs {
    Vgic(Arc<Vgic>),
    VirtioBlk(VirtioMmio),
    VirtioNet(VirtioMmio),
    VirtioConsole(VirtioMmio),
    None,
}

pub struct EmuContext {
    pub address: usize,
    pub width: usize,
    pub write: bool,
    pub sign_ext: bool,
    pub reg: usize,
    pub reg_width: usize,
}

#[allow(dead_code)]
struct EmuDevEntry {
    pub emu_type: EmuDeviceType,
    pub vm_id: usize,
    pub id: usize,
    pub ipa: usize,
    pub size: usize,
    pub handler: EmuDevHandler,
}

impl EmuDevEntry {
    #[inline]
    fn address_range(&self) -> Range<usize> {
        self.ipa..self.ipa + self.size
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmuDeviceType {
    EmuDeviceTConsole = 0,
    EmuDeviceTGicd = 1,
    EmuDeviceTGPPT = 2,
    EmuDeviceTVirtioBlk = 3,
    EmuDeviceTVirtioNet = 4,
    EmuDeviceTVirtioConsole = 5,
    EmuDeviceTShyper = 6,
    EmuDeviceTVirtioBlkMediated = 7,
    EmuDeviceTIOMMU = 8,
}

impl EmuDeviceType {
    pub fn removable(&self) -> bool {
        matches!(
            self,
            EmuDeviceType::EmuDeviceTGicd
                | EmuDeviceType::EmuDeviceTGPPT
                | EmuDeviceType::EmuDeviceTVirtioBlk
                | EmuDeviceType::EmuDeviceTVirtioNet
                | EmuDeviceType::EmuDeviceTVirtioConsole
        )
    }
}

impl From<usize> for EmuDeviceType {
    fn from(value: usize) -> Self {
        match value {
            0 => EmuDeviceType::EmuDeviceTConsole,
            1 => EmuDeviceType::EmuDeviceTGicd,
            2 => EmuDeviceType::EmuDeviceTGPPT,
            3 => EmuDeviceType::EmuDeviceTVirtioBlk,
            4 => EmuDeviceType::EmuDeviceTVirtioNet,
            5 => EmuDeviceType::EmuDeviceTVirtioConsole,
            6 => EmuDeviceType::EmuDeviceTShyper,
            7 => EmuDeviceType::EmuDeviceTVirtioBlkMediated,
            8 => EmuDeviceType::EmuDeviceTIOMMU,
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
        if active_vcpu.vm_id() == emu_dev.vm_id && emu_dev.address_range().contains(&ipa) {
            let handler = emu_dev.handler;
            let id = emu_dev.id;
            drop(emu_devs_list);
            return handler(id, emu_ctx);
        }
    }
    println!(
        "emu_handler: no emul handler for Core {} data abort ipa {:#x}",
        current_cpu().id,
        ipa
    );
    false
}

pub fn emu_register_dev(
    emu_type: EmuDeviceType,
    vm_id: usize,
    dev_id: usize,
    address: usize,
    size: usize,
    handler: EmuDevHandler,
) {
    let mut emu_devs_list = EMU_DEVS_LIST.lock();
    for emu_dev in &*emu_devs_list {
        if vm_id != emu_dev.vm_id {
            continue;
        }
        if emu_dev.address_range().contains(&address) || (address..address + size).contains(&emu_dev.ipa) {
            panic!("emu_register_dev: duplicated emul address region: prev address {:#x} size {:#x}, next address {:#x} size {:#x}", emu_dev.ipa, emu_dev.size, address, size);
        }
    }

    emu_devs_list.push(EmuDevEntry {
        emu_type,
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
        "emu_remove_dev: emu dev not exist address {:#x} size {:#x}",
        address, size
    );
}

static EMU_REGS_LIST: Mutex<Vec<EmuRegEntry>> = Mutex::new(Vec::new());

pub fn emu_reg_handler(emu_ctx: &EmuContext) -> bool {
    let address = emu_ctx.address;
    let emu_regs_list = EMU_REGS_LIST.lock();

    let active_vcpu = current_cpu().active_vcpu.clone().unwrap();
    let vm_id = active_vcpu.vm_id();

    for emu_reg in emu_regs_list.iter() {
        if emu_reg.addr == address {
            let handler = emu_reg.handler;
            drop(emu_regs_list);
            return handler(vm_id, emu_ctx);
        }
    }
    println!(
        "emu_reg_handler: no handler for Core{} {} reg ({:#x})",
        current_cpu().id,
        if emu_ctx.write { "write" } else { "read" },
        address
    );
    false
}

pub fn emu_register_reg(emu_type: EmuRegType, address: usize, handler: EmuRegHandler) {
    let mut emu_regs_list = EMU_REGS_LIST.lock();

    for emu_reg in emu_regs_list.iter() {
        if address == emu_reg.addr {
            warn!(
                "emu_register_reg: duplicated emul reg addr: prev address {:#x}",
                address
            );
            return;
        }
    }

    emu_regs_list.push(EmuRegEntry {
        emu_type,
        addr: address,
        handler,
    });
}

pub type EmuRegHandler = EmuDevHandler;

pub struct EmuRegEntry {
    pub emu_type: EmuRegType,
    // pub vm_id: usize,
    // pub id: usize,
    pub addr: usize,
    pub handler: EmuRegHandler,
}

pub enum EmuRegType {
    SysReg,
}
