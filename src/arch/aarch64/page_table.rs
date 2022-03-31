use alloc::vec::Vec;

// use rlibc::{memcpy, memset};
use spin::Mutex;

use crate::arch::ArchPageTableEntryTrait;
use crate::arch::WORD_SIZE;
use crate::kernel::Cpu;
use crate::lib::{memcpy_safe, memset_safe};
use crate::lib::round_up;
use crate::mm::PageFrame;

use super::{PAGE_SIZE, PTE_PER_PAGE};

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

pub const PTE_S2_FIELD_MEM_ATTR_DEVICE_NGNRNE: usize = 0;

pub const PTE_S2_FIELD_MEM_ATTR_NORMAL_OUTER_WRITE_BACK_CACHEABLE: usize = 0b11 << 4;

pub const PTE_S2_FIELD_MEM_ATTR_NORMAL_INNER_WRITE_BACK_CACHEABLE: usize = 0b11 << 2;

pub const PTE_S2_FIELD_AP_NONE: usize = 0b00 << 6;
pub const PTE_S2_FIELD_AP_RO: usize = 0b01 << 6;
pub const PTE_S2_FIELD_AP_WO: usize = 0b10 << 6;
pub const PTE_S2_FIELD_AP_RW: usize = 0b11 << 6;

pub const PTE_S2_FIELD_SH_NON_SHAREABLE: usize = 0b00 << 8;
pub const PTE_S2_FIELD_SH_RESERVED: usize = 0b01 << 8;
pub const PTE_S2_FIELD_SH_OUTER_SHAREABLE: usize = 0b10 << 8;
pub const PTE_S2_FIELD_SH_INNER_SHAREABLE: usize = 0b11 << 8;

pub const PTE_S2_FIELD_AF: usize = 1 << 10;

pub const PTE_S1_NORMAL: usize =
    pte_s1_field_attr_indx(1) | PTE_S1_FIELD_AP_RW_EL0_NONE | PTE_S1_FIELD_SH_OUTER_SHAREABLE | PTE_S1_FIELD_AF;

pub const PTE_S2_DEVICE: usize =
    PTE_S2_FIELD_MEM_ATTR_DEVICE_NGNRNE | PTE_S2_FIELD_AP_RW | PTE_S2_FIELD_SH_OUTER_SHAREABLE | PTE_S2_FIELD_AF;

pub const PTE_S2_NORMAL: usize = PTE_S2_FIELD_MEM_ATTR_NORMAL_INNER_WRITE_BACK_CACHEABLE
    | PTE_S2_FIELD_MEM_ATTR_NORMAL_OUTER_WRITE_BACK_CACHEABLE
    | PTE_S2_FIELD_AP_RW
    | PTE_S2_FIELD_SH_OUTER_SHAREABLE
    | PTE_S2_FIELD_AF;

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

    memcpy_safe(&(cpu.cpu_pt.lvl1) as *const _ as *mut u8, addr as *mut u8, PAGE_SIZE);
    memset_safe(&(cpu.cpu_pt.lvl2) as *const _ as *mut u8, 0, PAGE_SIZE);
    memset_safe(&(cpu.cpu_pt.lvl3) as *const _ as *mut u8, 0, PAGE_SIZE);

    let cpu_addr = cpu as *const _ as usize;
    let lvl2_addr = &(cpu.cpu_pt.lvl2) as *const _ as usize;
    let lvl3_addr = &(cpu.cpu_pt.lvl3) as *const _ as usize;
    cpu.cpu_pt.lvl1[pt_lvl1_idx(CPU_BANKED_ADDRESS)] = lvl2_addr | PTE_S1_NORMAL | PTE_TABLE;
    cpu.cpu_pt.lvl2[pt_lvl2_idx(CPU_BANKED_ADDRESS)] = lvl3_addr | PTE_S1_NORMAL | PTE_TABLE;

    use core::mem::size_of;
    let page_num = round_up(size_of::<Cpu>(), PAGE_SIZE) / PAGE_SIZE;

    // println!("cpu page num is {}", page_num);
    for i in 0..page_num {
        cpu.cpu_pt.lvl3[pt_lvl3_idx(CPU_BANKED_ADDRESS) + i] = (cpu_addr + i * PAGE_SIZE) | PTE_S1_NORMAL | PTE_PAGE;
    }

    // println!("cpu addr {:x}", cpu_addr);
    // println!("lvl2 addr {:x}", lvl2_addr);
    // println!("lvl3 addr {:x}", lvl3_addr);

    &(cpu.cpu_pt.lvl1) as *const _ as usize
}

