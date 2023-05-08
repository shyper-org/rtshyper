use core::sync::atomic::{AtomicUsize, Ordering};

use spin::Mutex;
use tock_registers::*;
use tock_registers::interfaces::*;
use tock_registers::registers::*;

use crate::arch::INTERRUPT_NUM_MAX;
use crate::board::{PlatOperation, Platform};
use crate::kernel::current_cpu;
use crate::util::bit_extract;

// GICD BITS
const GICD_CTLR_EN_BIT: usize = 0x1;

// GICC BITS
pub const GICC_CTLR_EN_BIT: usize = 0x1;
pub const GICC_CTLR_EOIMODENS_BIT: usize = 1 << 9;

// GICH BITS
const GICH_HCR_LRENPIE_BIT: usize = 1 << 2;

pub const GIC_SGIS_NUM: usize = 16;
const GIC_PPIS_NUM: usize = 16;
pub const GIC_INTS_MAX: usize = INTERRUPT_NUM_MAX;
pub const GIC_PRIVINT_NUM: usize = GIC_SGIS_NUM + GIC_PPIS_NUM;
pub const GIC_SPI_MAX: usize = INTERRUPT_NUM_MAX - GIC_PRIVINT_NUM;
pub const GIC_PRIO_BITS: usize = 8;
pub const GIC_TARGET_BITS: usize = 8;
pub const GIC_TARGETS_MAX: usize = GIC_TARGET_BITS;
pub const GIC_CONFIG_BITS: usize = 2;

const GIC_INT_REGS_NUM: usize = GIC_INTS_MAX / 32;
const GIC_PRIO_REGS_NUM: usize = GIC_INTS_MAX * 8 / 32;
const GIC_TARGET_REGS_NUM: usize = GIC_INTS_MAX * 8 / 32;
const GIC_CONFIG_REGS_NUM: usize = GIC_INTS_MAX * 2 / 32;
const GIC_SEC_REGS_NUM: usize = GIC_INTS_MAX * 2 / 32;
const GIC_SGI_REGS_NUM: usize = GIC_SGIS_NUM * 8 / 32;

pub const GIC_LIST_REGS_NUM: usize = 64;

pub const GICD_TYPER_CPUNUM_OFF: usize = 5;
// pub const GICD_TYPER_CPUNUM_LEN: usize = 3;
pub const GICD_TYPER_CPUNUM_MSK: usize = 0b11111;

static GIC_LRS_NUM: AtomicUsize = AtomicUsize::new(0);

static GICD_LOCK: Mutex<()> = Mutex::new(());

#[derive(Copy, Clone, Debug, PartialEq)]
pub(super) enum IrqState {
    Inactive = 0b00,
    Pend = 0b01,
    Active = 0b10,
    PendActive = 0b11,
}

impl From<u32> for IrqState {
    fn from(num: u32) -> Self {
        match num & 0b11 {
            0b00 => IrqState::Inactive,
            0b01 => IrqState::Pend,
            0b10 => IrqState::Active,
            _ => IrqState::PendActive,
        }
    }
}

impl IrqState {
    pub fn is_active(self) -> bool {
        matches!(self, IrqState::Active | IrqState::PendActive)
    }

    pub fn is_pend(self) -> bool {
        matches!(self, IrqState::Pend | IrqState::PendActive)
    }

    pub fn add_pend(self) -> Self {
        match self {
            IrqState::Inactive | IrqState::Pend => IrqState::Pend,
            IrqState::Active | IrqState::PendActive => IrqState::PendActive,
        }
    }

    pub fn clear_pend(self) -> Self {
        match self {
            IrqState::Inactive | IrqState::Active => self,
            IrqState::Pend => IrqState::Inactive,
            IrqState::PendActive => IrqState::Active,
        }
    }

    pub fn add_active(self) -> Self {
        match self {
            IrqState::Inactive => IrqState::Active,
            IrqState::Pend => IrqState::PendActive,
            IrqState::Active | IrqState::PendActive => self,
        }
    }

    pub fn clear_active(self) -> Self {
        match self {
            IrqState::Inactive | IrqState::Pend => self,
            IrqState::Active => IrqState::Inactive,
            IrqState::PendActive => IrqState::Pend,
        }
    }
}

