use super::gic::*;
use crate::arch::GICH;
use crate::kernel::Vcpu;
use crate::kernel::Vm;
use crate::kernel::{active_vcpu, cpu_id};
use crate::kernel::{ipi_register, IpiInnerMsg, IpiMessage, IpiType};
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

    fn owner_vm(&self) -> Vm {
        let owner = self.owner.as_ref().unwrap().lock();
        owner.vm.as_ref().unwrap().clone()
    }

    fn owner_id(&self) -> usize {
        let owner = self.owner.as_ref().unwrap().lock();
        owner.id
    }

    fn owner_phys_id(&self) -> usize {
        let owner = self.owner.as_ref().unwrap().lock();
        owner.phys_id
    }

    fn owner_vm_id(&self) -> usize {
        let owner = self.owner.as_ref().unwrap().lock();
        owner.vm_id()
    }
}

struct Vgicd {
    ctlr: u32,
    typer: u32,
    iidr: u32,
    interrupts: Vec<Arc<Mutex<VgicInt>>>,
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
    interrupts: Vec<Arc<Mutex<VgicInt>>>,
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

    fn cpu_priv_interrupt(&self, cpu_id: usize, idx: usize) -> Arc<Mutex<VgicInt>> {
        let cpu_priv = self.cpu_priv.lock();
        cpu_priv[cpu_id].interrupts[idx].clone()
    }

    fn vgicd_interrupt(&self, idx: usize) -> Arc<Mutex<VgicInt>> {
        let vgicd = self.vgicd.lock();
        vgicd.interrupts[idx].clone()
    }

    fn get_int(&self, vcpu: Arc<Mutex<Vcpu>>, int_id: usize) -> Option<Arc<Mutex<VgicInt>>> {
        if int_id < GIC_PRIVINT_NUM {
            let vcpu_lock = vcpu.lock();
            let vcpu_id = vcpu_lock.id;
            return Some(self.cpu_priv_interrupt(vcpu_id, int_id));
        } else if int_id >= GIC_PRIVINT_NUM && int_id < GIC_INTS_MAX {
            return Some(self.vgicd_interrupt(int_id - GIC_PRIVINT_NUM));
        }
        return None;
    }

    fn remove_lr(&self, vcpu_arc: Arc<Mutex<Vcpu>>, interrupt: Arc<Mutex<VgicInt>>) -> bool {
        if !vgic_owns(vcpu_arc.clone(), interrupt.clone()) {
            return false;
        }
        let vgic_int = interrupt.lock();
        let int_lr = vgic_int.lr;
        let int_id = vgic_int.id as usize;
        let vcpu = vcpu_arc.lock();
        let vcpu_id = vcpu.id;
        drop(vcpu);

        if !vgic_int.in_lr {
            return false;
        }
        drop(vgic_int);

        let mut lr_val = 0;
        if let Some(lr) = gich_get_lr(interrupt.clone()) {
            GICH.set_lr(int_lr as usize, 0);
            lr_val = lr;
        }

        let mut vgic_int = interrupt.lock();
        vgic_int.in_lr = false;

        let lr_state = lr_val >> 28 & 0b11;
        if lr_state != 0 {
            vgic_int.state = IrqState::num_to_state(lr_state as usize);
            if int_id < GIC_SGIS_NUM {
                let mut cpu_priv = self.cpu_priv.lock();
                if lr_state & 2 != 0 {
                    cpu_priv[vcpu_id].sgis[int_id].act = ((lr_val >> 10) & 0b111) as u8;
                } else if lr_state & 1 != 0 {
                    cpu_priv[vcpu_id].sgis[int_id].pend = 1 << ((lr_val >> 10) & 0b111) as u8;
                }
            }

            if (lr_state & 1 != 0) && vgic_int.enabled {
                let hcr = GICH.hcr();
                GICH.set_hcr(hcr | (1 << 3));
            }
            return true;
        }
        false
    }

