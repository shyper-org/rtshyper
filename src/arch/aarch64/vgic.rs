use super::gic::*;
use crate::board::PLATFORM_GICD_BASE;
use crate::device::EmuContext;
use crate::kernel::InitcEvent;
use crate::kernel::Vcpu;
use crate::kernel::Vm;
use crate::kernel::{
    active_vcpu, active_vm, active_vm_id, active_vm_ncpu, context_get_gpr, context_set_gpr, cpu_id,
};
use crate::kernel::{
    ipi_intra_broadcast_msg, ipi_register, ipi_send_msg, IpiInnerMsg, IpiMessage, IpiType,
};
use crate::lib::{bit_extract, bit_get, bit_set};
use crate::{arch::GICH, kernel::IpiInitcMessage};
use crate::{board::platform_cpuid_to_cpuif, kernel::active_vcpu_id};
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

#[derive(Clone)]
struct VgicInt {
    inner: Arc<Mutex<VgicIntInner>>,
}

impl VgicInt {
    fn new(id: usize) -> VgicInt {
        VgicInt {
            inner: Arc::new(Mutex::new(VgicIntInner::new(id))),
        }
    }

    fn priv_new(id: usize, owner: Vcpu, targets: usize, enabled: bool) -> VgicInt {
        VgicInt {
            inner: Arc::new(Mutex::new(VgicIntInner::priv_new(
                id, owner, targets, enabled,
            ))),
        }
    }

    fn set_enabled(&self, enabled: bool) {
        let mut vgic_int = self.inner.lock();
        vgic_int.enabled = enabled;
    }

    fn set_lr(&self, lr: u16) {
        let mut vgic_int = self.inner.lock();
        vgic_int.lr = lr;
    }

    fn set_targets(&self, targets: u8) {
        let mut vgic_int = self.inner.lock();
        vgic_int.targets = targets;
    }

    fn set_prio(&self, prio: u8) {
        let mut vgic_int = self.inner.lock();
        vgic_int.prio = prio;
    }

    fn set_in_lr(&self, in_lr: bool) {
        let mut vgic_int = self.inner.lock();
        vgic_int.in_lr = in_lr;
    }

    fn set_state(&self, state: IrqState) {
        let mut vgic_int = self.inner.lock();
        vgic_int.state = state;
    }

    fn set_owner(&self, owner: Option<Vcpu>) {
        let mut vgic_int = self.inner.lock();
        vgic_int.owner = owner;
    }

    fn set_hw(&self, hw: bool) {
        let mut vgic_int = self.inner.lock();
        vgic_int.hw = hw;
    }

    fn set_cfg(&self, cfg: u8) {
        let mut vgic_int = self.inner.lock();
        vgic_int.cfg = cfg;
    }

    fn lr(&self) -> u16 {
        let vgic_int = self.inner.lock();
        vgic_int.lr
    }

    fn in_lr(&self) -> bool {
        let vgic_int = self.inner.lock();
        vgic_int.in_lr
    }

    fn id(&self) -> u16 {
        let vgic_int = self.inner.lock();
        vgic_int.id
    }

    fn enabled(&self) -> bool {
        let vgic_int = self.inner.lock();
        vgic_int.enabled
    }

    fn prio(&self) -> u8 {
        let vgic_int = self.inner.lock();
        vgic_int.prio
    }

    fn targets(&self) -> u8 {
        let vgic_int = self.inner.lock();
        vgic_int.targets
    }

    fn hw(&self) -> bool {
        let vgic_int = self.inner.lock();
        vgic_int.hw
    }

    fn state(&self) -> IrqState {
        let vgic_int = self.inner.lock();
        vgic_int.state
    }

    fn cfg(&self) -> u8 {
        let vgic_int = self.inner.lock();
        vgic_int.cfg
    }

    fn owner(&self) -> Option<Vcpu> {
        let vgic_int = self.inner.lock();
        vgic_int.owner.clone()
    }

    fn owner_phys_id(&self) -> usize {
        let vgic_int = self.inner.lock();
        vgic_int.owner_phys_id()
    }

    fn owner_id(&self) -> usize {
        let vgic_int = self.inner.lock();
        vgic_int.owner_id()
    }

    fn owner_vm_id(&self) -> usize {
        let vgic_int = self.inner.lock();
        vgic_int.owner_vm_id()
    }

    fn owner_vm(&self) -> Vm {
        let vgic_int = self.inner.lock();
        vgic_int.owner_vm()
    }
}