#[repr(transparent)]
#[derive(Copy, Clone, Debug)]
pub struct Aarch64PageTableEntry(usize);

impl ArchPageTableEntryTrait for Aarch64PageTableEntry {
    fn from_pte(value: usize) -> Self {
        Aarch64PageTableEntry(value)
    }

    fn from_pa(pa: usize) -> Self {
        Aarch64PageTableEntry(pa)
    }

    fn to_pte(&self) -> usize {
        self.0
    }

    fn to_pa(&self) -> usize {
        self.0 & 0x0000_FFFF_FFFF_F000
    }

    fn valid(&self) -> bool {
        self.0 & 0b11 != 0
    }

    fn entry(&self, index: usize) -> Aarch64PageTableEntry {
        let addr = self.to_pa() + index * WORD_SIZE;
        unsafe { Aarch64PageTableEntry((addr as *const usize).read_volatile()) }
    }

    fn set_entry(&self, index: usize, value: Aarch64PageTableEntry) {
        let addr = self.to_pa() + index * WORD_SIZE;
        unsafe { (addr as *mut usize).write_volatile(value.0) }
    }

    fn make_table(frame_pa: usize) -> Self {
        Aarch64PageTableEntry::from_pa(frame_pa | PTE_TABLE)
    }
}

pub struct PageTable {
    directory: PageFrame,
    pages: Mutex<Vec<PageFrame>>,
}

impl PageTable {
    pub fn new(directory: PageFrame) -> PageTable {
        PageTable {
            directory,
            pages: Mutex::new(Vec::new()),
        }
    }

    pub fn base_pa(&self) -> usize {
        self.directory.pa()
    }

    pub fn read_only(&self, start: usize, len: usize) {
        let directory = Aarch64PageTableEntry::from_pa(self.directory.pa());
        let mut ipa = start;
        while ipa < (start + len) {
            let mut l1e = directory.entry(pt_lvl1_idx(ipa));
            if !l1e.valid() {
                ipa += 512 * 512 * 4096; // 1GB: 9 + 9 + 12 bits
                continue;
            }
            let l2e = l1e.entry(pt_lvl2_idx(ipa));
            if !l2e.valid() {
                ipa += 512 * 4096; // 2MB: 9 + 12 bits
                continue;
            } else if l2e.to_pte() & PTE_BLOCK != 0 {
                let pte = l2e.to_pte() & !(0b11 << 6) | PTE_S2_FIELD_AP_RO;
                l1e.set_entry(pt_lvl2_idx(ipa), Aarch64PageTableEntry::from_pa(pte));
                ipa += 512 * 4096; // 2MB: 9 + 12 bits
                continue;
            }
            let l3e = l2e.entry(pt_lvl3_idx(ipa));
            if l3e.valid() {
                let pte = l3e.to_pte() & !(0b11 << 6) | PTE_S2_FIELD_AP_RO;
                l2e.set_entry(pt_lvl3_idx(ipa), Aarch64PageTableEntry::from_pa(pte))
            }
            ipa += 4096; // 4KB: 12 bits
        }
    }

    pub fn map_2mb(&self, ipa: usize, pa: usize, pte: usize) {
        let directory = Aarch64PageTableEntry::from_pa(self.directory.pa());
        let mut l1e = directory.entry(pt_lvl1_idx(ipa));
        if !l1e.valid() {
            let result = crate::kernel::mem_page_alloc();
            if let Ok(frame) = result {
                l1e = Aarch64PageTableEntry::make_table(frame.pa());
                let mut pages = self.pages.lock();
                pages.push(frame);
                directory.set_entry(pt_lvl1_idx(ipa), l1e);
            } else {
                println!("map lv1 page failed");
                return;
            }
        }

        let l2e = l1e.entry(pt_lvl2_idx(ipa));
        if l2e.valid() {
            println!("map_2mb lvl 2 already mapped with 0x{:x}", l2e.to_pte());
        } else {
            l1e.set_entry(pt_lvl2_idx(ipa), Aarch64PageTableEntry::from_pa(pa | pte | PTE_BLOCK));
        }
    }

