use crate::board::{
    PLATFORM_GICC_BASE, PLATFORM_GICD_BASE, PLATFORM_GICH_BASE, PLATFORM_GICV_BASE,
};
use crate::kernel::INTERRUPT_NUM_MAX;
use register::mmio::*;
use register::*;
use spin::Mutex;

// GICD BITS
const GICD_CTLR_EN_BIT: usize = 0x1;

// GICC BITS
const GICC_CTLR_EN_BIT: usize = 0x1;
const GICC_CTLR_EOImodeNS_BIT: usize = 1 << 9;

// GICH BITS
const GICH_HCR_LRENPIE_BIT: usize = 1 << 2;

const GIC_INTS_MAX: usize = 1024;
pub const GIC_SGIS_NUM: usize = 16;
const GIC_PPIS_NUM: usize = 16;
pub const GIC_PRIVINT_NUM: usize = GIC_SGIS_NUM + GIC_PPIS_NUM;
pub const GIC_SPI_MAX: usize = INTERRUPT_NUM_MAX - GIC_PRIVINT_NUM;

const GIC_INT_REGS_NUM: usize = GIC_INTS_MAX / 32;
const GIC_PRIO_REGS_NUM: usize = GIC_INTS_MAX * 8 / 32;
const GIC_TARGET_REGS_NUM: usize = GIC_INTS_MAX * 8 / 32;
const GIC_CONFIG_REGS_NUM: usize = GIC_INTS_MAX * 2 / 32;
const GIC_SEC_REGS_NUM: usize = GIC_INTS_MAX * 2 / 32;
const GIC_SGI_REGS_NUM: usize = GIC_SGIS_NUM * 8 / 32;

pub const GIC_LIST_REGS_NUM: usize = 64;

pub const GICD_TYPER_CPUNUM_OFF: usize = 5;
pub const GICD_TYPER_CPUNUM_LEN: usize = 3;
pub const GICD_TYPER_CPUNUM_MSK: usize = 0b11111;

static GIC_LRS_NUM: Mutex<usize> = Mutex::new(0);

pub enum IrqState {
    IrqSInactive,
    IrqSPend,
    IrqSActive,
    IrqSPendActive,
}

register_structs! {
    #[allow(non_snake_case)]
    pub GicDistributorBlock {
        (0x0000 => CTLR: ReadWrite<u32>),
        (0x0004 => TYPER: ReadOnly<u32>),
        (0x0008 => IIDR: ReadOnly<u32>),
        (0x000c => reserve0),
        (0x0080 => IGROUPR: [ReadWrite<u32>; GIC_INT_REGS_NUM]),
        (0x0100 => ISENABLER: [ReadWrite<u32>; GIC_INT_REGS_NUM]),
        (0x0180 => ICENABLER: [ReadWrite<u32>; GIC_INT_REGS_NUM]),
        (0x0200 => ISPENDR: [ReadWrite<u32>; GIC_INT_REGS_NUM]),
        (0x0280 => ICPENDR: [ReadWrite<u32>; GIC_INT_REGS_NUM]),
        (0x0300 => ISACTIVER: [ReadWrite<u32>; GIC_INT_REGS_NUM]),
        (0x0380 => ICACTIVER: [ReadWrite<u32>; GIC_INT_REGS_NUM]),
        (0x0400 => IPRIORITYR: [ReadWrite<u32>; GIC_PRIO_REGS_NUM]),
        (0x0800 => ITARGETSR: [ReadWrite<u32>; GIC_TARGET_REGS_NUM]),
        (0x0c00 => ICFGR: [ReadWrite<u32>; GIC_CONFIG_REGS_NUM]),
        (0x0d00 => reserve1),
        (0x0e00 => NSACR: [ReadWrite<u32>; GIC_SEC_REGS_NUM]),
        (0x0f00 => SGIR: WriteOnly<u32>),
        (0x0f04 => reserve2),
        (0x0f10 => CPENDSGIR: [ReadWrite<u32>; GIC_SGI_REGS_NUM]),
        (0x0f20 => SPENDSGIR: [ReadWrite<u32>; GIC_SGI_REGS_NUM]),
        (0x0f30 => _reserved_3),
        (0x1000 => @END),
    }
}

pub struct GicDistributor {
    base_addr: usize,
}

impl core::ops::Deref for GicDistributor {
    type Target = GicDistributorBlock;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr() }
    }
}

impl GicDistributor {
    const fn new(base_addr: usize) -> GicDistributor {
        GicDistributor { base_addr }
    }

    pub fn ptr(&self) -> *const GicDistributorBlock {
        self.base_addr as *const GicDistributorBlock
    }

    fn global_init(&self) {
        let int_num = gic_max_spi();

        for i in GIC_PRIVINT_NUM / 32..int_num / 32 {
            self.ICENABLER[i].set(u32::MAX);
            self.ICPENDR[i].set(u32::MAX);
            self.ICACTIVER[i].set(u32::MAX);
        }

        for i in GIC_PRIVINT_NUM..int_num * 8 / 32 {
            self.IPRIORITYR[i].set(u32::MAX);
            self.ITARGETSR[i].set(0);
        }

        let prev = self.CTLR.get();
        self.CTLR.set(prev | GICD_CTLR_EN_BIT as u32);
    }