pub struct GicDesc {
    pub gicd_addr: usize,
    pub gicc_addr: usize,
    pub gich_addr: usize,
    pub gicv_addr: usize,
    pub maintenance_int_id: usize,
}

register_structs! {
    #[allow(non_snake_case)]
    pub GicDistributorBlock {
        (0x0000 => CTLR: ReadWrite<u32>),   // Distributor Control Register
        (0x0004 => TYPER: ReadOnly<u32>),   // Interrupt Controller Type Register
        (0x0008 => IIDR: ReadOnly<u32>),    // Distributor Implementer Identification Register
        (0x000c => reserve0),
        (0x0080 => IGROUPR: [ReadWrite<u32>; GIC_INT_REGS_NUM]),    // Interrupt Group Registers
        (0x0100 => ISENABLER: [ReadWrite<u32>; GIC_INT_REGS_NUM]),  // Interrupt Set-Enable Registers
        (0x0180 => ICENABLER: [ReadWrite<u32>; GIC_INT_REGS_NUM]),  // Interrupt Clear-Enable Registers
        (0x0200 => ISPENDR: [ReadWrite<u32>; GIC_INT_REGS_NUM]),    // Interrupt Set-Pending Registers
        (0x0280 => ICPENDR: [ReadWrite<u32>; GIC_INT_REGS_NUM]),    // Interrupt Clear-Pending Registers
        (0x0300 => ISACTIVER: [ReadWrite<u32>; GIC_INT_REGS_NUM]),  // GICv2 Interrupt Set-Active Registers
        (0x0380 => ICACTIVER: [ReadWrite<u32>; GIC_INT_REGS_NUM]),  // Interrupt Clear-Active Registers
        (0x0400 => IPRIORITYR: [ReadWrite<u32>; GIC_PRIO_REGS_NUM]),    // Interrupt Priority Registers
        (0x0800 => ITARGETSR: [ReadWrite<u32>; GIC_TARGET_REGS_NUM]),   // Interrupt Processor Targets Registers
        (0x0c00 => ICFGR: [ReadWrite<u32>; GIC_CONFIG_REGS_NUM]),   // Interrupt Configuration Registers
        (0x0d00 => reserve1),
        (0x0e00 => NSACR: [ReadWrite<u32>; GIC_SEC_REGS_NUM]),      // Non-secure Access Control Registers, optional
        (0x0f00 => SGIR: WriteOnly<u32>),                           // Software Generated Interrupt Register
        (0x0f04 => reserve2),
        (0x0f10 => CPENDSGIR: [ReadWrite<u32>; GIC_SGI_REGS_NUM]),  // SGI Clear-Pending Registers
        (0x0f20 => SPENDSGIR: [ReadWrite<u32>; GIC_SGI_REGS_NUM]),  // SGI Set-Pending Registers
        (0x0f30 => _reserved_3),
        (0x1000 => @END),
    }
}

pub(super) struct GicDistributor {
    base_addr: usize,
}

impl core::ops::Deref for GicDistributor {
    type Target = GicDistributorBlock;
    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.base_addr as *const Self::Target) }
    }
}

impl GicDistributor {
    const fn new(base_addr: usize) -> GicDistributor {
        GicDistributor { base_addr }
    }

    pub fn is_enabler(&self, idx: usize) -> u32 {
        self.ISENABLER[idx].get()
    }

    pub fn is_activer(&self, idx: usize) -> u32 {
        self.ISACTIVER[idx].get()
    }

    pub fn is_pender(&self, idx: usize) -> u32 {
        self.ISPENDR[idx].get()
    }

    pub fn cpendsgir(&self, idx: usize) -> u32 {
        self.CPENDSGIR[idx].get()
    }

    pub fn igroup(&self, idx: usize) -> u32 {
        self.IGROUPR[idx].get()
    }

    pub fn ipriorityr(&self, idx: usize) -> u32 {
        self.IPRIORITYR[idx].get()
    }

    pub fn itargetsr(&self, idx: usize) -> u32 {
        self.ITARGETSR[idx].get()
    }

