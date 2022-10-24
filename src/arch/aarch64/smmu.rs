use alloc::vec::Vec;
use core::mem::size_of;
use core::ptr;

use spin::Mutex;
use tock_registers::*;
use tock_registers::interfaces::*;
use tock_registers::registers::*;

use crate::board::PLAT_DESC;
use crate::device::EmuContext;
use crate::kernel::VM_NUM_MAX;
use crate::kernel::Vm;
use crate::kernel::{active_vm, active_vm_id, current_cpu};
use crate::lib::{bit_extract, FlexBitmap};

pub struct SmmuDesc {
    pub base: usize,
    pub interrupt_id: usize,
    pub global_mask: u16,
}

const SMMUV2_CBAR_TYPE_S1_S2: usize = 0x3 << 16;

const SMMUV2_IDR0_S1TS_BIT: usize = 1 << 30;
const SMMUV2_IDR0_S2TS_BIT: usize = 1 << 29;
const SMMUV2_IDR0_NTS_BIT: usize = 1 << 28;
const SMMUV2_IDR0_SMS_BIT: usize = 1 << 27;
const SMMUV2_IDR0_CTTW_BIT: usize = 1 << 14;
const SMMUV2_IDR0_BTM_BIT: usize = 1 << 13;

const SMMUV2_IDR1_PAGESIZE_BIT: usize = 1 << 31;
const SMMUV2_IDR1_NUMCB_OFF: usize = 0;
const SMMUV2_IDR1_NUMCB_LEN: usize = 8;
const SMMUV2_IDR1_NUMS2CB_OFF: usize = 16;
const SMMUV2_IDR1_NUMS2CB_LEN: usize = 8;
const SMMUV2_IDR1_NUMPAGEDXB_OFF: usize = 28;
const SMMUV2_IDR1_NUMPAGEDXB_LEN: usize = 3;

const SMMUV2_IDR2_PTFSV8_4KB_BIT: usize = 1 << 12;

const SMMUV2_CR0_CLIENTPD: usize = 1;
const SMMUV2_CR0_GFRE: usize = 1 << 1;
const SMMUV2_CR0_GFIE: usize = 1 << 2;
const SMMUV2_CR0_GCFGFRE: usize = 1 << 4;
const SMMUV2_CR0_GCFGFIE: usize = 1 << 5;
const SMMUV2_CR0_USFCFG: usize = 1 << 10;
const SMMUV2_CR0_SMCFCFG: usize = 1 << 21;

const SMMU_RS1_CBAR: usize = 0;
const SMMU_RS1_RES0: usize = 0x200;

const SMMUV2_CB_TTBA_END: usize = 48;

const SMMUV2_TCR_PS_OFF: usize = 16;
const SMMUV2_TCR_TG0_4K: usize = 0;
const SMMUV2_TCR_IRGN0_WB_RA_WA: usize = 1 << 8;
const SMMUV2_TCR_ORGN0_WB_RA_WA: usize = 1 << 10;
const SMMUV2_TCR_SH0_IS: usize = 0x3 << 12;
const SMMUV2_TCR_SL0_1: usize = 0x1 << 6;

const SMMUV2_SCTLR_CFIE: usize = 1 << 6;
const SMMUV2_SCTLR_CFRE: usize = 1 << 5;
const SMMUV2_SCTLR_M: usize = 1;

const SMMU_SMR_ID_OFF: usize = 0;
const SMMU_SMR_ID_LEN: usize = 15;
const SMMU_SMR_MASK_OFF: usize = 16;
const SMMU_SMR_MASK_LEN: usize = 15;

const SMMUV2_SMR_VALID: usize = 0x1 << 31;

const S2CR_CBNDX_OFF: usize = 0;
const S2CR_CBNDX_LEN: usize = 8;

const S2CR_IMPL_OFF: usize = 30;
const S2CR_IMPL_LEN: usize = 2;

const S2CR_DFLT: usize = 0;

