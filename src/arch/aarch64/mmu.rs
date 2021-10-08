global_asm!(include_str!("start.S"));

use super::interface::*;
use tock_registers::*;
use tock_registers::interfaces::*;

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
    const fn invalid() -> BlockDescriptor {
        BlockDescriptor(0)
    }
}

#[repr(C)]
#[repr(align(4096))]
pub struct PageTables {
    lvl1: [BlockDescriptor; ENTRY_PER_PAGE],
}

use crate::board::PLAT_DESC;
const LVL1_SHIFT: usize = 30;
const PLATFORM_PHYSICAL_LIMIT_GB: usize = 16;

#[no_mangle]
// #[link_section = ".text.boot"]
pub unsafe extern "C" fn pt_populate(pt: &mut PageTables) {
    for i in 0..PLATFORM_PHYSICAL_LIMIT_GB {
        let output_addr = i << LVL1_SHIFT;
        pt.lvl1[i] = if output_addr >= PLAT_DESC.mem_desc.base {
            BlockDescriptor::new(output_addr, false)
        } else {
            BlockDescriptor::new(output_addr, true)
        }
    }
    // pt.lvl1[0] = BlockDescriptor::new(0, true);
    // pt.lvl1[1] = BlockDescriptor::new(0x40000000, false);
    // pt.lvl1[2] = BlockDescriptor::new(0x80000000, false);
    for i in PLATFORM_PHYSICAL_LIMIT_GB..ENTRY_PER_PAGE {
        pt.lvl1[i] = BlockDescriptor::invalid();
    }
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