    fn add_lr(&self, vcpu_arc: Arc<Mutex<Vcpu>>, interrupt: Arc<Mutex<VgicInt>>) -> bool {
        let vgic_int = interrupt.lock();

        if (!vgic_int.enabled || vgic_int.in_lr) {
            return false;
        }
        drop(vgic_int);

        let gic_lrs_num = GIC_LRS_NUM.lock();
        let mut lr_ind = None;
        for i in 0..*gic_lrs_num {
            if (GICH.elsr(i / 32) & (1 << i % 32)) != 0 {
                lr_ind = Some(i);
            }
        }

        if lr_ind.is_none() {
            let mut pend_found = 0;
            let mut act_found = 0;
            let mut min_prio_act = 0;
            let mut min_prio_pend = 0;
            let mut act_ind = None;
            let mut pend_ind = None;

            for i in 0..*gic_lrs_num {
                let lr = GICH.lr(i);
                let lr_prio = (lr >> 23) & 0b11111;
                let lr_state = (lr >> 28) & 0b11;

                if lr_state & 2 != 0 {
                    if lr_prio > min_prio_act {
                        min_prio_act = lr_prio;
                        act_ind = Some(i);
                    }
                    act_found += 1;
                } else if lr_state & 1 != 0 {
                    if lr_prio > min_prio_pend {
                        min_prio_pend = lr_prio;
                        pend_ind = Some(i);
                    }
                    pend_found += 1;
                }
            }

            if pend_found > 1 {
                lr_ind = pend_ind;
            } else {
                lr_ind = act_ind;
            }

            if let Some(idx) = lr_ind {
                let spilled_int =
                    self.get_int(vcpu_arc.clone(), GICH.lr(idx) as usize & 0b1111111111);
                self.remove_lr(vcpu_arc.clone(), interrupt.clone());
                vgic_int_yield_owner(vcpu_arc.clone(), interrupt.clone());
            }
        }

        if let Some(idx) = lr_ind {
            // TODO: vgic_write_lr
            self.write_lr(vcpu_arc.clone(), interrupt.clone(), idx);
            return true;
        } else {
            // turn on maintenance interrupts
            if vgic_get_state(interrupt.clone()) & 1 != 0 {
                let hcr = GICH.hcr();
                GICH.set_hcr(hcr | (1 << 3));
            }
        }

        false
    }
    fn write_lr(&self, vcpu_arc: Arc<Mutex<Vcpu>>, interrupt: Arc<Mutex<VgicInt>>, lr_ind: usize) {
        let vcpu = vcpu_arc.lock();
        let cpu_priv = self.cpu_priv.lock();
        let vgic_int = interrupt.lock();

        let vcpu_id = vcpu.id;
        drop(vcpu);
        let int_id = vgic_int.id as usize;
        let int_prio = vgic_int.prio;
        drop(vgic_int);

        let prev_int_id = cpu_priv[vcpu_id].curr_lrs[lr_ind] as usize;
        drop(cpu_priv);
        if prev_int_id != int_id {
            let prev_interrupt_option = self.get_int(vcpu_arc.clone(), prev_int_id);
            if let Some(prev_interrupt) = prev_interrupt_option {
                if vgic_owns(vcpu_arc.clone(), prev_interrupt.clone()) {
                    let mut prev_vgic_int = prev_interrupt.lock();
                    if prev_vgic_int.lr == lr_ind as u16 && prev_vgic_int.in_lr {
                        prev_vgic_int.in_lr = false;
                        let prev_id = prev_vgic_int.id as usize;
                        drop(prev_vgic_int);
                        if !gic_is_priv(prev_id) {
                            vgic_int_yield_owner(vcpu_arc.clone(), prev_interrupt.clone());
                        }
                    }
                }
            }
        }

        let state = vgic_get_state(interrupt.clone());
        let mut lr = (int_id & 0b1111111111) | (((int_prio as usize >> 3) & 0b11111) << 23);

        if vgic_int_is_hw(interrupt.clone()) {
            // let vgic_int = interrupt.lock();
            lr |= (1 << 31);
            lr |= ((0b1111111111 & int_id) << 10);
            if state == 3 {
                lr |= ((2 & 0b11) << 28);
            } else {
                lr |= (state & 0b11) << 28;
            }
            if GICD.state(int_id) != 2 {
                GICD.set_state(int_id, 2);
            }
        } else if (int_id < GIC_SGIS_NUM) {
            if (state & 2) != 0 {
                let cpu_priv = self.cpu_priv.lock();
                // lr |=
            }
        }
    }

    fn route(&self, vcpu_arc: Arc<Mutex<Vcpu>>, interrupt: Arc<Mutex<VgicInt>>) {
        let vgic_int = interrupt.lock();
        if let IrqState::IrqSInactive = vgic_int.state {
            return;
        }

        if !vgic_int.enabled {
            return;
        }

        let int_targets = vgic_int.targets;
        let int_in_lr = vgic_int.in_lr;
        let int_id = vgic_int.id;
        drop(vgic_int);

        if (int_targets & (1 << cpu_id())) != 0 {
            self.add_lr(vcpu_arc.clone(), interrupt.clone());
            // TODO: vgic_add_lr
        }
    }
}