    pub fn ctlr(&self) -> u32 {
        self.CTLR.get()
    }

    pub fn icfgr(&self, idx: usize) -> u32 {
        self.ICFGR[idx].get()
    }

    pub fn ic_enabler(&self, idx: usize) -> u32 {
        self.ICENABLER[idx].get()
    }

    fn global_init(&self) {
        let int_num = gic_max_spi();

        for i in GIC_PRIVINT_NUM / 32..int_num / 32 {
            self.ICENABLER[i].set(u32::MAX);
            self.ICPENDR[i].set(u32::MAX);
            self.ICACTIVER[i].set(u32::MAX);
        }

        for i in GIC_PRIVINT_NUM / 4..int_num * 8 / 32 {
            self.IPRIORITYR[i].set(u32::MAX);
            self.ITARGETSR[i].set(0);
        }

        let prev = self.CTLR.get();
        self.CTLR.set(prev | GICD_CTLR_EN_BIT as u32);
    }

    fn cpu_init(&self) {
        for i in 0..GIC_PRIVINT_NUM / 32 {
            /*
             * Make sure all private interrupts are not enabled, non pending,
             * non active.
             */
            self.ICENABLER[i].set(u32::MAX);
            self.ICPENDR[i].set(u32::MAX);
            self.ICACTIVER[i].set(u32::MAX);
        }

        /* Clear any pending SGIs. */
        for i in 0..GIC_SGI_REGS_NUM {
            self.CPENDSGIR[i].set(u32::MAX);
        }

        /* All interrupts have lowest priority possible by default */
        for i in 0..(GIC_PRIVINT_NUM * 8) / 32 {
            self.IPRIORITYR[i].set(u32::MAX);
        }
    }

    pub fn send_sgi(&self, cpu_if: usize, sgi_num: usize) {
        // println!("Core {} send ipi to cpu {}", cpu_id(), cpu_if);
        self.SGIR.set(((1 << (16 + cpu_if)) | (sgi_num & 0b1111)) as u32);
    }

    pub fn prio(&self, int_id: usize) -> usize {
        let idx = (int_id * 8) / 32;
        let off = (int_id * 8) % 32;
        ((self.IPRIORITYR[idx].get() >> off) & 0xff) as usize
    }

    pub fn set_prio(&self, int_id: usize, prio: u8) {
        let idx = (int_id * 8) / 32;
        let off = (int_id * 8) % 32;
        let mask: u32 = 0b11111111 << off;

        let lock = GICD_LOCK.lock();
        let prev = self.IPRIORITYR[idx].get();
        let value = (prev & !mask) | (((prio as u32) << off) & mask);
        self.IPRIORITYR[idx].set(value);
        drop(lock);
    }

    pub fn trgt(&self, int_id: usize) -> usize {
        let idx = (int_id * 8) / 32;
        let off = (int_id * 8) % 32;
        ((self.ITARGETSR[idx].get() >> off) & 0xff) as usize
    }

    pub fn set_trgt(&self, int_id: usize, trgt: u8) {
        let idx = (int_id * 8) / 32;
        let off = (int_id * 8) % 32;
        let mask: u32 = 0b11111111 << off;

        let lock = GICD_LOCK.lock();
        let prev = self.ITARGETSR[idx].get();
        let value = (prev & !mask) | (((trgt as u32) << off) & mask);
        // println!("idx {}, val {:x}", idx, value);
        self.ITARGETSR[idx].set(value);
        drop(lock);
    }

    pub fn set_enable(&self, int_id: usize, en: bool) {
        // println!("gicd::set_enbale: en {}, int_id {}", en, int_id);
        let idx = int_id / 32;
        let bit = 1 << (int_id % 32);

        let lock = GICD_LOCK.lock();
        if en {
            self.ISENABLER[idx].set(bit);
        } else {
            self.ICENABLER[idx].set(bit);
        }
        drop(lock);
    }