macro_rules! bit_mask {
    ($off: expr, $len: expr) => {
        ((1 << ($off + $len)) - 1) & !((1 << $off) - 1)
    };
}

register_structs! {
    #[allow(non_snake_case)]
    pub SmmuGlobalRegisterSpace0 {
        (0x0000 => CR0: ReadWrite<u32>),
        (0x0004 => SCR1: ReadWrite<u32>),
        (0x0008 => CR2: ReadWrite<u32>),
        (0x000c => reserved_0),
        (0x0010 => ACR: ReadWrite<u32>),
        (0x0014 => reserved_1),
        (0x0020 => IDR0: ReadOnly<u32>),
        (0x0024 => IDR1: ReadOnly<u32>),
        (0x0028 => IDR2: ReadOnly<u32>),
        (0x002c => IDR3: ReadOnly<u32>),
        (0x0030 => IDR4: ReadOnly<u32>),
        (0x0034 => IDR5: ReadOnly<u32>),
        (0x0038 => IDR6: ReadOnly<u32>),
        (0x003c => IDR7: ReadOnly<u32>),
        (0x0040 => GFAR: ReadWrite<u64>),
        (0x0048 => GFSR: ReadWrite<u32>),
        (0x004c => GFSRRESTORE: WriteOnly<u32>),
        (0x0050 => GFSYNR0: ReadWrite<u32>),
        (0x0054 => GFSYNR1: ReadWrite<u32>),
        (0x0058 => GFSYNR2: ReadWrite<u32>),
        (0x005c => reserved_2),
        (0x0060 => STLBIALL: WriteOnly<u32>),
        (0x0064 => TLBIVMID: WriteOnly<u32>),
        (0x0068 => TLBIALLNSNH: WriteOnly<u32>),
        (0x006c => TLBIALLH: WriteOnly<u32>),
        (0x0070 => TLBGSYNC: WriteOnly<u32>),
        (0x0074 => TLBGSTATUS: ReadOnly<u32>),
        (0x0078 => TLBIVAH: WriteOnly<u32>),
        (0x007c => reserved_3),
        (0x00a0 => STLBIVALM: WriteOnly<u64>),
        (0x00a8 => STLBIVAM: WriteOnly<u64>),
        (0x00b0 => TLBIVALH64: WriteOnly<u64>),
        (0x00b8 => TLBIVMIDS1: WriteOnly<u32>),
        (0x00bc => TLBIALLM: WriteOnly<u32>),
        (0x00c0 => TLBIVAH64: WriteOnly<u64>),
        (0x00c8 => reserved_4),
        (0x0100 => GATS1UR: WriteOnly<u64>),
        (0x0108 => GATS1UW: WriteOnly<u64>),
        (0x0110 => GATS1PR: WriteOnly<u64>),
        (0x0118 => GATS1PW: WriteOnly<u64>),
        (0x0120 => GATS12UR: WriteOnly<u64>),
        (0x0128 => GATS12UW: WriteOnly<u64>),
        (0x0130 => GATS12PR: WriteOnly<u64>),
        (0x0138 => GATS12PW: WriteOnly<u64>),
        (0x0140 => reserved_5),
        (0x0180 => GPAR: ReadWrite<u64>),
        (0x0188 => GATSR: ReadOnly<u32>),
        (0x018c => reserved_6),
        (0x0400 => NSCR0: ReadWrite<u32>),
        (0x0404 => reserved_7),
        (0x0408 => NSCR2: ReadWrite<u32>),
        (0x040c => reserved_8),
        (0x0410 => NSACR2: ReadWrite<u32>),
        (0x0414 => reserved_9),
        (0x0440 => NSGFAR: ReadWrite<u64>),
        (0x0448 => NSGFSR: ReadWrite<u32>),
        (0x044c => NSGFSRRESTORE: WriteOnly<u32>),
        (0x0450 => NSGFSYNR0: ReadWrite<u32>),
        (0x0454 => NSGFSYNR1: ReadWrite<u32>),
        (0x0458 => NSGFSYNR2: ReadWrite<u32>),
        (0x045c => reserved_10),
        (0x0470 => NSTLBGSYNC: WriteOnly<u32>),
        (0x0474 => NSTLBGSTATUS: ReadOnly<u32>),
        (0x0478 => reserved_11),
        (0x0500 => NSGATS1UR: WriteOnly<u64>),
        (0x0508 => NSGATS1UW: WriteOnly<u64>),
        (0x0510 => NSGATS1PR: WriteOnly<u64>), // NOT SURE
        (0x0518 => NSGATS1PW: WriteOnly<u64>),
        (0x0520 => NSGATS12UR: WriteOnly<u64>),
        (0x0528 => NSGATS12UW: WriteOnly<u64>),
        (0x0530 => NSGATS12PR: WriteOnly<u64>),
        (0x0538 => NSGATS12PW: WriteOnly<u64>),
        (0x0540 => reserved_12),
        (0x0580 => NSGPAR: ReadWrite<u64>),
        (0x0588 => NSGATSR: ReadOnly<u32>),
        (0x058c => reserved_13),
        (0x0800 => SMR: [ReadWrite<u32>; 128]),
        (0x0a00 => reserved_14),
        (0x0c00 => S2CR: [ReadWrite<u32>; 128]),
        (0x0e00 => reserved_15),
        (0x1000 => @END),
    }
}