struct VgicIntInner {
    owner: Option<Vcpu>,
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

impl VgicIntInner {
    fn new(id: usize) -> VgicIntInner {
        VgicIntInner {
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

    fn priv_new(id: usize, owner: Vcpu, targets: usize, enabled: bool) -> VgicIntInner {
        VgicIntInner {
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
        let owner = self.owner.as_ref().unwrap();
        owner.vm().unwrap()
    }

    fn owner_id(&self) -> usize {
        let owner = self.owner.as_ref().unwrap();
        owner.id()
    }

    fn owner_phys_id(&self) -> usize {
        let owner = self.owner.as_ref().unwrap();
        owner.phys_id()
    }

    fn owner_vm_id(&self) -> usize {
        let owner = self.owner.as_ref().unwrap();
        owner.vm_id()
    }
}

struct Vgicd {
    ctlr: u32,
    typer: u32,
    iidr: u32,
    interrupts: Vec<VgicInt>,
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
    interrupts: Vec<VgicInt>,
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

    fn set_vgicd_ctlr(&self, ctlr: u32) {
        let mut vgicd = self.vgicd.lock();
        vgicd.ctlr = ctlr;
    }

    fn vgicd_ctlr(&self) -> u32 {
        let mut vgicd = self.vgicd.lock();
        vgicd.ctlr
    }
    fn vgicd_typer(&self) -> u32 {
        let mut vgicd = self.vgicd.lock();
        vgicd.typer
    }

    fn vgicd_iidr(&self) -> u32 {
        let mut vgicd = self.vgicd.lock();
        vgicd.iidr
    }

    fn cpu_priv_interrupt(&self, cpu_id: usize, idx: usize) -> VgicInt {
        let cpu_priv = self.cpu_priv.lock();
        cpu_priv[cpu_id].interrupts[idx].clone()
    }

    fn vgicd_interrupt(&self, idx: usize) -> VgicInt {
        let vgicd = self.vgicd.lock();
        vgicd.interrupts[idx].clone()
    }

    fn get_int(&self, vcpu: Vcpu, int_id: usize) -> Option<VgicInt> {
        if int_id < GIC_PRIVINT_NUM {
            let vcpu_id = vcpu.id();
            return Some(self.cpu_priv_interrupt(vcpu_id, int_id));
        } else if int_id >= GIC_PRIVINT_NUM && int_id < GIC_INTS_MAX {
            return Some(self.vgicd_interrupt(int_id - GIC_PRIVINT_NUM));
        }
        return None;
    }

    fn remove_lr(&self, vcpu: Vcpu, interrupt: VgicInt) -> bool {
        if !vgic_owns(vcpu.clone(), interrupt.clone()) {
            return false;
        }
        let int_lr = interrupt.lr();
        let int_id = interrupt.id() as usize;
        let vcpu_id = vcpu.id();

        if !interrupt.in_lr() {
            return false;
        }

        let mut lr_val = 0;
        if let Some(lr) = gich_get_lr(interrupt.clone()) {
            GICH.set_lr(int_lr as usize, 0);
            lr_val = lr;
        }

        interrupt.set_in_lr(false);

        let lr_state = lr_val >> 28 & 0b11;
        if lr_state != 0 {
            interrupt.set_state(IrqState::num_to_state(lr_state as usize));
            if int_id < GIC_SGIS_NUM {
                let mut cpu_priv = self.cpu_priv.lock();
                if lr_state & 2 != 0 {
                    cpu_priv[vcpu_id].sgis[int_id].act = ((lr_val >> 10) & 0b111) as u8;
                } else if lr_state & 1 != 0 {
                    cpu_priv[vcpu_id].sgis[int_id].pend = 1 << ((lr_val >> 10) & 0b111) as u8;
                }
            }

            if (lr_state & 1 != 0) && interrupt.enabled() {
                let hcr = GICH.hcr();
                GICH.set_hcr(hcr | (1 << 3));
            }
            return true;
        }
        false
    }

    fn add_lr(&self, vcpu: Vcpu, interrupt: VgicInt) -> bool {
        if !interrupt.enabled() || interrupt.in_lr() {
            return false;
        }

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
                let spilled_int = self.get_int(vcpu.clone(), GICH.lr(idx) as usize & 0b1111111111);
                self.remove_lr(vcpu.clone(), interrupt.clone());
                vgic_int_yield_owner(vcpu.clone(), interrupt.clone());
            }
        }

        if let Some(idx) = lr_ind {
            self.write_lr(vcpu.clone(), interrupt.clone(), idx);
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

    fn write_lr(&self, vcpu: Vcpu, interrupt: VgicInt, lr_ind: usize) {
        let cpu_priv = self.cpu_priv.lock();

        let vcpu_id = vcpu.id();
        let int_id = interrupt.id() as usize;
        let int_prio = interrupt.prio();

        let prev_int_id = cpu_priv[vcpu_id].curr_lrs[lr_ind] as usize;
        drop(cpu_priv);
        if prev_int_id != int_id {
            let prev_interrupt_option = self.get_int(vcpu.clone(), prev_int_id);
            if let Some(prev_interrupt) = prev_interrupt_option {
                if vgic_owns(vcpu.clone(), prev_interrupt.clone()) {
                    if prev_interrupt.lr() == lr_ind as u16 && prev_interrupt.in_lr() {
                        prev_interrupt.set_in_lr(false);
                        let prev_id = prev_interrupt.id() as usize;
                        if !gic_is_priv(prev_id) {
                            vgic_int_yield_owner(vcpu.clone(), prev_interrupt.clone());
                        }
                    }
                }
            }
        }

        let state = vgic_get_state(interrupt.clone());
        let mut lr = (int_id & 0b1111111111) | (((int_prio as usize >> 3) & 0b11111) << 23);

        if vgic_int_is_hw(interrupt.clone()) {
            lr |= 1 << 31;
            lr |= (0b1111111111 & int_id) << 10;
            if state == 3 {
                lr |= (2 & 0b11) << 28;
            } else {
                lr |= (state & 0b11) << 28;
            }
            if GICD.state(int_id) != 2 {
                GICD.set_state(int_id, 2);
            }
        } else if int_id < GIC_SGIS_NUM {
            let mut cpu_priv = self.cpu_priv.lock();
            if (state & 2) != 0 {
                lr |= ((cpu_priv[vcpu_id].sgis[int_id].act as usize) << 10) & (0b111 << 10);
                lr |= (2 & 0b11) << 28;
            } else {
                let mut idx = GIC_TARGETS_MAX - 1;
                while idx as isize >= 0 {
                    if (cpu_priv[vcpu_id].sgis[int_id].pend & (1 << idx)) != 0 {
                        lr |= (idx & 0b111) << 10;
                        cpu_priv[vcpu_id].sgis[int_id].pend &= !(1 << idx);

                        lr |= (1 & 0b11) << 28;
                        break;
                    }
                    idx -= 1;
                }
            }

            if cpu_priv[vcpu_id].sgis[int_id].pend != 0 {
                lr |= 1 << 19;
            }
        } else {
            if !gic_is_priv(int_id) && !vgic_int_is_hw(interrupt.clone()) {
                lr |= 1 << 19;
            }

            lr |= (state & 0b11) << 28;
        }

        let mut cpu_priv = self.cpu_priv.lock();
        interrupt.set_state(IrqState::IrqSInactive);
        interrupt.set_in_lr(true);
        interrupt.set_lr(lr_ind as u16);
        cpu_priv[vcpu_id].curr_lrs[lr_ind] = int_id as u16;
        GICH.set_lr(lr_ind, lr as u32);
    }

    fn route(&self, vcpu: Vcpu, interrupt: VgicInt) {
        let cpu_id = cpu_id();
        if let IrqState::IrqSInactive = interrupt.state() {
            return;
        }

        if !interrupt.enabled() {
            return;
        }

        let int_targets = interrupt.targets();
        if (int_targets & (1 << cpu_id)) != 0 {
            self.add_lr(vcpu.clone(), interrupt.clone());
        }

        if !interrupt.in_lr() && (int_targets & !(1 << cpu_id)) != 0 {
            let vcpu_vm_id = vcpu.vm_id();

            let ipi_msg = IpiInitcMessage {
                event: InitcEvent::VgicdRoute,
                vm_id: vcpu_vm_id,
                int_id: interrupt.id(),
                val: 0,
            };
            vgic_int_yield_owner(vcpu.clone(), interrupt.clone());
            ipi_intra_broadcast_msg(
                active_vm().unwrap(),
                IpiType::IpiTIntc,
                IpiInnerMsg::Initc(ipi_msg),
            );
        }
    }

    fn set_enable(&self, vcpu: Vcpu, int_id: usize, en: bool) {
        println!("DEBUG in vgic::set_enable");
        if int_id < GIC_SGIS_NUM {
            return;
        }
        match self.get_int(active_vcpu().unwrap(), int_id) {
            Some(interrupt) => {
                if vgic_int_get_owner(vcpu.clone(), interrupt.clone()) {
                    if interrupt.enabled() ^ en {
                        interrupt.set_enabled(en);
                        if !interrupt.enabled() {
                            self.remove_lr(vcpu.clone(), interrupt.clone());
                        } else {
                            self.route(vcpu.clone(), interrupt.clone());
                        }
                        if interrupt.hw() {
                            GICD.set_enable(interrupt.id() as usize, en);
                        }
                    }
                    vgic_int_yield_owner(vcpu.clone(), interrupt.clone());
                } else {
                    let int_phys_id = interrupt.owner_phys_id();
                    let vcpu_vm_id = vcpu.vm_id();
                    let ipi_msg = IpiInitcMessage {
                        event: InitcEvent::VgicdSetEn,
                        vm_id: vcpu_vm_id,
                        int_id: interrupt.id(),
                        val: en as u8,
                    };
                    if !ipi_send_msg(int_phys_id, IpiType::IpiTIntc, IpiInnerMsg::Initc(ipi_msg)) {
                        println!(
                            "vgicd_set_enable: Failed to send ipi message, target {} type {}",
                            int_phys_id, 0
                        );
                    }
                }
            }
            None => {
                println!("vgicd_set_enable: interrupt {} is illegal", int_id);
                return;
            }
        }
    }

    fn get_enable(&self, vcpu: Vcpu, int_id: usize) -> bool {
        self.get_int(vcpu.clone(), int_id).unwrap().enabled()
    }

    fn set_pend(&self, vcpu: Vcpu, int_id: usize, pend: bool) {
        // TODO: sgi_get_pend ?
        if bit_extract(int_id, 0, 10) < GIC_SGIS_NUM {
            self.sgi_set_pend(vcpu.clone(), int_id, pend);
            return;
        }

        let interrupt_option = self.get_int(active_vcpu().unwrap(), bit_extract(int_id, 0, 10));

        if let Some(interrupt) = interrupt_option {
            if vgic_int_get_owner(vcpu.clone(), interrupt.clone()) {
                self.remove_lr(vcpu.clone(), interrupt.clone());

                let state = interrupt.state().to_num();
                if pend && ((state & 1) == 0) {
                    interrupt.set_state(IrqState::num_to_state(state | 1));
                } else if !pend && (state & 1) != 0 {
                    interrupt.set_state(IrqState::num_to_state(state & !1));
                }

                let state = interrupt.state().to_num();
                if interrupt.hw() {
                    let vgic_int_id = interrupt.id() as usize;
                    GICD.set_state(vgic_int_id, if state == 1 { 2 } else { state })
                }
                self.route(vcpu.clone(), interrupt.clone());
                vgic_int_yield_owner(vcpu.clone(), interrupt.clone());
            } else {
                let vm_id = vcpu.vm_id();

                let m = IpiInitcMessage {
                    event: InitcEvent::VgicdSetPend,
                    vm_id,
                    int_id: interrupt.id(),
                    val: pend as u8,
                };
                let phys_id = interrupt.owner_phys_id();
                if !ipi_send_msg(phys_id, IpiType::IpiTIntc, IpiInnerMsg::Initc(m)) {
                    println!(
                        "vgicd_set_pend: Failed to send ipi message, target {} type {}",
                        phys_id, 0
                    );
                }
            }
        }
    }

    fn set_active(&self, vcpu: Vcpu, int_id: usize, act: bool) {
        let interrupt_option = self.get_int(active_vcpu().unwrap(), bit_extract(int_id, 0, 10));
        if let Some(interrupt) = interrupt_option {
            if vgic_int_get_owner(vcpu.clone(), interrupt.clone()) {
                self.remove_lr(vcpu.clone(), interrupt.clone());
                let state = interrupt.state().to_num();
                if act && ((state & 2) == 0) {
                    interrupt.set_state(IrqState::num_to_state(state | 2));
                } else if !act && (state & 2) != 0 {
                    interrupt.set_state(IrqState::num_to_state(state & !2));
                }

                let state = interrupt.state().to_num();
                if interrupt.hw() {
                    let vgic_int_id = interrupt.id() as usize;
                    GICD.set_state(vgic_int_id, if state == 1 { 2 } else { state })
                }
                self.route(vcpu.clone(), interrupt.clone());
                vgic_int_yield_owner(vcpu.clone(), interrupt.clone());
            } else {
                let vm_id = vcpu.vm_id();

                let m = IpiInitcMessage {
                    event: InitcEvent::VgicdSetPend,
                    vm_id,
                    int_id: interrupt.id(),
                    val: act as u8,
                };
                let phys_id = interrupt.owner_phys_id();
                if !ipi_send_msg(phys_id, IpiType::IpiTIntc, IpiInnerMsg::Initc(m)) {
                    println!(
                        "vgicd_set_active: Failed to send ipi message, target {} type {}",
                        phys_id, 0
                    );
                }
            }
        }
    }

    fn set_icfgr(&self, vcpu: Vcpu, int_id: usize, cfg: u8) {
        let interrupt_option = self.get_int(vcpu.clone(), int_id);
        if let Some(interrupt) = interrupt_option {
            if vgic_int_get_owner(vcpu.clone(), interrupt.clone()) {
                interrupt.set_cfg(cfg);
                if interrupt.hw() {
                    GICD.set_icfgr(interrupt.id() as usize, cfg);
                }
                vgic_int_yield_owner(vcpu.clone(), interrupt.clone());
            } else {
                let m = IpiInitcMessage {
                    event: InitcEvent::VgicdSetCfg,
                    vm_id: vcpu.vm_id(),
                    int_id: interrupt.id(),
                    val: cfg,
                };
                if !ipi_send_msg(
                    interrupt.owner_phys_id(),
                    IpiType::IpiTIntc,
                    IpiInnerMsg::Initc(m),
                ) {
                    println!(
                        "set_icfgr: Failed to send ipi message, target {} type {}",
                        interrupt.owner_phys_id(),
                        0
                    );
                }
            }
        } else {
            unimplemented!();
        }
    }

    fn get_icfgr(&self, vcpu: Vcpu, int_id: usize) -> u8 {
        let interrupt_option = self.get_int(vcpu.clone(), int_id);
        if let Some(interrupt) = interrupt_option {
            return interrupt.cfg();
        } else {
            unimplemented!();
        }
    }

    fn sgi_set_pend(&self, vcpu: Vcpu, int_id: usize, pend: bool) {
        if bit_extract(int_id, 0, 10) > GIC_SGIS_NUM {
            return;
        }

        let interrupt_option = self.get_int(active_vcpu().unwrap(), bit_extract(int_id, 0, 10));
        let source = bit_extract(int_id, 10, 5);

        if let Some(interrupt) = interrupt_option {
            self.remove_lr(vcpu.clone(), interrupt.clone());
            let vcpu_id = vcpu.id();

            let vgic_int_id = interrupt.id() as usize;
            let mut cpu_priv = self.cpu_priv.lock();
            let pendstate = cpu_priv[vcpu_id].sgis[vgic_int_id].pend;
            let new_pendstate = if pend {
                pendstate | (1 << source) as u8
            } else {
                pendstate & !(1 << source) as u8
            };

            if (pendstate ^ new_pendstate) != 0 {
                cpu_priv[vcpu_id].sgis[vgic_int_id].pend = new_pendstate;
                let state = interrupt.state().to_num();
                if new_pendstate != 0 {
                    interrupt.set_state(IrqState::num_to_state(state | 1));
                } else {
                    interrupt.set_state(IrqState::num_to_state(state & !1));
                }
                match interrupt.state() {
                    IrqState::IrqSInactive => {
                        self.add_lr(vcpu.clone(), interrupt.clone());
                    }
                    _ => {}
                }
            }
        } else {
            println!(
                "sgi_set_pend: interrupt {} is None",
                bit_extract(int_id, 0, 10)
            );
        }
    }

    fn set_prio(&self, vcpu: Vcpu, int_id: usize, mut prio: u8) {
        let interrupt_option = self.get_int(active_vcpu().unwrap(), int_id);
        prio &= 0xf0; // gic-400 only allows 4 priority bits in non-secure state

        if let Some(interrupt) = interrupt_option {
            if vgic_int_get_owner(vcpu.clone(), interrupt.clone()) {
                if interrupt.prio() != prio {
                    self.remove_lr(vcpu.clone(), interrupt.clone());
                    let prev_prio = interrupt.prio();
                    interrupt.set_prio(prio);
                    if prio <= prev_prio {
                        self.route(vcpu.clone(), interrupt.clone());
                    }
                    if interrupt.hw() {
                        GICD.set_prio(interrupt.id() as usize, prio);
                    }
                }
                vgic_int_yield_owner(vcpu.clone(), interrupt.clone());
            } else {
                let vm_id = vcpu.vm_id();

                let m = IpiInitcMessage {
                    event: InitcEvent::VgicdSetPrio,
                    vm_id,
                    int_id: interrupt.id(),
                    val: prio,
                };
                if !ipi_send_msg(
                    interrupt.owner_phys_id(),
                    IpiType::IpiTIntc,
                    IpiInnerMsg::Initc(m),
                ) {
                    println!(
                        "set_prio: Failed to send ipi message, target {} type {}",
                        interrupt.owner_phys_id(),
                        0
                    );
                }
            }
        }
    }

    fn get_prio(&self, vcpu: Vcpu, int_id: usize) -> u8 {
        let interrupt_option = self.get_int(vcpu.clone(), int_id);
        return interrupt_option.unwrap().prio();
    }

    fn set_trgt(&self, vcpu: Vcpu, int_id: usize, trgt: u8) {
        // println!("DEBUG: set trgt");
        let interrupt_option = self.get_int(active_vcpu().unwrap(), int_id);
        if let Some(interrupt) = interrupt_option {
            if vgic_int_get_owner(vcpu.clone(), interrupt.clone()) {
                if interrupt.targets() != trgt {
                    interrupt.set_targets(trgt);
                    let mut ptrgt = 0;
                    for cpuid in 0..8 {
                        if bit_get(trgt as usize, cpuid) != 0 {
                            ptrgt = bit_set(ptrgt, platform_cpuid_to_cpuif(cpuid))
                        }
                    }
                    if interrupt.hw() {
                        GICD.set_trgt(interrupt.id() as usize, ptrgt as u8);
                    }
                    if vgic_get_state(interrupt.clone()) != 0 {
                        self.route(vcpu.clone(), interrupt.clone());
                    }
                }
                vgic_int_yield_owner(vcpu.clone(), interrupt.clone());
            } else {
                let vm_id = vcpu.vm_id();
                let m = IpiInitcMessage {
                    event: InitcEvent::VgicdSetTrgt,
                    vm_id,
                    int_id: interrupt.id(),
                    val: trgt,
                };
                if !ipi_send_msg(
                    interrupt.owner_phys_id(),
                    IpiType::IpiTIntc,
                    IpiInnerMsg::Initc(m),
                ) {
                    println!(
                        "set_trgt: Failed to send ipi message, target {} type {}",
                        interrupt.owner_phys_id(),
                        0
                    );
                }
            }
        }
    }

    fn get_trgt(&self, vcpu: Vcpu, int_id: usize) -> u8 {
        let interrupt_option = self.get_int(vcpu.clone(), int_id);
        return interrupt_option.unwrap().targets();
    }

    pub fn inject(&self, id: usize, src: usize) {
        let vcpu = active_vcpu().unwrap();
        let interrupt_option = self.get_int(vcpu.clone(), bit_extract(id, 0, 10));
        if let Some(interrupt) = interrupt_option {
            if interrupt.hw() {
                interrupt.set_owner(Some(vcpu.clone()));
                interrupt.set_state(IrqState::IrqSPend);
                interrupt.set_in_lr(false);
                self.route(vcpu.clone(), interrupt);
            } else {
                self.set_pend(vcpu.clone(), id, true);
            }
        }
    }

    fn emu_ctrl_access(&self, emu_ctx: &EmuContext) {
        if emu_ctx.write {
            let prev_ctlr = self.vgicd_ctlr();
            self.set_vgicd_ctlr(context_get_gpr(emu_ctx.reg) as u32 & 0x1);
            if prev_ctlr ^ self.vgicd_ctlr() != 0 {
                let enable = self.vgicd_ctlr() != 0;
                let hcr = GICH.hcr();
                if enable {
                    GICH.set_hcr(hcr | 1);
                } else {
                    GICH.set_hcr(hcr & !1);
                }

                let m = IpiInitcMessage {
                    event: InitcEvent::VgicdGichEn,
                    vm_id: active_vm_id(),
                    int_id: 0,
                    val: enable as u8,
                };
                ipi_intra_broadcast_msg(
                    active_vm().unwrap(),
                    IpiType::IpiTIntc,
                    IpiInnerMsg::Initc(m),
                );
            }
        } else {
            context_set_gpr(emu_ctx.reg, self.vgicd_ctlr() as usize);
        }
    }

    fn emu_typer_access(&self, emu_ctx: &EmuContext) {
        if !emu_ctx.write {
            context_set_gpr(emu_ctx.reg, self.vgicd_typer() as usize);
        } else {
            println!("emu_typer_access: can't write to RO reg");
        }
    }

    fn emu_iidr_access(&self, emu_ctx: &EmuContext) {
        if !emu_ctx.write {
            context_set_gpr(emu_ctx.reg, self.vgicd_iidr() as usize);
        } else {
            println!("emu_iidr_access: can't write to RO reg");
        }
    }

    fn emu_isenabler_access(&self, emu_ctx: &EmuContext) {
        // println!("DEBUG: in emu_isenabler_access");
        let reg_idx = (emu_ctx.address & 0b1111111) / 4;
        let mut val = if emu_ctx.write {
            context_get_gpr(emu_ctx.reg)
        } else {
            0
        };
        let first_int = reg_idx * 32;
        let vm_id = active_vm_id();
        let vm = match active_vm() {
            Ok(vm) => vm,
            Err(()) => {
                panic!("emu_isenabler_access: current vcpu.vm is none");
            }
        };
        let mut vm_has_interrupt_flag = false;

        for i in 0..32 {
            if vm.has_interrupt(first_int + i) || vm.emu_has_interrupt(first_int + i) {
                vm_has_interrupt_flag = true;
                break;
            }
        }
        if first_int >= 16 && !vm_has_interrupt_flag {
            println!(
                "emu_isenabler_access: vm[{}] does not have interrupt {}",
                vm_id, first_int
            );
            return;
        }

        if emu_ctx.write {
            for i in 0..32 {
                if bit_get(val, i) != 0 {
                    self.set_enable(active_vcpu().unwrap(), first_int + i, true);
                }
            }
        } else {
            for i in 0..32 {
                if self.get_enable(active_vcpu().unwrap(), first_int + i) {
                    val |= 1 << i;
                }
            }
            context_set_gpr(emu_ctx.reg, val);
        }
    }

    fn emu_pendr_access(&self, emu_ctx: &EmuContext, set: bool) {
        let reg_idx = (emu_ctx.address & 0b1111111) / 4;
        let mut val = if emu_ctx.write {
            context_get_gpr(emu_ctx.reg)
        } else {
            0
        };
        let first_int = reg_idx * 32;
        let vm_id = active_vm_id();
        let vm = match active_vm() {
            Ok(vm) => vm,
            Err(()) => {
                panic!("emu_pendr_access: current vcpu.vm is none");
            }
        };
        let mut vm_has_interrupt_flag = false;

        for i in 0..emu_ctx.width {
            if vm.has_interrupt(first_int + i) || vm.emu_has_interrupt(first_int + i) {
                vm_has_interrupt_flag = true;
                break;
            }
        }
        if first_int >= 16 && !vm_has_interrupt_flag {
            println!(
                "emu_pendr_access: vm[{}] does not have interrupt {}",
                vm_id, first_int
            );
            return;
        }

        if emu_ctx.write {
            for i in 0..32 {
                if bit_get(val, i) != 0 {
                    self.set_pend(active_vcpu().unwrap(), first_int + i, set);
                }
            }
        } else {
            for i in 0..32 {
                match self.get_int(active_vcpu().unwrap(), first_int + i) {
                    Some(interrupt) => {
                        if vgic_get_state(interrupt.clone()) & 1 != 0 {
                            val |= 1 << i;
                        }
                    }
                    None => {
                        unimplemented!();
                    }
                }
            }
            context_set_gpr(emu_ctx.reg, val);
        }
    }

    fn emu_ispendr_access(&self, emu_ctx: &EmuContext) {
        self.emu_pendr_access(emu_ctx, true);
    }

    fn emu_activer_access(&self, emu_ctx: &EmuContext, set: bool) {
        // println!("DEBUG: in emu_activer_access");
        let reg_idx = (emu_ctx.address & 0b1111111) / 4;
        let mut val = if emu_ctx.write {
            context_get_gpr(emu_ctx.reg)
        } else {
            0
        };
        let first_int = reg_idx * 32;
        let vm_id = active_vm_id();
        let vm = match active_vm() {
            Ok(vm) => vm,
            Err(()) => {
                panic!("emu_activer_access: current vcpu.vm is none");
            }
        };
        let mut vm_has_interrupt_flag = false;

        for i in 0..32 {
            if vm.has_interrupt(first_int + i) || vm.emu_has_interrupt(first_int + i) {
                vm_has_interrupt_flag = true;
                break;
            }
        }
        if first_int >= 16 && !vm_has_interrupt_flag {
            println!(
                "emu_activer_access: vm[{}] does not have interrupt {}",
                vm_id, first_int
            );
            return;
        }

        if emu_ctx.write {
            for i in 0..32 {
                if bit_get(val, i) != 0 {
                    self.set_active(active_vcpu().unwrap(), first_int + i, set);
                }
            }
        } else {
            for i in 0..32 {
                match self.get_int(active_vcpu().unwrap(), first_int + i) {
                    Some(interrupt) => {
                        if vgic_get_state(interrupt.clone()) & 2 != 0 {
                            val |= 1 << i;
                        }
                    }
                    None => {
                        unimplemented!();
                    }
                }
            }
            context_set_gpr(emu_ctx.reg, val);
        }
    }

    fn emu_isactiver_access(&self, emu_ctx: &EmuContext) {
        self.emu_activer_access(emu_ctx, true);
    }

    fn emu_icenabler_access(&self, emu_ctx: &EmuContext) {
        let reg_idx = (emu_ctx.address & 0b1111111) / 4;
        let mut val = if emu_ctx.write {
            context_get_gpr(emu_ctx.reg)
        } else {
            0
        };
        let first_int = reg_idx * 32;
        let vm_id = active_vm_id();
        let vm = match active_vm() {
            Ok(vm) => vm,
            Err(()) => {
                panic!("emu_activer_access: current vcpu.vm is none");
            }
        };
        let mut vm_has_interrupt_flag = false;

        for i in 0..32 {
            if vm.has_interrupt(first_int + i) || vm.emu_has_interrupt(first_int + i) {
                vm_has_interrupt_flag = true;
                break;
            }
        }
        if first_int >= 16 && !vm_has_interrupt_flag {
            println!(
                "emu_icenabler_access: vm[{}] does not have interrupt {}",
                vm_id, first_int
            );
            return;
        }

        if emu_ctx.write {
            for i in 0..32 {
                if bit_get(val, i) != 0 {
                    self.set_enable(active_vcpu().unwrap(), first_int + i, false);
                }
            }
        } else {
            for i in 0..32 {
                if self.get_enable(active_vcpu().unwrap(), first_int + i) {
                    val |= 1 << i;
                }
            }
            context_set_gpr(emu_ctx.reg, val);
        }
    }

    fn emu_icpendr_access(&self, emu_ctx: &EmuContext) {
        self.emu_pendr_access(emu_ctx, false);
    }

    fn emu_icativer_access(&self, emu_ctx: &EmuContext) {
        self.emu_activer_access(emu_ctx, false);
    }

    fn emu_icfgr_access(&self, emu_ctx: &EmuContext) {
        let first_int = (32 / GIC_CONFIG_BITS) * bit_extract(emu_ctx.address, 0, 9) / 4;
        let vm_id = active_vm_id();
        let vm = match active_vm() {
            Ok(vm) => vm,
            Err(()) => {
                panic!("emu_icfgr_access: current vcpu.vm is none");
            }
        };
        let mut vm_has_interrupt_flag = false;

        for i in 0..emu_ctx.width * 8 {
            if vm.has_interrupt(first_int + i) || vm.emu_has_interrupt(first_int + i) {
                vm_has_interrupt_flag = true;
                break;
            }
        }
        if first_int >= 16 && !vm_has_interrupt_flag {
            println!(
                "emu_icfgr_access: vm[{}] does not have interrupt {}",
                vm_id, first_int
            );
            return;
        }

        if emu_ctx.write {
            let cfg = context_get_gpr(emu_ctx.reg);
            let mut irq = first_int;
            let mut bit = 0;
            while bit < emu_ctx.width * 8 {
                self.set_icfgr(
                    active_vcpu().unwrap(),
                    irq,
                    bit_extract(cfg as usize, bit, 2) as u8,
                );
                bit += 2;
                irq += 1;
            }
        } else {
            let mut cfg = 0;
            let mut irq = first_int;
            let mut bit = 0;
            while bit < emu_ctx.width * 8 {
                cfg |= (self.get_icfgr(active_vcpu().unwrap(), irq) as usize) << bit;
                bit += 2;
                irq += 1;
            }
            context_set_gpr(emu_ctx.reg, cfg);
        }
    }

    fn emu_sgiregs_access(&self, emu_ctx: &EmuContext) {
        let val = if emu_ctx.write {
            context_get_gpr(emu_ctx.reg)
        } else {
            0
        };
        let vm = match active_vm() {
            Ok(vm) => vm,
            Err(()) => {
                panic!("emu_icfgr_access: current vcpu.vm is none");
            }
        };

        if bit_extract(emu_ctx.address, 0, 12) == bit_extract(PLATFORM_GICD_BASE + 0x0f00, 0, 12) {
            if emu_ctx.write {
                let sgir_trglstflt = bit_extract(val, 24, 2);
                let mut trgtlist = 0;
                match sgir_trglstflt {
                    0 => {
                        trgtlist =
                            vgic_target_translate(vm.clone(), bit_extract(val, 16, 8) as u32, true)
                                as usize;
                    }
                    1 => {
                        trgtlist = active_vm_ncpu() & !(1 << cpu_id());
                    }
                    2 => {
                        trgtlist = 1 << cpu_id();
                    }
                    3 => {
                        return;
                    }
                    _ => {}
                }

                for i in 0..8 {
                    let m = IpiInitcMessage {
                        event: InitcEvent::VgicdSetPend,
                        vm_id: active_vm_id(),
                        int_id: (bit_extract(val, 0, 8) | (active_vcpu_id() << 10)) as u16,
                        val: true as u8,
                    };
                    if !ipi_send_msg(i, IpiType::IpiTIntc, IpiInnerMsg::Initc(m)) {
                        println!(
                            "emu_sgiregs_access: Failed to send ipi message, target {} type {}",
                            i, 0
                        );
                    }
                }
            }
        } else {
            // TODO: CPENDSGIR and SPENDSGIR access
        }
    }

    fn emu_ipriorityr_access(&self, emu_ctx: &EmuContext) {
        let mut val = if emu_ctx.write {
            context_get_gpr(emu_ctx.reg)
        } else {
            0
        };
        let first_int = (8 / GIC_PRIO_BITS) * bit_extract(emu_ctx.address, 0, 9);
        let vm_id = active_vm_id();
        let vm = match active_vm() {
            Ok(vm) => vm,
            Err(()) => {
                panic!("emu_ipriorityr_access: current vcpu.vm is none");
            }
        };
        let mut vm_has_interrupt_flag = false;

        for i in 0..emu_ctx.width {
            if vm.has_interrupt(first_int + i) || vm.emu_has_interrupt(first_int + i) {
                vm_has_interrupt_flag = true;
                break;
            }
        }
        if first_int >= 16 && !vm_has_interrupt_flag {
            println!(
                "emu_ipriorityr_access: vm[{}] does not have interrupt {}",
                vm_id, first_int
            );
            return;
        }

        if emu_ctx.write {
            for i in 0..emu_ctx.width {
                self.set_prio(
                    active_vcpu().unwrap(),
                    first_int + i,
                    bit_extract(val, GIC_PRIO_BITS * i, GIC_PRIO_BITS) as u8,
                );
            }
        } else {
            for i in 0..emu_ctx.width {
                val |= (self.get_prio(active_vcpu().unwrap(), first_int + i) as usize)
                    << (GIC_PRIO_BITS * i);
            }
            context_set_gpr(emu_ctx.reg, val);
        }
    }

    fn emu_itargetr_access(&self, emu_ctx: &EmuContext) {
        let mut val = if emu_ctx.write {
            context_get_gpr(emu_ctx.reg)
        } else {
            0
        };
        let first_int = (8 / GIC_TARGET_BITS) * bit_extract(emu_ctx.address, 0, 9);
        let vm_id = active_vm_id();
        let vm = match active_vm() {
            Ok(vm) => vm,
            Err(()) => {
                panic!("emu_itargetr_access: current vcpu.vm is none");
            }
        };
        let mut vm_has_interrupt_flag = false;

        for i in 0..emu_ctx.width {
            if vm.has_interrupt(first_int + i) || vm.emu_has_interrupt(first_int + i) {
                vm_has_interrupt_flag = true;
                break;
            }
        }
        if first_int >= 16 && !vm_has_interrupt_flag {
            println!(
                "emu_itargetr_access: vm[{}] does not have interrupt {}",
                vm_id, first_int
            );
            return;
        }

        if emu_ctx.write {
            // println!("write");
            val = vgic_target_translate(active_vm().unwrap(), val as u32, true) as usize;
            for i in 0..emu_ctx.width {
                self.set_trgt(
                    active_vcpu().unwrap(),
                    first_int + i,
                    bit_extract(val, GIC_TARGET_BITS * i, GIC_TARGET_BITS) as u8,
                );
            }
        } else {
            // println!("read, first_int {}, width {}", first_int, emu_ctx.width);
            for i in 0..emu_ctx.width {
                println!("{}", self.get_trgt(active_vcpu().unwrap(), first_int + i));
                val |= ((self.get_trgt(active_vcpu().unwrap(), first_int + i) as usize)
                    << (GIC_TARGET_BITS * i));
            }
            // println!("after read val {}", val);
            val = vgic_target_translate(active_vm().unwrap(), val as u32, false) as usize;
            context_set_gpr(emu_ctx.reg, val);
        }
    }
}

fn vgic_target_translate(vm: Vm, trgt: u32, v2p: bool) -> u32 {
    // println!("in vgic_target_translate");
    let mut to = [0; 4];
    let mut from = [0; 4];
    for i in 0..4 {
        from[i] = (trgt >> (8 * i)) & 0xff;
        to[i] = if v2p {
            vm.vcpu_to_pcpu_mask(from[i] as usize, 8) as u32
        } else {
            vm.pcpu_to_vcpu_mask(from[i] as usize, 8) as u32
        };
    }

    println!(
        "finish vgic_target_translate: from {:x} to {:x}",
        trgt,
        (to[0] & 0xff) | ((to[1] & 0xff) << 8) | ((to[2] & 0xff) << 16) | ((to[3] & 0xff) << 24)
    );
    return (to[0] & 0xff)
        | ((to[1] & 0xff) << 8)
        | ((to[2] & 0xff) << 16)
        | ((to[3] & 0xff) << 24);
}

fn vgic_owns(vcpu: Vcpu, interrupt: VgicInt) -> bool {
    let owner_vcpu_id = interrupt.owner_id();
    let owner_vm_id = interrupt.owner_vm_id();

    let vcpu_id = vcpu.id();
    let vcpu_vm_id = vcpu.vm_id();

    return (owner_vcpu_id == vcpu_id && owner_vm_id == vcpu_vm_id)
        || gic_is_priv(interrupt.id() as usize);
}

fn vgic_get_state(interrupt: VgicInt) -> usize {
    let mut state = interrupt.state().to_num();

    if interrupt.in_lr() && interrupt.owner_phys_id() == cpu_id() {
        let lr_option = gich_get_lr(interrupt.clone());
        if let Some(lr_val) = lr_option {
            state = lr_val as usize;
        }
    }

    if interrupt.id() as usize >= GIC_SGIS_NUM {
        return state;
    }
    if interrupt.owner().is_none() {
        return state;
    }

    let vm = interrupt.owner_vm();
    let vgic = vm.vgic();
    let vcpu_id = interrupt.owner_id();

    let cpu_priv = vgic.cpu_priv.lock();
    if cpu_priv[vcpu_id].sgis[interrupt.id() as usize].pend != 0 {
        state |= 1;
    }

    state
}

fn vgic_int_yield_owner(vcpu: Vcpu, interrupt: VgicInt) {
    if !vgic_owns(vcpu.clone(), interrupt.clone()) {
        return;
    }
    if gic_is_priv(interrupt.id() as usize) || interrupt.in_lr() {
        return;
    }

    if vgic_get_state(interrupt.clone()) & 2 == 0 {
        interrupt.set_owner(None);
    }
}

fn vgic_int_is_hw(interrupt: VgicInt) -> bool {
    interrupt.id() as usize >= GIC_SGIS_NUM && interrupt.hw()
}

fn gich_get_lr(interrupt: VgicInt) -> Option<u32> {
    let cpu_id = cpu_id();
    let phys_id = interrupt.owner_phys_id();

    if !interrupt.in_lr() || phys_id != cpu_id {
        println!("DEBUG: gich_get_lr illegal");
        return None;
    }

    let lr_val = GICH.lr(interrupt.lr() as usize);
    if (lr_val & 0b1111111111 == interrupt.id() as u32) && (lr_val >> 28 & 0b11 != 0) {
        return Some(lr_val);
    }
    return None;
}

fn vgic_int_get_owner(vcpu: Vcpu, interrupt: VgicInt) -> bool {
    if interrupt.owner().is_none() {
        interrupt.set_owner(Some(vcpu.clone()));
        return true;
    }

    let owner_vcpu_id = interrupt.owner_id();
    let owner_vm_id = interrupt.owner_vm_id();

    let vcpu_id = vcpu.id();
    let vcpu_vm_id = vcpu.vm_id();
    if owner_vm_id == vcpu_vm_id && owner_vcpu_id == vcpu_id {
        return true;
    }

    return false;
}

pub fn gic_maintenance_handler(arg: usize, source: usize) {
    // TODO
    unimplemented!();
}

const VGICD_REG_OFFSET_PREFIX_CTLR: usize = 0x0; // same as TYPER & IIDR
const VGICD_REG_OFFSET_PREFIX_ISENABLER: usize = 0x2;
const VGICD_REG_OFFSET_PREFIX_ICENABLER: usize = 0x3;
const VGICD_REG_OFFSET_PREFIX_ISPENDR: usize = 0x4;
const VGICD_REG_OFFSET_PREFIX_ICPENDR: usize = 0x5;
const VGICD_REG_OFFSET_PREFIX_ISACTIVER: usize = 0x6;
const VGICD_REG_OFFSET_PREFIX_ICACTIVER: usize = 0x7;
const VGICD_REG_OFFSET_PREFIX_ICFGR: usize = 0x18;
const VGICD_REG_OFFSET_PREFIX_SGIR: usize = 0x1e;

pub fn emu_intc_handler(emu_dev_id: usize, emu_ctx: &EmuContext) -> bool {
    let offset = emu_ctx.address & 0xfff;
    if emu_ctx.width > 4 {
        return false;
    }

    let vm = match crate::kernel::active_vm() {
        Err(_) => {
            panic!("emu_intc_handler: vm is None");
        }
        Ok(x) => x,
    };
    let vgic = vm.vgic();
    let vgicd_offset_prefix = (offset & 0xf80) >> 7;
    println!(
        "emu_intc_handler: vgicd_offset_prefix 0x{:x}, offset 0x{:x}",
        vgicd_offset_prefix, offset
    );
    match vgicd_offset_prefix {
        VGICD_REG_OFFSET_PREFIX_CTLR
        | VGICD_REG_OFFSET_PREFIX_ISENABLER
        | VGICD_REG_OFFSET_PREFIX_ISPENDR
        | VGICD_REG_OFFSET_PREFIX_ISACTIVER
        | VGICD_REG_OFFSET_PREFIX_ICENABLER
        | VGICD_REG_OFFSET_PREFIX_ICPENDR
        | VGICD_REG_OFFSET_PREFIX_ICACTIVER
        | VGICD_REG_OFFSET_PREFIX_ICFGR => {
            if emu_ctx.width != 4 || emu_ctx.address & 0x3 != 0 {
                return false;
            }
        }
        VGICD_REG_OFFSET_PREFIX_SGIR => {
            if (emu_ctx.width == 4 && emu_ctx.address & 0x3 != 0)
                || (emu_ctx.width == 2 && emu_ctx.address & 0x1 != 0)
            {
                return false;
            }
        }
        _ => {
            // TODO: hard code to rebuild (gicd IPRIORITYR and ITARGETSR)
            if offset >= 0x400 && offset < 0xc00 {
                if (emu_ctx.width == 4 && emu_ctx.address & 0x3 != 0)
                    || (emu_ctx.width == 2 && emu_ctx.address & 0x1 != 0)
                {
                    return false;
                }
            }
        }
    }

    match vgicd_offset_prefix {
        VGICD_REG_OFFSET_PREFIX_ISENABLER => {
            vgic.emu_isenabler_access(emu_ctx);
        }
        VGICD_REG_OFFSET_PREFIX_ISPENDR => {
            vgic.emu_ispendr_access(emu_ctx);
        }
        VGICD_REG_OFFSET_PREFIX_ISACTIVER => {
            vgic.emu_isactiver_access(emu_ctx);
        }
        VGICD_REG_OFFSET_PREFIX_ICENABLER => {
            vgic.emu_icenabler_access(emu_ctx);
        }
        VGICD_REG_OFFSET_PREFIX_ICPENDR => {
            vgic.emu_icpendr_access(emu_ctx);
        }
        VGICD_REG_OFFSET_PREFIX_ICACTIVER => {
            vgic.emu_icativer_access(emu_ctx);
        }
        VGICD_REG_OFFSET_PREFIX_ICFGR => {
            vgic.emu_icfgr_access(emu_ctx);
        }
        VGICD_REG_OFFSET_PREFIX_SGIR => {
            vgic.emu_sgiregs_access(emu_ctx);
        }
        _ => {
            match offset {
                // VGICD_REG_OFFSET(CTLR)
                0 => {
                    vgic.emu_ctrl_access(emu_ctx);
                }
                // VGICD_REG_OFFSET(TYPER)
                0x004 => {
                    vgic.emu_typer_access(emu_ctx);
                }
                // VGICD_REG_OFFSET(IIDR)
                0x008 => {
                    vgic.emu_iidr_access(emu_ctx);
                }
                _ => {
                    if !emu_ctx.write {
                        context_set_gpr(emu_ctx.reg, 0);
                    }
                }
            }
            if offset >= 0x400 && offset < 0x800 {
                vgic.emu_ipriorityr_access(emu_ctx);
            } else if offset >= 0x800 && offset < 0xc00 {
                vgic.emu_itargetr_access(emu_ctx);
            }
        }
    }
    println!("finish emu_intc_handler");
    true
}

fn vgic_ipi_handler(msg: &IpiMessage) {
    // println!("DEBUG: vgic ipi handler");
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

    let vm = match crate::kernel::active_vm() {
        Err(_) => {
            panic!("vgic_ipi_handler: vm is None");
        }
        Ok(x) => x,
    };
    let vgic = vm.vgic();

    if vm_id as usize != vm.vm_id() {
        println!(
            "VM {} received vgic msg from another vm {}",
            vm.vm_id(),
            vm_id
        );
        return;
    }

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
                vgic.set_enable(active_vcpu().unwrap(), int_id as usize, val != 0);
            }
            InitcEvent::VgicdSetPend => {
                vgic.set_pend(active_vcpu().unwrap(), int_id as usize, val != 0);
            }
            InitcEvent::VgicdSetPrio => {
                vgic.set_prio(active_vcpu().unwrap(), int_id as usize, val);
            }
            InitcEvent::VgicdSetTrgt => {
                vgic.set_trgt(active_vcpu().unwrap(), int_id as usize, val);
            }
            InitcEvent::VgicdRoute => {
                let interrupt_option =
                    vgic.get_int(active_vcpu().unwrap(), bit_extract(int_id as usize, 0, 10));
                if let Some(interrupt) = interrupt_option {
                    if vgic_int_get_owner(active_vcpu().unwrap(), interrupt.clone()) {
                        if (interrupt.targets() & (1 << cpu_id())) != 0 {
                            vgic.add_lr(active_vcpu().unwrap(), interrupt.clone());
                        }
                        vgic_int_yield_owner(active_vcpu().unwrap(), interrupt.clone());
                    }
                }
            }
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
    vgicd.iidr = GICD.iidr();

    for i in 0..GIC_SPI_MAX {
        vgicd.interrupts.push(VgicInt::new(i));
    }
    drop(vgicd);

    for i in 0..vgic_cpu_num {
        let mut cpu_priv = VgicCpuPriv::default();
        for int_idx in 0..GIC_PRIVINT_NUM {
            let vcpu = vm.vcpu(i);
            let phys_id = vcpu.phys_id();

            cpu_priv.interrupts.push(VgicInt::priv_new(
                int_idx,
                vcpu.clone(),
                1 << phys_id,
                int_idx < GIC_SGIS_NUM,
            ));
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

pub fn vgic_set_hw_int(vm: Vm, int_id: usize) {
    if int_id < GIC_SGIS_NUM {
        return;
    }

    let vgic = vm.vgic();

    if int_id < GIC_PRIVINT_NUM {
        for i in 0..vm.cpu_num() {
            let interrupt_option = vgic.get_int(vm.vcpu(i), int_id);
            match interrupt_option {
                Some(interrupt) => interrupt.set_hw(true),
                None => {}
            }
        }
    } else {
        let interrupt_option = vgic.get_int(vm.vcpu(0), int_id);
        match interrupt_option {
            Some(interrupt) => interrupt.set_hw(true),
            None => {}
        }
    }
}