    pub fn set_pend(&self, int_id: usize, pend: bool) {
        let lock = GICD_LOCK.lock();
        if gic_is_sgi(int_id) {
            let reg_ind = int_id / 4;
            let off = (int_id % 4) * 8;
            if pend {
                self.SPENDSGIR[reg_ind].set(1 << (off + current_cpu().id));
            } else {
                self.CPENDSGIR[reg_ind].set(0b11111111 << off);
            }
        } else {
            let reg_ind = int_id / 32;
            let mask = 1 << (int_id % 32);
            if pend {
                self.ISPENDR[reg_ind].set(mask);
            } else {
                self.ICPENDR[reg_ind].set(mask);
            }
        }

        drop(lock);
    }

    pub fn set_act(&self, int_id: usize, act: bool) {
        let reg_ind = int_id / 32;
        let mask = 1 << (int_id % 32);

        let lock = GICD_LOCK.lock();
        if act {
            self.ISACTIVER[reg_ind].set(mask);
        } else {
            self.ICACTIVER[reg_ind].set(mask);
        }
        drop(lock);
    }

    pub fn set_state(&self, int_id: usize, state: IrqState) {
        self.set_act(int_id, state.is_active());
        self.set_pend(int_id, state.is_pend());
    }

    pub fn set_icfgr(&self, int_id: usize, cfg: u8) {
        let lock = GICD_LOCK.lock();
        let reg_ind = (int_id * GIC_CONFIG_BITS) / 32;
        let off = (int_id * GIC_CONFIG_BITS) % 32;
        let mask = 0b11 << off;

        let icfgr = self.ICFGR[reg_ind].get();
        self.ICFGR[reg_ind].set((icfgr & !mask) | (((cfg as u32) << off) & mask));
        drop(lock);
    }

    pub fn typer(&self) -> u32 {
        self.TYPER.get()
    }

    pub fn iidr(&self) -> u32 {
        self.IIDR.get()
    }

    pub fn state(&self, int_id: usize) -> usize {
        let reg_ind = int_id / 32;
        let mask = 1 << (int_id % 32);

        let lock = GICD_LOCK.lock();
        let pend = usize::from((self.ISPENDR[reg_ind].get() & mask) != 0);
        let act = usize::from((self.ISACTIVER[reg_ind].get() & mask) != 0) << 1;
        drop(lock);
        pend | act
    }
}

register_structs! {
  #[allow(non_snake_case)]
  pub GicCpuInterfaceBlock {
    (0x0000 => CTLR: ReadWrite<u32>),   // CPU Interface Control Register
    (0x0004 => PMR: ReadWrite<u32>),    // Interrupt Priority Mask Register
    (0x0008 => BPR: ReadWrite<u32>),    // Binary Point Register
    (0x000c => IAR: ReadOnly<u32>),     // Interrupt Acknowledge Register
    (0x0010 => EOIR: WriteOnly<u32>),   // End of Interrupt Register
    (0x0014 => RPR: ReadOnly<u32>),     // Running Priority Register
    (0x0018 => HPPIR: ReadOnly<u32>),   // Highest Priority Pending Interrupt Register
    (0x001c => ABPR: ReadWrite<u32>),   // Aliased Binary Point Register
    (0x0020 => AIAR: ReadOnly<u32>),    // Aliased Interrupt Acknowledge Register
    (0x0024 => AEOIR: WriteOnly<u32>),  // Aliased End of Interrupt Register
    (0x0028 => AHPPIR: ReadOnly<u32>),  // Aliased Highest Priority Pending Interrupt Register
    (0x002c => reserved_0),
    (0x00d0 => APR: [ReadWrite<u32>; 4]),    // Active Priorities Register
    (0x00e0 => NSAPR: [ReadWrite<u32>; 4]),  // Non-secure Active Priorities Register
    (0x00f0 => reserved_1),
    (0x00fc => IIDR: ReadOnly<u32>),    // CPU Interface Identification Register
    (0x0100 => reserved_2),
    (0x1000 => DIR: WriteOnly<u32>),    // Deactivate Interrupt Register
    (0x1004 => reserved_3),
    (0x2000 => @END),
  }
}

pub struct GicCpuInterface {
    base_addr: usize,
}

impl core::ops::Deref for GicCpuInterface {
    type Target = GicCpuInterfaceBlock;
    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.base_addr as *const Self::Target) }
    }
}