pub struct SmmuGlbRS0 {
    base_addr: usize,
}

impl core::ops::Deref for SmmuGlbRS0 {
    type Target = SmmuGlobalRegisterSpace0;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr() }
    }
}

impl SmmuGlbRS0 {
    const fn new(base_addr: usize) -> SmmuGlbRS0 {
        SmmuGlbRS0 { base_addr }
    }

    pub fn ptr(&self) -> *const SmmuGlobalRegisterSpace0 {
        self.base_addr as *const SmmuGlobalRegisterSpace0
    }
}

register_structs! {
    #[allow(non_snake_case)]
    pub SmmuGlobalRegisterSpace1 {
        (0x0000 => CBAR: [ReadWrite<u32>; 128]),
        (0x0200 => reserved_0),
        (0x0400 => CBFRSYNRA: [ReadWrite<u32>; 128]),
        (0x0600 => reserved_1),
        (0x0800 => CBA2R: [ReadWrite<u32>; 128]),
        (0x0a00 => reserved_2),
        (0x1000 => @END),
    }
}

pub struct SmmuGlbRS1 {
    base_addr: usize,
}

impl core::ops::Deref for SmmuGlbRS1 {
    type Target = SmmuGlobalRegisterSpace1;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr() }
    }
}

impl SmmuGlbRS1 {
    const fn new(base_addr: usize) -> SmmuGlbRS1 {
        SmmuGlbRS1 { base_addr }
    }

    pub fn ptr(&self) -> *const SmmuGlobalRegisterSpace1 {
        self.base_addr as *const SmmuGlobalRegisterSpace1
    }
}

