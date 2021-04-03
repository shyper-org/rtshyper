use super::{PAGE_SIZE, PTE_PER_PAGE};
use crate::kernel::{Cpu, CpuPt};
use rlibc::{memcpy, memset};

// page_table const
pub const LVL1_SHIFT: usize = 30;
pub const LVL2_SHIFT: usize = 21;
pub const LVL3_SHIFT: usize = 12;

pub const PTE_TABLE: usize = 0b11;
pub const PTE_PAGE: usize = 0b11;
pub const PTE_BLOCK: usize = 0b01;

pub const PTE_S1_FIELD_AP_RW_EL0_NONE: usize = 0b00 << 6;
pub const PTE_S1_FIELD_AP_RW_EL0_RW: usize = 0b01 << 6;
pub const PTE_S1_FIELD_AP_R0_EL0_NONE: usize = 0b10 << 6;
pub const PTE_S1_FIELD_AP_R0_EL0_RW: usize = 0b11 << 6;

pub const PTE_S1_FIELD_SH_NON_SHAREABLE: usize = 0b00 << 8;
pub const PTE_S1_FIELD_SH_RESERVED: usize = 0b01 << 8;
pub const PTE_S1_FIELD_SH_OUTER_SHAREABLE: usize = 0b10 << 8;
pub const PTE_S1_FIELD_SH_INNER_SHAREABLE: usize = 0b11 << 8;

pub const PTE_S1_FIELD_AF: usize = 1 << 10;

pub const PTE_S1_NORMAL: usize = pte_s1_field_attr_indx(1)
    | PTE_S1_FIELD_AP_RW_EL0_NONE
    | PTE_S1_FIELD_SH_OUTER_SHAREABLE
    | PTE_S1_FIELD_AF;

pub const CPU_BANKED_ADDRESS: usize = 0x400000000;

pub const fn pte_s1_field_attr_indx(idx: usize) -> usize {
    idx << 2
}

// page_table function
pub fn pt_lvl1_idx(va: usize) -> usize {
    (va >> LVL1_SHIFT) & (PTE_PER_PAGE - 1)
}
pub fn pt_lvl2_idx(va: usize) -> usize {
    (va >> LVL2_SHIFT) & (PTE_PER_PAGE - 1)
}
pub fn pt_lvl3_idx(va: usize) -> usize {
    (va >> LVL3_SHIFT) & (PTE_PER_PAGE - 1)
}

pub fn pt_map_banked_cpu(cpu: &mut Cpu) -> usize {
    extern "C" {
        fn lvl1_page_table();
    }
    let addr: usize = lvl1_page_table as usize;

    unsafe {
        memcpy(
            &(cpu.cpu_pt.lvl1) as *const _ as *mut u8,
            addr as *mut u8,
            PAGE_SIZE,
        );
        memset(&(cpu.cpu_pt.lvl2) as *const _ as *mut u8, 0, PAGE_SIZE);
        memset(&(cpu.cpu_pt.lvl3) as *const _ as *mut u8, 0, PAGE_SIZE);
    }

    let cpu_addr = cpu as *const _ as usize;
    let lvl2_addr = &(cpu.cpu_pt.lvl2) as *const _ as usize;
    let lvl3_addr = &(cpu.cpu_pt.lvl3) as *const _ as usize;
    cpu.cpu_pt.lvl1[pt_lvl1_idx(CPU_BANKED_ADDRESS)] = lvl2_addr | PTE_S1_NORMAL | PTE_TABLE;
    cpu.cpu_pt.lvl2[pt_lvl2_idx(CPU_BANKED_ADDRESS)] = lvl3_addr | PTE_S1_NORMAL | PTE_TABLE;

    use crate::lib::round_up;
    use core::mem::size_of;
    let page_num = round_up(size_of::<Cpu>(), PAGE_SIZE) / PAGE_SIZE;

    // println!("cpu page num is {}", page_num);
    for i in 0..page_num {
        cpu.cpu_pt.lvl3[pt_lvl3_idx(CPU_BANKED_ADDRESS) + i] =
            (cpu_addr + i * PAGE_SIZE) | PTE_S1_NORMAL | PTE_PAGE;
    }

    // println!("cpu addr {:x}", cpu_addr);
    // println!("lvl2 addr {:x}", lvl2_addr);
    // println!("lvl3 addr {:x}", lvl3_addr);

    &(cpu.cpu_pt.lvl1) as *const _ as usize
}
