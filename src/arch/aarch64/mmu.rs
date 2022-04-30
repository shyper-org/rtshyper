use core::arch::global_asm;

use tock_registers::*;
use tock_registers::interfaces::*;

use crate::arch::pt_lvl2_idx;
use crate::board::PLAT_DESC;
use crate::lib::memset_safe;

use super::interface::*;

#[cfg(not(feature = "update"))]
global_asm!(include_str!("start.S"));
#[cfg(feature = "update")]
global_asm!(include_str!("start_update.S"));

// const PHYSICAL_ADDRESS_LIMIT_GB: usize = BOARD_PHYSICAL_ADDRESS_LIMIT >> 30;
// const PAGE_SIZE: usize = 4096;
// const PAGE_SHIFT: usize = 12;
// const ENTRY_PER_PAGE: usize = PAGE_SIZE / 8;

register_bitfields! {u64,
    pub TableDescriptor [
        NEXT_LEVEL_TABLE_PPN OFFSET(12) NUMBITS(36) [], // [47:12]
        TYPE  OFFSET(1) NUMBITS(1) [
            Block = 0,
            Table = 1
        ],
        VALID OFFSET(0) NUMBITS(1) [
            False = 0,
            True = 1
        ]
    ]
}

register_bitfields! {u64,
    pub PageDescriptorS1 [
        UXN      OFFSET(54) NUMBITS(1) [
            False = 0,
            True = 1
        ],
        PXN      OFFSET(53) NUMBITS(1) [
            False = 0,
            True = 1
        ],
        OUTPUT_PPN OFFSET(12) NUMBITS(36) [], // [47:12]
        AF       OFFSET(10) NUMBITS(1) [
            False = 0,
            True = 1
        ],
        SH       OFFSET(8) NUMBITS(2) [
            OuterShareable = 0b10,
            InnerShareable = 0b11
        ],
        AP       OFFSET(6) NUMBITS(2) [
            RW_ELx = 0b00,
            RW_ELx_EL0 = 0b01,
            RO_ELx = 0b10,
            RO_ELx_EL0 = 0b11
        ],
        AttrIndx OFFSET(2) NUMBITS(3) [
            Attr0 = 0b000,
            Attr1 = 0b001,
            Attr2 = 0b010
        ],
        TYPE     OFFSET(1) NUMBITS(1) [
            Block = 0,
            Table = 1
        ],
        VALID    OFFSET(0) NUMBITS(1) [
            False = 0,
            True = 1
        ]
    ]
}

#[derive(Copy, Clone)]
#[repr(transparent)]
struct BlockDescriptor(u64);

impl BlockDescriptor {
    fn new(output_addr: usize, device: bool) -> BlockDescriptor {
        BlockDescriptor(
            (PageDescriptorS1::OUTPUT_PPN.val((output_addr >> PAGE_SHIFT) as u64)
                + PageDescriptorS1::AF::True
                + PageDescriptorS1::AP::RW_ELx
                + PageDescriptorS1::TYPE::Block
                + PageDescriptorS1::VALID::True
                + if device {
                    PageDescriptorS1::AttrIndx::Attr0 + PageDescriptorS1::SH::OuterShareable
                } else {
                    PageDescriptorS1::AttrIndx::Attr1 + PageDescriptorS1::SH::OuterShareable
                })
            .value,
        )
    }

    fn table(output_addr: usize) -> BlockDescriptor {
        BlockDescriptor(
            (PageDescriptorS1::OUTPUT_PPN.val((output_addr >> PAGE_SHIFT) as u64)
                + PageDescriptorS1::VALID::True
                + PageDescriptorS1::TYPE::Table)
                .value,
        )
    }
    const fn invalid() -> BlockDescriptor {
        BlockDescriptor(0)
    }
}

#[repr(C)]
#[repr(align(4096))]
pub struct PageTables {
    lvl1: [BlockDescriptor; ENTRY_PER_PAGE],
}