register_structs! {
    #[allow(non_snake_case)]
    pub SmmuStage2TranslationContextBankAddressSpace {
        (0x0000 => SCTLR: ReadWrite<u32>),
        (0x0004 => ACTLR: ReadWrite<u32>),
        (0x0008 => RESUME: WriteOnly<u32>),
        (0x000c => reserved_0),
        (0x0020 => TTBR0: ReadWrite<u64>),
        (0x0028 => reserved_1),
        (0x0030 => TCR: ReadWrite<u32>),
        (0x0034 => reserved_2),
        (0x0058 => FSR: ReadWrite<u32>),
        (0x005c => FSRRESTORE: WriteOnly<u32>),
        (0x0060 => FAR: ReadWrite<u64>),
        (0x0068 => FSYNR0: ReadWrite<u32>),
        (0x006c => FSYNR1: ReadWrite<u32>),
        (0x0070 => IPAFAR: ReadWrite<u64>),
        (0x0078 => reserved_3),
        (0x0630 => TLBIIPAS2: WriteOnly<u64>),
        (0x0638 => TLBIIPAS2L: WriteOnly<u64>),
        (0x0640 => reserved_4),
        (0x07F0 => TLBSYNC: WriteOnly<u32>),
        (0x07F4 => TLBSTATUS: ReadOnly<u32>),
        (0x07F8 => reserved_5),
        (0x0e00 => PMEVCNTRm: ReadWrite<u32>),   // what about 0xe04~0xe38
        (0x0e3c => reserved_6),
        (0x0e80 => PMEVTYPERm: ReadWrite<u32>),   // what about 0xe84~0xeB8
        (0x0ebc => reserved_7),
        (0x0f00 => PMCFGR: ReadOnly<u32>),
        (0x0f04 => PMCR: ReadWrite<u32>),
        (0x0f08 => reserved_8),
        (0x0f20 => PMCEID0: ReadOnly<u32>),
        (0x0f24 => PMCEID1: ReadOnly<u32>),
        (0x0f28 => reserved_9),
        (0x0f40 => PMCNTENSET: ReadWrite<u32>),
        (0x0f44 => PMCNTENCLR: ReadWrite<u32>),
        (0x0f48 => PMINTENSET: ReadWrite<u32>),
        (0x0f4c => PMINTENCLR: ReadWrite<u32>),
        (0x0f50 => PMOVSCLR: ReadWrite<u32>),
        (0x0f54 => reserved_10),
        (0x0f58 => PMOVSSET: ReadWrite<u32>),
        (0x0f5c => reserved_11),
        (0x0fb8 => PMAUTHSTATUS: ReadOnly<u32>),
        (0x0fbc => reserved_12),
        (0x1000 => @END),
    }
}

pub struct SmmuContextBank {
    base_addr: usize,
}

impl core::ops::Deref for SmmuContextBank {
    type Target = SmmuStage2TranslationContextBankAddressSpace;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.ptr() }
    }
}

impl SmmuContextBank {
    const fn new(base_addr: usize) -> SmmuContextBank {
        SmmuContextBank { base_addr }
    }

    pub fn ptr(&self) -> *const SmmuStage2TranslationContextBankAddressSpace {
        self.base_addr as *const SmmuStage2TranslationContextBankAddressSpace
    }
}

pub struct SmmuV2 {
    glb_rs0: Option<SmmuGlbRS0>,
    glb_rs1: Option<SmmuGlbRS1>,
    context_s2_idx: usize,
    context_bank: Vec<SmmuContextBank>,
    context_alloc_bitmap: Option<FlexBitmap>,

    smr_num: usize,
    smr_alloc_bitmap: Option<FlexBitmap>,
    group_alloc_bitmap: Option<FlexBitmap>,
}

impl SmmuV2 {
    pub const fn new() -> SmmuV2 {
        SmmuV2 {
            glb_rs0: None,
            glb_rs1: None,
            context_s2_idx: 0,
            context_bank: vec![],
            context_alloc_bitmap: None,
            smr_num: 0,
            smr_alloc_bitmap: None,
            group_alloc_bitmap: None,
        }
    }

