use super::gic::*;
use crate::kernel::Vcpu;
use crate::kernel::Vm;
use crate::kernel::{ipi_register, IpiMessage, IpiType};
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

struct VgicInt {
    owner: Option<Arc<Mutex<Vcpu>>>,
    id: u16,
    hw: bool,
    in_lr: bool,
    lr: u16,
    enabled: bool,
    state: IrqState,
    prio: u8,
    targets: u8,
    cfg: u8,
}

impl VgicInt {
    fn new(id: usize) -> VgicInt {
        VgicInt {
            owner: None,
            id: (id + GIC_PRIVINT_NUM) as u16,
            hw: false,
            in_lr: false,
            lr: 0,
            enabled: false,
            state: IrqState::IrqSInactive,
            prio: 0xff,
            targets: 0,
            cfg: 0,
        }
    }

    fn priv_new(id: usize, owner: Arc<Mutex<Vcpu>>, targets: usize, enabled: bool) -> VgicInt {
        VgicInt {
            owner: Some(owner),
            id: id as u16,
            hw: false,
            in_lr: false,
            lr: 0,
            enabled,
            state: IrqState::IrqSInactive,
            prio: 0xff,
            targets: targets as u8,
            cfg: 0,
        }
    }
}

struct Vgicd {
    ctlr: u32,
    typer: u32,
    iidr: u32,
    interrupts: Vec<Mutex<VgicInt>>,
}

impl Vgicd {
    fn default() -> Vgicd {
        Vgicd {
            ctlr: 0,
            typer: 0,
            iidr: 0,
            interrupts: Vec::new(),
        }
    }
}

#[derive(Copy, Clone)]
struct Sgis {
    pend: u8,
    act: u8,
}

impl Sgis {
    fn default() -> Sgis {
        Sgis { pend: 0, act: 0 }
    }
}

struct VgicCpuPriv {
    // gich: GicHypervisorInterfaceBlock,
    curr_lrs: [u16; GIC_LIST_REGS_NUM],
    sgis: [Sgis; GIC_SGIS_NUM],
    interrupts: Vec<Mutex<VgicInt>>,
}

impl VgicCpuPriv {
    fn default() -> VgicCpuPriv {
        VgicCpuPriv {
            curr_lrs: [0; GIC_LIST_REGS_NUM],
            sgis: [Sgis::default(); GIC_SGIS_NUM],
            interrupts: Vec::new(),
        }
    }
}

pub struct Vgic {
    vgicd: Mutex<Vgicd>,
    cpu_priv: Mutex<Vec<VgicCpuPriv>>,
}

impl Vgic {
    fn default() -> Vgic {
        Vgic {
            vgicd: Mutex::new(Vgicd::default()),
            cpu_priv: Mutex::new(Vec::new()),
        }
    }
}

// TODO
pub fn gic_maintenance_handler(arg: usize, source: usize) {
    unimplemented!()
}

// TODO
use crate::device::EmuContext;
pub fn emu_intc_handler(emu_dev_id: usize, emu_ctx: &EmuContext) -> bool {
    true
}

fn vgic_ipi_handler(msg: &IpiMessage) {}

use crate::device::EmuDevs;
pub fn emu_intc_init(vm: Vm, emu_dev_id: usize) {
    let vgic_cpu_num = vm.config().cpu.num;
    let vgic = Arc::new(Vgic::default());

    let mut vgicd = vgic.vgicd.lock();
    vgicd.typer = (GICD.typer() & GICD_TYPER_CPUNUM_MSK as u32)
        | (((vm.cpu_num() - 1) << GICD_TYPER_CPUNUM_OFF) & GICD_TYPER_CPUNUM_MSK) as u32;
    vgicd.iidr = (GICD.iidr());

    for i in 0..GIC_SPI_MAX {
        vgicd.interrupts.push(Mutex::new(VgicInt::new(i)));
    }
    drop(vgicd);

    for i in 0..vgic_cpu_num {
        let mut cpu_priv = VgicCpuPriv::default();
        for int_idx in 0..GIC_PRIVINT_NUM {
            let vcpu_arc = vm.vcpu(i);
            let vcpu = vcpu_arc.lock();
            let phys_id = vcpu.phys_id;
            drop(vcpu);

            cpu_priv.interrupts.push(Mutex::new(VgicInt::priv_new(
                int_idx,
                vcpu_arc.clone(),
                phys_id,
                int_idx < GIC_SGIS_NUM,
            )));
        }

        let mut vgic_cpu_priv = vgic.cpu_priv.lock();
        vgic_cpu_priv.push(cpu_priv);
    }

    vm.set_emu_devs(emu_dev_id, EmuDevs::Vgic(vgic.clone()));

    if !ipi_register(IpiType::IpiTIntc, vgic_ipi_handler) {
        panic!(
            "emu_intc_init: failed to register ipi {}",
            IpiType::IpiTIntc as usize
        )
    }
}
