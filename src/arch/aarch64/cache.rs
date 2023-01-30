use core::fmt::Display;
use core::arch::asm;

use crate::{
    device::{EmuContext, emu_register_reg, EmuRegType},
    kernel::current_cpu,
    arch::{CacheInfoTrait, CacheType, CacheIndexed, cache},
};
use crate::sysreg_encode_addr;
use alloc::vec::Vec;
use cache::CpuCacheInfo;
use cortex_a::registers::{CLIDR_EL1, CCSIDR_EL1, CSSELR_EL1, ID_AA64MMFR2_EL1};
use spin::{Mutex, Once};
use tock_registers::interfaces::{Readable, Writeable};

use super::PAGE_SIZE;

// pub static LAST_LEVEL_CACHE: Once<Mutex<Aarch64CacheInfo>> = Once::new();

pub static CPU_CACHE: Once<Mutex<CpuCacheInfo<Aarch64CacheInfo>>> = Once::new();

#[derive(Copy, Clone)]
pub struct Aarch64CacheInfo {
    level: usize,
    size: usize,

    num_sets: usize,

    cache_type: CacheType,

    indexed: CacheIndexed,

    associativity: usize, // a.k.a `ways`

    line_size: usize,

    // CCIDX feature, from ID_AA64MMFR2_EL1.read(ID_AA64MMFR2_EL1::CCIDX)
    has_ccidx: bool,
}

const MAX_CACHE_LEVEL: usize = 7;

impl Aarch64CacheInfo {
    fn new(
        level: usize,
        num_sets: usize,
        associativity: usize,
        line_size: usize,
        cache_type: CacheType,
        indexed: CacheIndexed,
        has_ccidx: bool,
    ) -> Self {
        let size = num_sets * associativity * line_size;
        Self {
            level,
            size,
            num_sets,
            associativity,
            line_size,
            cache_type,
            indexed,
            has_ccidx,
        }
    }

    #[inline]
    fn ctype(level: usize) -> usize {
        ((CLIDR_EL1.get() >> (3 * (level - 1))) & 0b111) as usize
    }

    #[inline]
    fn set_cache_level(level: u64) {
        CSSELR_EL1.write(CSSELR_EL1::Level.val(level - 1));
    }

    #[inline]
    fn get_cache_level() -> u64 {
        CSSELR_EL1.read(CSSELR_EL1::Level) + 1
    }
}

#[warn(unused_doc_comments)]
impl CacheInfoTrait for Aarch64CacheInfo {
    fn get_cache_info(level: usize) -> Self {
        let has_ccidx = ID_AA64MMFR2_EL1.read(ID_AA64MMFR2_EL1::CCIDX) != 0;

        Self::set_cache_level(level as u64);
        // (Number of sets in cache) - 1, therefore a value of 0 indicates 1 set in the cache.
        // The number of sets does not have to be a power of 2.
        let num_sets = (CCSIDR_EL1.get_num_sets() + 1) as usize;

        // (Associativity of cache) - 1, therefore a value of 0 indicates an associativity of 1.
        // The associativity does not have to be a power of 2.
        let associativity = (CCSIDR_EL1.get_associativity() + 1) as usize;

        // (Log2(Number of bytes in cache line)) - 4. For example:
        // For a line length of 16 bytes: Log2(16) = 4, LineSize entry = 0. This is the minimum line length.
        // For a line length of 32 bytes: Log2(32) = 5, LineSize entry = 1.
        let line_size = 1 << (CCSIDR_EL1.read(CCSIDR_EL1::LineSize) + 4);

        let cache_type = match Self::ctype(level) {
            0b001 => CacheType::Instruction,
            0b010 => CacheType::Data,
            0b011 => CacheType::Separate,
            0b100 => CacheType::Unified,
            _ => CacheType::NoCache,
        };

        let indexed = if level == 1 {
            const CTR_L1LP_OFF: usize = 14;
            const CTR_L1LP_LEN: usize = 2;
            const CTR_L1LP_VPIPT: usize = 0b00 << CTR_L1LP_OFF;
            const CTR_L1LP_AIVIVT: usize = 0b01 << CTR_L1LP_OFF;
            const CTR_L1LP_VIPT: usize = 0b10 << CTR_L1LP_OFF;
            const CTR_L1LP_PIPT: usize = 0b11 << CTR_L1LP_OFF;
            const CTR_L1LP_MASK: usize = (1 << CTR_L1LP_LEN - 1) << CTR_L1LP_OFF;

            let mut ctr: usize;
            unsafe {
                asm!("mrs {0}, CTR_EL0", out(reg) ctr);
            }
            if ctr & CTR_L1LP_MASK == CTR_L1LP_PIPT {
                CacheIndexed::PIPT
            } else {
                CacheIndexed::VIPT
            }
        } else {
            CacheIndexed::PIPT
        };

        Self::new(
            level,
            num_sets,
            associativity,
            line_size,
            cache_type,
            indexed,
            has_ccidx,
        )
    }