    pub fn init(&mut self) {
        let smmu_base_addr = PLAT_DESC.arch_desc.smmu_desc.base + 0x8_0000_0000;

        self.glb_rs0 = Some(SmmuGlbRS0::new(smmu_base_addr));
        let rs0 = self.glb_rs0.as_ref().unwrap();
        /* IDR1 */
        let idr1 = rs0.IDR1.get() as usize;
        let page_size = if (idr1 & SMMUV2_IDR1_PAGESIZE_BIT) == 0 {
            0x1000
        } else {
            0x10000
        };

        self.glb_rs1 = Some(SmmuGlbRS1::new(smmu_base_addr + page_size));
        let num_pages = 1 << (1 + bit_extract(idr1, SMMUV2_IDR1_NUMPAGEDXB_OFF, SMMUV2_IDR1_NUMPAGEDXB_LEN));
        let context_bank_num = bit_extract(idr1, SMMUV2_IDR1_NUMCB_OFF, SMMUV2_IDR1_NUMCB_LEN);
        let context_base = smmu_base_addr + num_pages * page_size;
        for i in 0..context_bank_num {
            self.context_bank
                .push(SmmuContextBank::new(context_base + page_size * i));
        }
        let stage2_context_bank_num = bit_extract(idr1, SMMUV2_IDR1_NUMS2CB_OFF, SMMUV2_IDR1_NUMS2CB_LEN);
        // TODO: not a good way to seperate context bank into 2 parts,
        // one for VMs, one for hypervisor
        self.context_s2_idx = context_bank_num - VM_NUM_MAX;
        self.context_alloc_bitmap = Some(FlexBitmap::new(context_bank_num));

        self.check_features();

        // NUMSMRG: Number of Stream Mapping Register Groups
        // Indicates the number of Stream mapping register groups in the Stream match table, in the range 0-128.
        let smr_num = rs0.IDR0.get() as usize & 0xff;
        self.smr_num = smr_num;
        self.smr_alloc_bitmap = Some(FlexBitmap::new(smr_num));
        self.group_alloc_bitmap = Some(FlexBitmap::new(smr_num));

        /* Clear random reset state. */
        rs0.GFSR.set(rs0.GFSR.get());
        rs0.NSGFSR.set(rs0.NSGFSR.get());

        println!(
            concat!(
                "SMMU info:\n",
                "  page size {:#x}, num pages {}, context base {:#x}\n",
                "  stream matching with {} register groups\n",
                "  {} context banks ({} stage-2 only)"
            ),
            page_size, num_pages, context_base, smr_num, context_bank_num, stage2_context_bank_num,
        );

        for i in 0..smr_num {
            rs0.SMR[i].set(0);
        }
        for i in 0..context_bank_num {
            self.context_bank[i].SCTLR.set(0);
            self.context_bank[i].FSR.set(u32::MAX);
        }

        /* Enable IOMMU. */
        let mut cr0 = rs0.CR0.get() as usize;
        cr0 &= (0x3 << 30) | (0x1 << 11);
        // fault and interrupt configration
        cr0 |= SMMUV2_CR0_USFCFG | SMMUV2_CR0_SMCFCFG;
        cr0 |= SMMUV2_CR0_GFRE | SMMUV2_CR0_GFIE | SMMUV2_CR0_GCFGFRE | SMMUV2_CR0_GCFGFIE;
        cr0 &= !SMMUV2_CR0_CLIENTPD;
        rs0.CR0.set(cr0 as u32);
    }

    pub fn check_features(&self) {
        let glb_rs0 = self.glb_rs0.as_ref().unwrap();
        let version = bit_extract(glb_rs0.IDR7.get() as usize, 4, 4);
        if version != 2 {
            panic!("smmu unspoorted version: {}", version);
        }

        if glb_rs0.IDR0.get() as usize & SMMUV2_IDR0_S2TS_BIT == 0 {
            panic!("smmuv2 does not support 2nd stage translation");
        } else {
            if glb_rs0.IDR0.get() as usize & SMMUV2_IDR0_NTS_BIT == 0 {
                panic!("smmuv2 does not support Nested Translation (Stage 1 followed by stage 2 translation)");
            }
        }

        if glb_rs0.IDR0.get() as usize & SMMUV2_IDR0_SMS_BIT == 0 {
            panic!("smmuv2 does not support stream match");
        }

        /**
         * TODO: the most common smmuv2 implementation (mmu-500) does not provide
         * ptw coherency. So we must add some mechanism software-managed
         * coherency mechanism for the vms using the smmu according to the
         * result of this feature test.
         */
        if glb_rs0.IDR0.get() as usize & SMMUV2_IDR0_CTTW_BIT == 0 {
            println!("smmuv2 does not support coherent page table walks");
        }

        if glb_rs0.IDR0.get() as usize & SMMUV2_IDR0_BTM_BIT == 0 {
            panic!("smmuv2 does not support tlb maintenance broadcast");
        }

        if glb_rs0.IDR2.get() as usize & SMMUV2_IDR2_PTFSV8_4KB_BIT == 0 {
            panic!("smmuv2 does not support 4kb page granule");
        }

        let pasize = bit_extract(glb_rs0.IDR2.get() as usize, 4, 4);
        let ipasize = bit_extract(glb_rs0.IDR2.get() as usize, 0, 4);

        let mut parange = cortex_a::registers::ID_AA64MMFR0_EL1.get() as usize;
        parange = parange & bit_extract(parange, 0, 4);
        if (pasize as isize) < (parange as isize) {
            panic!("smmuv2 does not support the full available pa range")
        }
        if (ipasize as isize) < (parange as isize) {
            panic!("smmuv2 does not support the full available ipa range")
        }
    }