    pub fn map(&self, ipa: usize, pa: usize, pte: usize) {
        let directory = Aarch64PageTableEntry::from_pa(self.directory.pa());
        let mut l1e = directory.entry(pt_lvl1_idx(ipa));
        if !l1e.valid() {
            let result = crate::kernel::mem_page_alloc();
            if let Ok(frame) = result {
                l1e = Aarch64PageTableEntry::make_table(frame.pa());
                let mut pages = self.pages.lock();
                pages.push(frame);
                directory.set_entry(pt_lvl1_idx(ipa), l1e);
            } else {
                println!("map lv1 page failed");
                return;
            }
        }

        let mut l2e = l1e.entry(pt_lvl2_idx(ipa));
        if !l2e.valid() {
            let result = crate::kernel::mem_page_alloc();
            if let Ok(frame) = result {
                l2e = Aarch64PageTableEntry::make_table(frame.pa());
                let mut pages = self.pages.lock();
                pages.push(frame);
                l1e.set_entry(pt_lvl2_idx(ipa), l2e);
            } else {
                println!("map lv2 page failed");
                return;
            }
        }
        let l3e = l2e.entry(pt_lvl3_idx(ipa));
        if l3e.valid() {
            println!("map lvl 3 already mapped with 0x{:x}", l3e.to_pte());
        } else {
            l2e.set_entry(pt_lvl3_idx(ipa), Aarch64PageTableEntry::from_pa(pa | PTE_TABLE | pte));
        }
    }

    pub fn map_range_2mb(&self, ipa: usize, len: usize, pa: usize, pte: usize) {
        let size_2mb = 1 << LVL2_SHIFT;
        let page_num = round_up(len, size_2mb) / size_2mb;
        // println!(
        //     "map_range_2mb: ipa {:x}, len {:x}, pa {:x}, pte 0b{:b}, page_num {:x}, size_2mb {:x}",
        //     ipa, len, pa, pte, page_num, size_2mb
        // );

        for i in 0..page_num {
            self.map_2mb(ipa + i * size_2mb, pa + i * size_2mb, pte);
        }
    }

    pub fn map_range(&self, ipa: usize, len: usize, pa: usize, pte: usize) {
        let page_num = round_up(len, PAGE_SIZE) / PAGE_SIZE;
        // if ipa == 0x8010000 {
        //     println!(
        //         "map_range: ipa {:x}, len {:x}, pa {:x}, pte 0b{:b}, page_num {:x}",
        //         ipa, len, pa, pte, page_num,
        //     );
        // }
        for i in 0..page_num {
            self.map(ipa + i * PAGE_SIZE, pa + i * PAGE_SIZE, pte);
        }
    }

    pub fn show_pt(&self, ipa: usize) {
        // println!("show_pt");
        let directory = Aarch64PageTableEntry::from_pa(self.directory.pa());
        println!("1 {:x}", directory.to_pa());
        let l1e = directory.entry(pt_lvl1_idx(ipa));
        println!("2 {:x}", l1e.to_pa());
        let l2e = l1e.entry(pt_lvl2_idx(ipa));
        println!("3 {:x}", l2e.to_pa());
        if l2e.to_pte() & 0b11 == PTE_BLOCK {
            println!("ipa {:x} to pa {:x}", ipa, l2e.to_pa());
        } else {
            let l3e = l2e.entry(pt_lvl3_idx(ipa));
            println!("ipa {:x} to pa {:x}", ipa, l3e.to_pa());
        }
    }

    pub fn pt_map_range(&self, ipa: usize, len: usize, pa: usize, pte: usize) {
        let size_2mb = 1 << LVL2_SHIFT;
        if ipa % size_2mb == 0 && len % size_2mb == 0 && pa % size_2mb == 0 {
            self.map_range_2mb(ipa, len, pa, pte);
            if ipa == 0x17000000 {
                println!("map 2mb for gp10b");
                self.show_pt(ipa);
            }
        } else {
            if ipa == 0x17000000 {
                println!("normal map for gp10b");
            }
            self.map_range(ipa, len, pa, pte);
        }
    }
}