    #[inline]
    fn num_colors(&self) -> usize {
        self.size / (self.associativity * PAGE_SIZE)
    }

    #[inline]
    fn level(&self) -> usize {
        self.level
    }

    #[inline]
    fn num_sets(&self) -> usize {
        self.num_sets
    }

    #[inline]
    fn size(&self) -> usize {
        self.size
    }

    #[inline]
    fn ways(&self) -> usize {
        self.associativity
    }

    #[inline]
    fn line_size(&self) -> usize {
        self.line_size
    }

    fn init_cache_level() -> usize {
        let mut level = 1; // same with reg definition
        while level < MAX_CACHE_LEVEL {
            if Self::ctype(level) == 0b000 {
                break;
            }
            level += 1;
        }
        level - 1
    }
}

impl Display for Aarch64CacheInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let units = ["", "K", "M"];
        let mut size = self.size;
        let mut index = 0;
        while index < units.len() {
            if size >> 10 <= 0 {
                break;
            }
            size >>= 10;
            index += 1;
        }
        let unit = units[index];
        write!(
            f,
            "L{} cache: {}{}B, line size {}B, {} associativity, {} num_sets, {} colors",
            self.level,
            size,
            unit,
            self.line_size,
            self.associativity,
            self.num_sets,
            self.num_colors()
        )
    }
}

pub fn cache_init() {
    let num_levels = Aarch64CacheInfo::init_cache_level();

    let mut info_list = Vec::new();
    let mut min_share_level = 0;
    // TODO
    let mut _num_leaves = 0;

    let mut first_unified = false;

    for i in 1..=num_levels {
        let cache_info = Aarch64CacheInfo::get_cache_info(i);
        info_list.push(cache_info);
        if cache_info.cache_type == CacheType::Unified && first_unified == false {
            first_unified = true;
            min_share_level = i;
        }
        println!("{}", cache_info);
    }

    CPU_CACHE.call_once(|| {
        Mutex::new(CpuCacheInfo {
            info_list,
            min_share_level,
            num_levels,
            _num_leaves,
        })
    });

    // registration
    const CCSIDR_EL1_ADDR: usize = sysreg_encode_addr!(0b11, 0b001, 0b0000, 0b0000, 0b000);
    const CLIDR_EL1_ADDR: usize = sysreg_encode_addr!(0b11, 0b001, 0b0000, 0b0000, 0b001);
    const CSSELR_EL1_ADDR: usize = sysreg_encode_addr!(0b11, 0b010, 0b0000, 0b0000, 0b000);
    const CTR_EL0_ADDR: usize = sysreg_encode_addr!(0b11, 0b011, 0b0000, 0b0000, 0b001);
    emu_register_reg(EmuRegType::SysReg, CCSIDR_EL1_ADDR, vcache_ccsidr_el1_handler);
    emu_register_reg(EmuRegType::SysReg, CLIDR_EL1_ADDR, vcache_clidr_el1_handler);
    emu_register_reg(EmuRegType::SysReg, CSSELR_EL1_ADDR, vcache_csselr_el1_handler);
    emu_register_reg(EmuRegType::SysReg, CTR_EL0_ADDR, vcache_ctr_el0_handler);
}