    #[inline]
    fn smr_is_group(&self, smr: usize) -> bool {
        self.group_alloc_bitmap.as_ref().unwrap().get(smr) == 1
    }

    #[inline]
    fn smr_get_context(&self, smr: usize) -> usize {
        bit_extract(
            self.glb_rs0.as_ref().unwrap().S2CR[smr].get() as usize,
            S2CR_CBNDX_OFF,
            S2CR_CBNDX_LEN,
        )
    }

    #[inline]
    fn smr_get_id(&self, smr: usize) -> u16 {
        bit_extract(
            self.glb_rs0.as_ref().unwrap().SMR[smr].get() as usize,
            SMMU_SMR_ID_OFF,
            SMMU_SMR_ID_LEN,
        ) as u16
    }

    #[inline]
    fn smr_get_mask(&self, smr: usize) -> u16 {
        bit_extract(
            self.glb_rs0.as_ref().unwrap().SMR[smr].get() as usize,
            SMMU_SMR_MASK_OFF,
            SMMU_SMR_MASK_LEN,
        ) as u16
    }

    pub fn alloc_smr(&self) -> Option<usize> {
        let alloc_bitmap = self.smr_alloc_bitmap.as_ref().unwrap();
        for i in 0..alloc_bitmap.vec_len() {
            if alloc_bitmap.get(i) == 0 {
                return Some(i);
            }
        }
        None
    }

    pub fn compatible_smr_exists(&mut self, mask: u16, id: u16, context_id: usize, group: bool) -> bool {
        for smr in 0..self.smr_num {
            let bit = self.smr_alloc_bitmap.as_ref().unwrap().get(smr);
            if bit == 0 {
                continue;
            } else {
                let smr_mask = self.smr_get_mask(smr);
                let mask_r = smr_mask & mask;
                let diff_id = (self.smr_get_id(smr) ^ id) & !(mask | smr_mask);
                if diff_id != 0 {
                    if group
                        || (self.smr_is_group(smr) && (mask_r == mask || mask_r == smr_mask))
                        || (context_id == self.smr_get_context(smr))
                    {
                        if mask > smr_mask {
                            self.smr_alloc_bitmap.as_mut().unwrap().set(smr, false);
                        } else {
                            return true;
                        }
                    } else {
                        panic!("SMMU smr conflict");
                    }
                }
            }
        }
        false
    }

    pub fn write_smr(&mut self, smr: usize, mask: u16, id: u16, group: bool) {
        if self.smr_alloc_bitmap.as_ref().unwrap().get(smr) != 0 {
            panic!("smmu: trying to write unallocated smr {}", smr);
        } else {
            let mut val: usize = (mask as usize) << SMMU_SMR_MASK_OFF;
            val |= (id & bit_mask!(SMMU_SMR_ID_OFF, SMMU_SMR_ID_LEN)) as usize;
            val |= SMMUV2_SMR_VALID;
            self.glb_rs0.as_ref().unwrap().SMR[smr].set(val as u32);
            if group {
                self.group_alloc_bitmap.as_mut().unwrap().set(smr, true);
            }
        }
    }

