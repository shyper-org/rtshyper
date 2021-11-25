use alloc::sync::Arc;
use alloc::vec::Vec;

use spin::Mutex;

use crate::arch::Vgic;
use crate::device::VirtioMmio;
use crate::kernel::current_cpu;
use crate::lib::in_range;

pub const EMU_DEV_NUM_MAX: usize = 32;
pub static EMU_DEVS_LIST: Mutex<Vec<EmuDevEntry>> = Mutex::new(Vec::new());

#[derive(Clone)]
pub enum EmuDevs {
    Vgic(Arc<Vgic>),
    VirtioBlk(VirtioMmio),
    VirtioNet(VirtioMmio),
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

pub struct EmuDevEntry {
    vm_id: usize,
    id: usize,
    ipa: usize,
    size: usize,
    handler: EmuDevHandler,
}

pub enum EmuDeviceType {
    EmuDeviceTConsole,
    EmuDeviceTGicd,
    EmuDeviceTVirtioBlk,
    EmuDeviceTVirtioNet,
    EmuDeviceTShyper,
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
        "emu_handler: no emul handler for data abort ipa 0x{:x}",
        ipa
    );
    return false;
}

pub fn emu_register_dev(
    vm_id: usize,
    dev_id: usize,
    address: usize,
    size: usize,
    handler: EmuDevHandler,
) {
    let mut emu_devs_list = EMU_DEVS_LIST.lock();
    if emu_devs_list.len() >= EMU_DEV_NUM_MAX {
        panic!("emu_register_dev: can't register more devs");
    }

    for emu_dev in &*emu_devs_list {
        if vm_id != emu_dev.vm_id {
            continue;
        }
        if in_range(address, emu_dev.ipa, emu_dev.size - 1)
            || in_range(emu_dev.ipa, address, size - 1)
        {
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