/// Current Cache Size ID Register
pub fn vcache_ccsidr_el1_handler(_id: usize, emu_ctx: &EmuContext) -> bool {
    match emu_ctx.write {
        true => {
            println!("Core{} cannot write CCSIDR_EL1", current_cpu().id);
            false
        }
        false => {
            let last_level = CPU_CACHE.get().unwrap().lock().min_share_level as u64;

            let val = if Aarch64CacheInfo::get_cache_level() != last_level {
                CCSIDR_EL1.get()
            } else {
                todo!("need to give L{} cache info of VM", last_level);
            };
            current_cpu().set_gpr(emu_ctx.reg, val as usize);

            println!(
                "Core{} {} CCSIDR_EL1 with x{}={:#x}",
                current_cpu().id,
                if emu_ctx.write { "write" } else { "read" },
                emu_ctx.reg,
                val
            );
            true
        }
    }
}

/// Cache Level ID Register
/// no more operation
pub fn vcache_clidr_el1_handler(_id: usize, emu_ctx: &EmuContext) -> bool {
    match emu_ctx.write {
        true => {
            println!("Core{} cannot write CLIDR_EL1", current_cpu().id);
            false
        }
        false => {
            let val = CLIDR_EL1.get();
            current_cpu().set_gpr(emu_ctx.reg, val as usize);

            println!(
                "Core{} {} CLIDR_EL1 with x{}={:#x}",
                current_cpu().id,
                if emu_ctx.write { "write" } else { "read" },
                emu_ctx.reg,
                val
            );
            true
        }
    }
}

/// Cache Size Selection Register
/// no more operation
pub fn vcache_csselr_el1_handler(_id: usize, emu_ctx: &EmuContext) -> bool {
    match emu_ctx.write {
        true => {
            let val = current_cpu().get_gpr(emu_ctx.reg);
            CSSELR_EL1.set(val as u64);
            println!(
                "Core{} {} CSSELR_EL1 with x{}={:#x}",
                current_cpu().id,
                if emu_ctx.write { "write" } else { "read" },
                emu_ctx.reg,
                val
            );
        }
        false => {
            let val = CSSELR_EL1.get();
            current_cpu().set_gpr(emu_ctx.reg, val as usize);

            println!(
                "Core{} {} CSSELR_EL1 with x{}={:#x}",
                current_cpu().id,
                if emu_ctx.write { "write" } else { "read" },
                emu_ctx.reg,
                val
            );
        }
    }
    true
}

/// Cache Type Register
/// no more operation
pub fn vcache_ctr_el0_handler(_id: usize, emu_ctx: &EmuContext) -> bool {
    // unsafe {
    //     static mut times: usize = 0;
    //     times += 1;
    //     if times % 100 == 0 {
    //         println!(
    //             "Core{} {} CTR_EL0, times {}",
    //             current_cpu().id,
    //             if emu_ctx.write { "write" } else { "read" },
    //             times
    //         );
    //     }
    // }
    match emu_ctx.write {
        true => {
            println!("Core{} cannot write CTR_EL0", current_cpu().id);
            false
        }
        false => {
            let mut val: usize;
            unsafe {
                asm!("mrs {0}, CTR_EL0", out(reg) val);
            }
            current_cpu().set_gpr(emu_ctx.reg, val);
            true
        }
    }
}

// // ec=0x18
// pub enum EmuRegType {
//     IdRegs(EmuIdReg),
// }

// // control by HCR_EL2.{TID0, TID1, TID2, TID3}
// // Traps to EL2 of EL0 and EL1 accesses to the ID registers
// pub enum EmuIdReg {
//     CacheIdRegs(EmuCacheIdReg),
// }

// #[allow(non_camel_case_types)]
// // control by HCR_EL2.TID2
// // Traps accesses to cache identification registers at EL1 and EL0 to EL2
// pub enum EmuCacheIdReg {
//     CTR_EL0,
//     CCSIDR_EL1,
//     CCSIDR2_EL1,
//     CLIDR_EL1,
//     CSSELR_EL1,
// }