    // Stream-to-Context
    pub fn write_s2c(&mut self, smr: usize, context_id: usize) {
        if self.smr_alloc_bitmap.as_ref().unwrap().get(smr) != 0 {
            panic!("smmu: trying to write unallocated s2c {}", smr);
        } else {
            let mut s2cr: usize = self.glb_rs0.as_ref().unwrap().S2CR[smr].get() as usize;
            s2cr &= bit_mask!(S2CR_IMPL_OFF, S2CR_IMPL_LEN);
            s2cr |= S2CR_DFLT;
            s2cr |= context_id & bit_mask!(S2CR_CBNDX_OFF, S2CR_CBNDX_LEN);

            self.glb_rs0.as_ref().unwrap().S2CR[smr].set(s2cr as u32);
        }
    }

    pub fn alloc_ctxbnk(&mut self) -> Option<usize> {
        let bitmap = match &mut self.context_alloc_bitmap {
            None => panic!("smmu_alloc_ctxbnk: smmu v2 context_alloc_bitmap not init"),
            Some(bitmap) => bitmap,
        };
        for i in self.context_s2_idx..self.context_bank.len() {
            if bitmap.get(i) == 0 {
                bitmap.set(i, true);
                return Some(i);
            }
        }
        println!("smmu_alloc_ctxbnk: cannot alloc ctxbnk");
        None
    }

    pub fn write_ctxbnk(&mut self, context_id: usize, root_pt: usize, vm_id: usize) {
        if self.context_alloc_bitmap.is_none() || self.context_alloc_bitmap.as_ref().unwrap().get(context_id) == 0 {
            panic!("smmu ctx {} not allocated", context_id);
        }
        let rs1 = self.glb_rs1.as_ref().unwrap();
        // Set type as stage 2 only.
        rs1.CBAR[context_id].set((vm_id as u32) & 0xFF);
        rs1.CBA2R[context_id].set(1); // CBA2R_RW64_64BIT

        let ps = 1; // PASize, 36-bit
        let t0sz = 28;
        let tcr = ((ps & 0x7) << SMMUV2_TCR_PS_OFF)
            | (t0sz & 0x1F)
            | SMMUV2_TCR_TG0_4K
            | SMMUV2_TCR_ORGN0_WB_RA_WA
            | SMMUV2_TCR_IRGN0_WB_RA_WA
            | SMMUV2_TCR_SH0_IS
            | SMMUV2_TCR_SL0_1;
        self.context_bank[context_id].TCR.set(tcr as u32);
        self.context_bank[context_id]
            .TTBR0
            .set((root_pt & bit_mask!(12, SMMUV2_CB_TTBA_END - 12)) as u64);
        println!(
            "write smmu cb[{}] TTBR0 {:#x}, vm[{}] root_pt {:#x}",
            context_id,
            self.context_bank[context_id].TTBR0.get(),
            vm_id,
            root_pt
        );
        let mut sctlr = self.context_bank[context_id].SCTLR.get() as usize;
        sctlr = (sctlr) & (0xF << 28 | 0x1 << 20 | 0xF << 9 | 0x1 << 11);
        sctlr |= SMMUV2_SCTLR_CFRE | SMMUV2_SCTLR_CFIE | SMMUV2_SCTLR_M;
        self.context_bank[context_id].SCTLR.set(sctlr as u32);
    }
}

static SMMU_V2: Mutex<SmmuV2> = Mutex::new(SmmuV2::new());

pub fn smmu_init() {
    let mut smmu = SMMU_V2.lock();
    smmu.init();
}