impl GicCpuInterface {
    pub const fn new(base_addr: usize) -> GicCpuInterface {
        GicCpuInterface { base_addr }
    }

    fn init(&self) {
        for i in 0..gic_lrs() {
            GICH.LR[i].set(0);
        }

        self.PMR.set(u32::MAX);
        let ctlr_prev = self.CTLR.get();
        self.CTLR
            .set(ctlr_prev | GICC_CTLR_EN_BIT as u32 | GICC_CTLR_EOIMODENS_BIT as u32);

        let hcr_prev = GICH.HCR.get();
        GICH.HCR.set(hcr_prev | GICH_HCR_LRENPIE_BIT as u32);
    }

    pub fn hppir(&self) -> u32 {
        self.HPPIR.get()
    }

    pub fn rpr(&self) -> u32 {
        self.RPR.get()
    }

    pub fn bpr(&self) -> u32 {
        self.BPR.get()
    }

    pub fn abpr(&self) -> u32 {
        self.ABPR.get()
    }

    pub fn apr(&self, idx: usize) -> u32 {
        self.APR[idx].get()
    }

    pub fn nsapr(&self, idx: usize) -> u32 {
        self.NSAPR[idx].get()
    }
}

register_structs! {
    #[allow(non_snake_case)]
    pub GicHypervisorInterfaceBlock {
        (0x0000 => HCR: ReadWrite<u32>),    // Hypervisor Control Register
        (0x0004 => VTR: ReadOnly<u32>),     // VGIC Type Register
        (0x0008 => VMCR: ReadWrite<u32>),   // Virtual Machine Control Register
        (0x000c => reserve0),
        (0x0010 => MISR: ReadOnly<u32>),    // Maintenance Interrupt Status Register
        (0x0014 => reserve1),
        (0x0020 => EISR: [ReadOnly<u32>; GIC_LIST_REGS_NUM / 32]),  // End of Interrupt Status Registers
        (0x0028 => reserve2),
        (0x0030 => ELRSR: [ReadOnly<u32>; GIC_LIST_REGS_NUM / 32]), // Empty List Register Status Registers
        (0x0038 => reserve3),
        (0x00f0 => APR: ReadWrite<u32>),    // Active Priorities Register
        (0x00f4 => reserve4),
        (0x0100 => LR: [ReadWrite<u32>; GIC_LIST_REGS_NUM]),    // List Registers 0-63
        (0x0200 => reserve5),
        (0x1000 => @END),
    }
}

pub struct GicHypervisorInterface {
    base_addr: usize,
}

impl core::ops::Deref for GicHypervisorInterface {
    type Target = GicHypervisorInterfaceBlock;
    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.base_addr as *const Self::Target) }
    }
}

impl GicHypervisorInterface {
    const fn new(base_addr: usize) -> GicHypervisorInterface {
        GicHypervisorInterface { base_addr }
    }

    pub fn hcr(&self) -> u32 {
        self.HCR.get()
    }

    pub fn set_hcr(&self, hcr: u32) {
        self.HCR.set(hcr);
    }

    pub fn elrsr(&self, elsr_idx: usize) -> u32 {
        self.ELRSR[elsr_idx].get()
    }

    pub fn eisr(&self, eisr_idx: usize) -> u32 {
        self.EISR[eisr_idx].get()
    }

    pub fn lr(&self, lr_idx: usize) -> u32 {
        self.LR[lr_idx].get()
    }

    pub fn misr(&self) -> u32 {
        self.MISR.get()
    }

    pub fn apr(&self) -> u32 {
        self.APR.get()
    }