const LVL1_SHIFT: usize = 30;
const PLATFORM_PHYSICAL_LIMIT_GB: usize = 16;

#[no_mangle]
// #[link_section = ".text.boot"]
pub unsafe extern "C" fn pt_populate(lvl1_pt: &mut PageTables, lvl2_pt: &mut PageTables) {
    let lvl1_base = lvl1_pt as *const _ as usize;
    let lvl2_base = lvl2_pt as *const _ as usize;
    memset_safe(lvl1_base as *mut u8, 0, PAGE_SIZE);
    memset_safe(lvl2_base as *mut u8, 0, PAGE_SIZE);

    for i in 0..PLATFORM_PHYSICAL_LIMIT_GB {
        let output_addr = i << LVL1_SHIFT;
        lvl1_pt.lvl1[i] = if output_addr >= PLAT_DESC.mem_desc.base {
            BlockDescriptor::new(output_addr, false)
        } else {
            BlockDescriptor::invalid()
        }
    }
    // for i in PLATFORM_PHYSICAL_LIMIT_GB..ENTRY_PER_PAGE {
    //     pt.lvl1[i] = BlockDescriptor::invalid();
    // }

    lvl1_pt.lvl1[32] = BlockDescriptor::table(lvl2_base);
    // 0x200000 ~ 2MB
    // UART0 ~ 0x3000000 - 0x3200000 (0x3100000)
    // UART1 ~ 0xc200000 - 0xc400000 (0xc280000)
    // EMMC ~ 0x3400000 - 0x3600000 (0x3460000)
    // GIC  ~ 0x3800000 - 0x3a00000 (0x3881000)
    // SMMU ~ 0x12000000 - 0x13000000
    lvl2_pt.lvl1[pt_lvl2_idx(0x3000000)] = BlockDescriptor::new(0x3000000, true);
    lvl2_pt.lvl1[pt_lvl2_idx(0xc200000)] = BlockDescriptor::new(0xc200000, true);
    // lvl2_pt.lvl1[pt_lvl2_idx(0x3400000)] = BlockDescriptor::new(0x3400000, true);
    lvl2_pt.lvl1[pt_lvl2_idx(0x3800000)] = BlockDescriptor::new(0x3800000, true);
    // for i in 0..8 {
    //     let addr = 0x12000000 + i * 0x200000;
    //     lvl2_pt.lvl1[pt_lvl2_idx(addr) + 32] = BlockDescriptor::new(addr, true);
    // }
}

#[no_mangle]
// #[link_section = ".text.boot"]
pub unsafe extern "C" fn mmu_init(pt: &PageTables) {
    use cortex_a::registers::*;
    MAIR_EL2.write(
        MAIR_EL2::Attr0_Device::nonGathering_nonReordering_noEarlyWriteAck
            + MAIR_EL2::Attr1_Normal_Outer::WriteBack_NonTransient_ReadWriteAlloc
            + MAIR_EL2::Attr1_Normal_Inner::WriteBack_NonTransient_ReadWriteAlloc
            + MAIR_EL2::Attr2_Normal_Outer::NonCacheable
            + MAIR_EL2::Attr2_Normal_Inner::NonCacheable,
    );
    TTBR0_EL2.set(&pt.lvl1 as *const _ as u64);

    TCR_EL2.write(
        TCR_EL2::PS::Bits_48
            + TCR_EL2::SH0::Inner
            + TCR_EL2::TG0::KiB_4
            + TCR_EL2::ORGN0::WriteBack_ReadAlloc_WriteAlloc_Cacheable
            + TCR_EL2::IRGN0::WriteBack_ReadAlloc_WriteAlloc_Cacheable
            + TCR_EL2::T0SZ.val(64 - 39),
    );

    // barrier::isb(barrier::SY);
    // SCTLR_EL2.modify(SCTLR_EL2::M::Enable + SCTLR_EL2::C::Cacheable + SCTLR_EL2::I::Cacheable);
    // barrier::isb(barrier::SY);
}