pub fn smmu_vm_init(vm: Vm) -> bool {
    let mut smmu_v2 = SMMU_V2.lock();
    match smmu_v2.alloc_ctxbnk() {
        Some(context_id) => {
            smmu_v2.write_ctxbnk(context_id, vm.pt_dir(), vm.id());
            vm.set_iommu_ctx_id(context_id);
            true
        }
        None => panic!("iommu: smmuv2 could not allocate ctx for vm[{}]", vm.id()),
    }
}

pub fn smmu_add_device(context_id: usize, stream_id: usize) -> bool {
    let mut smmu_v2 = SMMU_V2.lock();
    let prep_id = (stream_id & bit_mask!(SMMU_SMR_ID_OFF, SMMU_SMR_ID_LEN)) as u16;

    if !smmu_v2.compatible_smr_exists(0, prep_id, context_id, false) {
        match smmu_v2.alloc_smr() {
            Some(smr) => {
                smmu_v2.write_smr(smr, 0, prep_id, false);
                smmu_v2.write_s2c(smr, context_id);
            }
            _ => {
                println!("smmu_add_device: smmuv2 no more free sme available.");
                return false;
            }
        }
    }
    true
}

fn emu_smmu_revise_cbar(emu_ctx: &EmuContext) {
    let smmu_v2 = SMMU_V2.lock();

    let cbar_addr = smmu_v2.glb_rs1.as_ref().unwrap().CBAR.as_ptr() as usize;
    let context_id = (emu_ctx.address - (cbar_addr - 0x8_0000_0000)) / size_of::<u32>();
    let vm_context_id = active_vm().unwrap().iommu_ctx_id();
    println!(
        "emu_smmu_revise_cbar: vm {} access context id {}, vm context is {}",
        active_vm_id(),
        context_id,
        vm_context_id
    );

    let mut cbar = SMMUV2_CBAR_TYPE_S1_S2;
    // stage 2 context bank index
    // The SMMUv2 manual suggests that we should use identical VMID for both stages' CBAR
    cbar |= (vm_context_id & 0xFF) << 8;
    cbar |= active_vm_id() & 0xFF;
    smmu_v2.glb_rs1.as_ref().unwrap().CBAR[context_id].set(cbar as u32);
}

pub fn emu_smmu_handler(_emu_dev_id: usize, emu_ctx: &EmuContext) -> bool {
    let address = emu_ctx.address;
    let smmu_v2 = SMMU_V2.lock();

    let mut permit_write = true;
    let cbar = &smmu_v2.glb_rs1.as_ref().unwrap().CBAR;
    if cbar.as_ptr_range().contains(&((address + 0x8_0000_0000) as *const _)) && emu_ctx.write {
        drop(smmu_v2);
        emu_smmu_revise_cbar(emu_ctx);
        return true;
    } else if address + 0x8_0000_0000 >= smmu_v2.context_bank[smmu_v2.context_s2_idx].base_addr {
        // Forbid writing hypervisor's context banks.
        permit_write = false;
    }

    if !emu_ctx.write {
        let val = if emu_ctx.width > 4 {
            unsafe { ptr::read_volatile((address + 0x8_0000_0000) as *const usize) }
        } else {
            unsafe { ptr::read_volatile((address + 0x8_0000_0000) as *const u32) as usize }
        };
        current_cpu().set_gpr(emu_ctx.reg, val);
    } else {
        let val = current_cpu().get_gpr(emu_ctx.reg);
        if permit_write {
            if emu_ctx.width > 4 {
                unsafe { ptr::write_volatile((address + 0x8_0000_0000) as *mut usize, val) };
            } else {
                unsafe { ptr::write_volatile((address + 0x8_0000_0000) as *mut u32, val as u32) };
            };
        } else {
            println!(
                "emu_smmu_handler: vm {} is not allowed to access context[{}]",
                active_vm_id(),
                (address - (smmu_v2.context_bank[0].base_addr as usize - 0x8_0000_0000)) / 0x10000,
            );
        }
    }

    true
}