    pub fn set_lr(&self, lr_idx: usize, val: u32) {
        self.LR[lr_idx].set(val)
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct GicState {
    pub hcr: u32,
    eisr: [u32; GIC_LIST_REGS_NUM / 32],
    elrsr: [u32; GIC_LIST_REGS_NUM / 32],
    apr: u32,
    pub lr: [u32; GIC_LIST_REGS_NUM],
    pub ctlr: u32,
}

impl Default for GicState {
    fn default() -> Self {
        Self {
            hcr: 0,
            eisr: [0; GIC_LIST_REGS_NUM / 32],
            elrsr: [0; GIC_LIST_REGS_NUM / 32],
            apr: 0,
            lr: [0; GIC_LIST_REGS_NUM],
            ctlr: 0,
        }
    }
}

impl GicState {
    pub fn save_state(&mut self) {
        self.hcr = GICH.hcr();
        self.apr = GICH.APR.get();
        for i in 0..(GIC_LIST_REGS_NUM / 32) {
            self.eisr[i] = GICH.eisr(i);
            self.elrsr[i] = GICH.elrsr(i);
        }
        // println!("save state");
        // println!("GICH hcr {:x}", self.hcr);
        // println!("GICH apr {:x}", self.apr);
        // println!("GICH eisr {:x}", self.eisr[0]);
        // println!("GICH elrsr {:x}", self.elrsr[0]);
        for i in 0..gic_lrs() {
            if self.elrsr[0] & 1 << i == 0 {
                self.lr[i] = GICH.lr(i);
            } else {
                self.lr[i] = 0;
            }
            // println!("GICH_LR[{}] {:x}", i, GICH.lr(i));
        }
        self.ctlr = GICC.CTLR.get();
    }

    pub fn restore_state(&self) {
        // println!("before restore");
        // println!("GICH hcr {:x}", GICH.hcr());
        // println!("GICC ctlr {:x}", GICC.CTLR.get());
        // for i in 0..gic_lrs() {
        //     println!("lr[{}] {:x}", i, GICH.lr(i));
        // }

        // println!("after restore state");
        GICH.set_hcr(self.hcr);
        GICH.APR.set(self.apr);
        // println!("GICH hcr {:x}", self.hcr);
        // println!("GICH apr {:x}", self.apr);

        for i in 0..gic_lrs() {
            // println!("lr[{}] {:x}", i, self.lr[i]);
            GICH.set_lr(i, self.lr[i]);
        }
        GICC.CTLR.set(self.ctlr);
        // println!("GICC ctlr {:x}", self.ctlr);
    }
}

pub(super) static GICD: GicDistributor = GicDistributor::new(Platform::GICD_BASE);
pub(super) static GICC: GicCpuInterface = GicCpuInterface::new(Platform::GICC_BASE);
pub(super) static GICH: GicHypervisorInterface = GicHypervisorInterface::new(Platform::GICH_BASE);

#[inline(always)]
fn gic_max_spi() -> usize {
    let typer = GICD.TYPER.get();
    let value = typer & 0b11111;
    (32 * (value + 1)) as usize
}

pub fn gic_glb_init() {
    let gich_lrs_num = {
        let vtr = GICH.VTR.get();
        ((vtr & 0b111111) + 1) as usize
    };

    GIC_LRS_NUM.store(gich_lrs_num, Ordering::Relaxed);
    GICD.global_init();
}

pub fn gic_cpu_init() {
    GICD.cpu_init();
    GICC.init();
}

pub fn gic_cpu_reset() {
    GICC.init();
}

pub fn gic_is_priv(int_id: usize) -> bool {
    int_id < GIC_PRIVINT_NUM
}

fn gic_is_sgi(int_id: usize) -> bool {
    int_id < GIC_SGIS_NUM
}

pub(super) fn gicc_clear_current_irq(for_hypervisor: bool) {
    let irq = current_cpu().current_irq as u32;
    if irq == 0 {
        return;
    }
    GICC.EOIR.set(irq);
    if for_hypervisor {
        GICC.DIR.set(irq);
    }
    current_cpu().current_irq = 0;
}

pub(super) fn gicc_get_current_irq() -> Option<(usize, usize)> {
    let iar = GICC.IAR.get();
    let irq = iar as usize;
    current_cpu().current_irq = irq;
    let id = bit_extract(iar as usize, 0, 10);
    let src = bit_extract(iar as usize, 10, 3);
    if id >= 1022 {
        None
    } else {
        Some((id, src))
    }
}

pub fn gic_lrs() -> usize {
    GIC_LRS_NUM.load(Ordering::Relaxed)
}