fn vgic_owns(vcpu_arc: Arc<Mutex<Vcpu>>, interrupt: Arc<Mutex<VgicInt>>) -> bool {
    let vgic_int = interrupt.lock();
    let owner_vcpu_id = vgic_int.owner_id();
    let owner_vm_id = vgic_int.owner_vm_id();

    let vcpu = vcpu_arc.lock();
    let vcpu_id = vcpu.id;
    let vcpu_vm_id = vcpu.vm_id();
    drop(vcpu);

    return (owner_vcpu_id == vcpu_id && owner_vm_id == vcpu_vm_id)
        || gic_is_priv(vgic_int.id as usize);
}

fn vgic_get_state(interrupt: Arc<Mutex<VgicInt>>) -> usize {
    let vgic_int = interrupt.lock();
    let in_lr = vgic_int.in_lr;
    let owner_phys_id = vgic_int.owner_phys_id();

    let mut state = vgic_int.state.to_num();
    drop(vgic_int);

    if in_lr && owner_phys_id == cpu_id() {
        let lr_option = gich_get_lr(interrupt.clone());
        if let Some(lr_val) = lr_option {
            state = lr_val as usize;
        }
    }

    let vgic_int = interrupt.lock();
    if vgic_int.id as usize >= GIC_SGIS_NUM {
        return state;
    }
    if vgic_int.owner.is_none() {
        return state;
    }

    let vm = vgic_int.owner_vm();
    let vgic = vm.vgic();
    let vcpu_id = vgic_int.owner_id();

    let cpu_priv = vgic.cpu_priv.lock();
    if cpu_priv[vcpu_id].sgis[vgic_int.id as usize].pend != 0 {
        state |= 1;
    }

    state
}

fn vgic_int_yield_owner(vcpu_arc: Arc<Mutex<Vcpu>>, interrupt: Arc<Mutex<VgicInt>>) {
    if !vgic_owns(vcpu_arc.clone(), interrupt.clone()) {
        return;
    }
    let vgic_int = interrupt.lock();
    if gic_is_priv(vgic_int.id as usize) || vgic_int.in_lr {
        return;
    }
    drop(vgic_int);

    if (vgic_get_state(interrupt.clone()) & 2 == 0) {
        let mut vgic_int = interrupt.lock();
        vgic_int.owner = None;
    }
}

fn vgic_int_is_hw(interrupt: Arc<Mutex<VgicInt>>) -> bool {
    let vgic_int = interrupt.lock();
    vgic_int.id as usize >= GIC_SGIS_NUM && vgic_int.hw
}

fn gich_get_lr(interrupt: Arc<Mutex<VgicInt>>) -> Option<u32> {
    let vgic_int = interrupt.lock();
    let cpu_id = cpu_id();
    let phys_id = vgic_int.owner_phys_id();

    if !vgic_int.in_lr || phys_id != cpu_id {
        println!("DEBUG: gich_get_lr illegal");
        return None;
    }

    let lr_val = GICH.lr(vgic_int.lr as usize);
    if (lr_val & 0b1111111111 == vgic_int.id as u32) && (lr_val >> 28 & 0b11 != 0) {
        return Some(lr_val);
    }
    return None;
}

// fn vgic_get_int(
//     vgic: Arc<Vgic>,
//     vcpu: Arc<Mutex<Vcpu>>,
//     int_id: usize,
// ) -> Option<Arc<Mutex<VgicInt>>> {
//     if int_id < GIC_PRIVINT_NUM {
//         let vcpu_lock = vcpu.lock();
//         let vcpu_id = vcpu_lock.id;
//         return Some(vgic.cpu_priv_interrupt(vcpu_id, int_id));
//     } else if int_id >= GIC_PRIVINT_NUM && int_id < GIC_INTS_MAX {
//         return Some(vgic.vgicd_interrupt(int_id - GIC_PRIVINT_NUM));
//     }
//     return None;
// }

fn vgic_int_get_owner(vcpu_arc: Arc<Mutex<Vcpu>>, interrupt: Arc<Mutex<VgicInt>>) -> bool {
    let mut vgic_int = interrupt.lock();

    if vgic_int.owner.is_none() {
        vgic_int.owner = Some(vcpu_arc.clone());
        return true;
    }

    let owner = vgic_int.owner.as_ref().unwrap().lock();
    let owner_vcpu_id = vgic_int.owner_id();
    let owner_vm_id = vgic_int.owner_vm_id();

    let vcpu = vcpu_arc.lock();
    let vcpu_id = vcpu.id;
    let vcpu_vm_id = vcpu.vm_id();
    drop(vcpu);
    if owner_vm_id == vcpu_vm_id && owner_vcpu_id == vcpu_id {
        return true;
    }

    return false;
}