    fn cpu_init(&self) {
        for i in 0..GIC_PRIVINT_NUM / 32 {
            self.ICENABLER[i].set(u32::MAX);
            self.ICPENDR[i].set(u32::MAX);
            self.ICACTIVER[i].set(u32::MAX);
        }

        for i in 0..GIC_SGIS_NUM * 8 / 32 {
            self.IPRIORITYR[i].set(u32::MAX);
        }

        for i in 0..GIC_PRIVINT_NUM * 8 / 32 {
            self.IPRIORITYR[i].set(u32::MAX);
        }
    }

    pub fn set_enable(&self, int_id: usize, en: bool) {
        let idx = int_id / 32;
        let bit = 1 << (int_id % 32);
        if en {
            self.ISENABLER[idx].set(bit);
        } else {
            self.ICENABLER[idx].set(bit);
        }
    }

    pub fn set_prio(&self, int_id: usize, prio: u8) {
        let idx = int_id * 8 / 32;
        let off = (int_id * 8) % 32;
        let mask: u32 = 0b11111111 << off;

        let prev = self.IPRIORITYR[idx].get();
        let value = (prev & !mask) | (((prio as u32) << off) & mask);
        self.IPRIORITYR[idx].set(value);
    }

    pub fn set_trgt(&self, int_id: usize, trgt: u8) {
        let idx = int_id * 8 / 32;
        let off = (int_id * 8) % 32;
        let mask: u32 = 0b11111111 << off;

        let prev = self.ITARGETSR[idx].get();
        let value = (prev & !mask) | (((trgt as u32) << off) & mask);
        self.ITARGETSR[idx].set(value);
    }

    pub fn typer(&self) -> u32 {
        self.TYPER.get()
    }

    pub fn iidr(&self) -> u32 {
        self.IIDR.get()
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
    (0x00ed => reserved_1),
    (0x00fc => IIDR: ReadOnly<u32>),    // CPU Interface Identification Register
    (0x00fd => reserved_2),
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
        unsafe { &*self.ptr() }
    }
}

impl GicCpuInterface {
    pub const fn new(base_addr: usize) -> GicCpuInterface {
        GicCpuInterface { base_addr }
    }

    pub fn ptr(&self) -> *const GicCpuInterfaceBlock {
        self.base_addr as *const GicCpuInterfaceBlock
    }

    fn init(&self) {
        for i in 0..gich_lrs_num() {
            GICH.LR[i].set(0);
        }

        self.PMR.set(u32::MAX);
        let ctlr_prev = self.CTLR.get();
        self.CTLR
            .set(ctlr_prev | GICC_CTLR_EN_BIT as u32 | GICC_CTLR_EOImodeNS_BIT as u32);

        let hcr_prev = GICH.HCR.get();
        GICH.HCR.set(hcr_prev | GICH_HCR_LRENPIE_BIT as u32);
    }
}

register_structs! {
    #[allow(non_snake_case)]
    pub GicHypervisorInterfaceBlock {
        (0x0000 => HCR: ReadWrite<u32>),
        (0x0004 => VTR: ReadWrite<u32>),
        (0x0008 => VMCR: ReadWrite<u32>),
        (0x000c => reserve0),
        (0x0010 => MISR: ReadWrite<u32>),
        (0x0014 => reserve1),
        (0x0020 => EISR: [ReadWrite<u32>; GIC_LIST_REGS_NUM / 32]),
        (0x0028 => reserve2),
        (0x0030 => ELSR: [ReadWrite<u32>; GIC_LIST_REGS_NUM / 32]),
        (0x0038 => reserve3),
        (0x00f0 => APR: ReadWrite<u32>),
        (0x00f4 => reserve4),
        (0x0100 => LR: [ReadWrite<u32>; GIC_LIST_REGS_NUM]),
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
        unsafe { &*self.ptr() }
    }
}

impl GicHypervisorInterface {
    const fn new(base_addr: usize) -> GicHypervisorInterface {
        GicHypervisorInterface { base_addr }
    }

    pub fn ptr(&self) -> *const GicHypervisorInterfaceBlock {
        self.base_addr as *const GicHypervisorInterfaceBlock
    }
}

// use crate::board::{PLATFORM_GICC_BASE, PLATFORM_GICD_BASE};
pub static GICD: GicDistributor = GicDistributor::new(PLATFORM_GICD_BASE);
pub static GICC: GicCpuInterface = GicCpuInterface::new(PLATFORM_GICC_BASE);
pub static GICH: GicHypervisorInterface = GicHypervisorInterface::new(PLATFORM_GICH_BASE);

#[inline(always)]
pub fn gich_lrs_num() -> usize {
    let vtr = GICH.VTR.get();
    ((vtr & 0b11111) + 1) as usize
}

#[inline(always)]
pub fn gic_max_spi() -> usize {
    let typer = GICD.TYPER.get();
    let value = typer & 0b11111;
    (32 * value + 1) as usize
}

pub fn gic_glb_init() {
    let mut lock = GIC_LRS_NUM.lock();
    *lock = gich_lrs_num();
    drop(lock);

    GICD.global_init();
}

pub fn gic_cpu_init() {
    GICD.cpu_init();
    GICC.init();
}
