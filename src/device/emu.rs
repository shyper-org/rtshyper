use core::ops::Range;

use alloc::vec::Vec;

use spin::RwLock;

use crate::kernel::{current_cpu, active_vm};
use crate::util::downcast::Downcast;

pub trait EmuDev: Downcast + Send + Sync {
    fn emu_type(&self) -> EmuDeviceType;
    fn address_range(&self) -> Range<usize>;
    fn handler(&self, emu_ctx: &EmuContext) -> bool;
}

pub struct EmuContext {
    pub address: usize,
    pub width: usize,
    pub write: bool,
    pub sign_ext: bool,
    pub reg: usize,
    pub reg_width: usize,
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
    VirtioBalloon = 9,
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
            9 => EmuDeviceType::VirtioBalloon,
            _ => panic!("Unknown EmuDeviceType value: {}", value),
        }
    }
}

type EmuDevHandler = fn(usize, &EmuContext) -> bool;

// TO CHECK
pub fn emu_handler(emu_ctx: &EmuContext) -> bool {
    let ipa = emu_ctx.address;

    if let Some(emu_dev) = active_vm().unwrap().find_emu_dev(ipa) {
        return emu_dev.handler(emu_ctx);
    }

    error!(
        "emu_handler: no emul handler for Core {} data abort ipa {:#x}",
        current_cpu().id,
        ipa
    );
    false
}

static EMU_REGS_LIST: RwLock<Vec<EmuRegEntry>> = RwLock::new(Vec::new());

pub fn emu_reg_handler(emu_ctx: &EmuContext) -> bool {
    let address = emu_ctx.address;
    let emu_regs_list = EMU_REGS_LIST.read();

    let active_vcpu = current_cpu().active_vcpu.as_ref().unwrap();
    let vm_id = active_vcpu.vm_id();

    for emu_reg in emu_regs_list.iter() {
        if emu_reg.addr == address {
            let handler = emu_reg.handler;
            drop(emu_regs_list);
            return handler(vm_id, emu_ctx);
        }
    }
    error!(
        "emu_reg_handler: no handler for Core{} {} reg ({:#x})",
        current_cpu().id,
        if emu_ctx.write { "write" } else { "read" },
        address
    );
    false
}

pub fn emu_register_reg(emu_type: EmuRegType, address: usize, handler: EmuRegHandler) {
    let mut emu_regs_list = EMU_REGS_LIST.write();

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

type EmuRegHandler = EmuDevHandler;

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