fn vgicd_set_enable(vgic: Arc<Vgic>, vcpu: Arc<Mutex<Vcpu>>, int_id: usize, en: bool) {
    if int_id < GIC_SGIS_NUM {
        return;
    }

    match vgic.get_int(vcpu.clone(), int_id) {
        Some(interrupt_arc) => {
            if vgic_int_get_owner(vcpu.clone(), interrupt_arc.clone()) {
                let mut interrupt = interrupt_arc.lock();
                let interrupt_id = interrupt.id;
                let interrupt_hw = interrupt.hw;
                if interrupt.enabled ^ en {
                    interrupt.enabled = en;
                    if !interrupt.enabled {
                        drop(interrupt);
                        vgic.remove_lr(vcpu.clone(), interrupt_arc.clone());
                    } else {
                        // TODO: vgic_route
                    }

                    if interrupt_hw {
                        // TODO: gicd_set_enable(interrupt_id, en);
                    }
                }
            }
        }
        None => {
            println!("vgicd_set_enable: interrupt {} is illegal", int_id);
            return;
        }
    }
}

pub fn gic_maintenance_handler(arg: usize, source: usize) {
    unimplemented!();
}

use crate::device::EmuContext;
pub fn emu_intc_handler(emu_dev_id: usize, emu_ctx: &EmuContext) -> bool {
    unimplemented!();
    true
}

fn vgic_ipi_handler(msg: &IpiMessage) {
    println!("DEBUG: vgic ipi handler");
    let mut vm_id = 0;
    let mut int_id = 0;
    let mut val = 0;
    match &msg.ipi_message {
        IpiInnerMsg::Initc(intc) => {
            vm_id = intc.vm_id;
            int_id = intc.int_id;
            val = intc.val;
        }
        _ => {
            println!("vgic_ipi_handler: illegal ipi");
            return;
        }
    }

    let vm = crate::kernel::active_vm();
    let vgic = vm.vgic();

    if vm_id as usize != vm.vm_id() {
        println!(
            "VM {} received vgic msg from another vm {}",
            vm.vm_id(),
            vm_id
        );
        return;
    }

    use crate::kernel::InitcEvent;
    if let IpiInnerMsg::Initc(intc) = &msg.ipi_message {
        match intc.event {
            InitcEvent::VgicdGichEn => {
                let hcr = GICH.hcr();
                if val != 0 {
                    GICH.set_hcr(hcr | 0b1);
                } else {
                    GICH.set_hcr(hcr & !0b1);
                }
            }
            InitcEvent::VgicdSetEn => {
                vgicd_set_enable(vgic.clone(), active_vcpu(), int_id as usize, val != 0);
            }
            // TODO: initc event
            _ => {
                println!("vgic_ipi_handler: core {} received unknown event", cpu_id())
            }
        }
    }
}

use crate::device::EmuDevs;
pub fn emu_intc_init(vm: Vm, emu_dev_id: usize) {
    let vgic_cpu_num = vm.config().cpu.num;
    let vgic = Arc::new(Vgic::default());

    let mut vgicd = vgic.vgicd.lock();
    vgicd.typer = (GICD.typer() & GICD_TYPER_CPUNUM_MSK as u32)
        | (((vm.cpu_num() - 1) << GICD_TYPER_CPUNUM_OFF) & GICD_TYPER_CPUNUM_MSK) as u32;
    vgicd.iidr = (GICD.iidr());

    for i in 0..GIC_SPI_MAX {
        vgicd.interrupts.push(Arc::new(Mutex::new(VgicInt::new(i))));
    }
    drop(vgicd);

    for i in 0..vgic_cpu_num {
        let mut cpu_priv = VgicCpuPriv::default();
        for int_idx in 0..GIC_PRIVINT_NUM {
            let vcpu_arc = vm.vcpu(i);
            let vcpu = vcpu_arc.lock();
            let phys_id = vcpu.phys_id;
            drop(vcpu);

            cpu_priv
                .interrupts
                .push(Arc::new(Mutex::new(VgicInt::priv_new(
                    int_idx,
                    vcpu_arc.clone(),
                    phys_id,
                    int_idx < GIC_SGIS_NUM,
                ))));
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
