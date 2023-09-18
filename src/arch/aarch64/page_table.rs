use alloc::collections::BTreeMap;

use spin::Mutex;

use crate::arch::Address;
use crate::arch::ArchPageTableEntryTrait;
use crate::arch::ArchTrait;
use crate::arch::TlbInvalidate;
use crate::kernel::mem_page_alloc;
use crate::kernel::Cpu;
use crate::mm::PageFrame;
use crate::util::memcpy_safe;
use crate::util::round_up;

use super::{Arch, PAGE_SIZE, PTE_PER_PAGE};

const PTE_TABLE: usize = 0b11;
const PTE_PAGE: usize = 0b11;
const PTE_BLOCK: usize = 0b01;

const PTE_S1_FIELD_AP_RW_EL0_NONE: usize = 0b00 << 6;
const PTE_S1_FIELD_AP_RW_EL0_RW: usize = 0b01 << 6;
const PTE_S1_FIELD_AP_RO_EL0_NONE: usize = 0b10 << 6;
const PTE_S1_FIELD_AP_RO_EL0_RW: usize = 0b11 << 6;

const PTE_S1_FIELD_SH_NON_SHAREABLE: usize = 0b00 << 8;
const PTE_S1_FIELD_SH_RESERVED: usize = 0b01 << 8;
const PTE_S1_FIELD_SH_OUTER_SHAREABLE: usize = 0b10 << 8;
const PTE_S1_FIELD_SH_INNER_SHAREABLE: usize = 0b11 << 8;

const PTE_S1_FIELD_AF: usize = 1 << 10;

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
    pte_s1_field_attr_indx(1) | PTE_S1_FIELD_AP_RW_EL0_NONE | PTE_S1_FIELD_SH_INNER_SHAREABLE | PTE_S1_FIELD_AF;

const PTE_S1_RO: usize =
    pte_s1_field_attr_indx(1) | PTE_S1_FIELD_AP_RO_EL0_NONE | PTE_S1_FIELD_SH_INNER_SHAREABLE | PTE_S1_FIELD_AF;

pub const PTE_S1_DEVICE: usize =
    pte_s1_field_attr_indx(0) | PTE_S1_FIELD_AP_RW_EL0_NONE | PTE_S1_FIELD_SH_OUTER_SHAREABLE | PTE_S1_FIELD_AF;

pub const PTE_S2_DEVICE: usize =
    PTE_S2_FIELD_MEM_ATTR_DEVICE_NGNRNE | PTE_S2_FIELD_AP_RW | PTE_S2_FIELD_SH_OUTER_SHAREABLE | PTE_S2_FIELD_AF;

pub const PTE_S2_NORMAL: usize = PTE_S2_FIELD_MEM_ATTR_NORMAL_INNER_WRITE_BACK_CACHEABLE
    | PTE_S2_FIELD_MEM_ATTR_NORMAL_OUTER_WRITE_BACK_CACHEABLE
    | PTE_S2_FIELD_AP_RW
    | PTE_S2_FIELD_SH_OUTER_SHAREABLE
    | PTE_S2_FIELD_AF;

pub const PTE_S2_RO: usize = PTE_S2_FIELD_MEM_ATTR_NORMAL_INNER_WRITE_BACK_CACHEABLE
    | PTE_S2_FIELD_MEM_ATTR_NORMAL_OUTER_WRITE_BACK_CACHEABLE
    | PTE_S2_FIELD_AP_RO
    | PTE_S2_FIELD_SH_OUTER_SHAREABLE
    | PTE_S2_FIELD_AF;

const fn pte_s1_field_attr_indx(idx: usize) -> usize {
    idx << 2
}

// page_table const
pub const LVL1_SHIFT: usize = 30;
pub const LVL2_SHIFT: usize = 21;
pub const LVL3_SHIFT: usize = 12;

// cfg_if::cfg_if! {
//     if #[cfg(any(feature = "pa-bits-39"))] {
//         const LVL_SHIFT: &[usize] = &[30, 21, 12];
//     } else if #[cfg(any(feature = "pa-bits-48"))] {
//         const LVL_SHIFT: &[usize] = &[39, 30, 21, 12];
//     }
// }

// pub fn pt_lvl_idx(va: usize, lvl: usize) -> usize {
//     (va >> LVL_SHIFT[lvl]) & (PTE_PER_PAGE - 1)
// }

// fn pt_lvl_offset(va: usize, lvl: usize) -> usize {
//     va & ((1 << LVL_SHIFT[lvl]) - 1)
// }

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

fn pt_lvl1_offset(va: usize) -> usize {
    va & ((1 << LVL1_SHIFT) - 1)
}

fn pt_lvl2_offset(va: usize) -> usize {
    va & ((1 << LVL2_SHIFT) - 1)
}

fn pt_lvl3_offset(va: usize) -> usize {
    va & ((1 << LVL3_SHIFT) - 1)
}

pub fn pt_map_banked_cpu(cpu: &mut Cpu) -> usize {
    use crate::mm::vpage_allocator::CPU_BANKED_ADDRESS;

    let addr = unsafe { &super::mmu::LVL1_PAGE_TABLE as *const _ } as usize;

    memcpy_safe(cpu.cpu_pt.lvl1.as_ptr() as *const _, addr as *mut _, PAGE_SIZE);
    cpu.cpu_pt.lvl2.fill(0);
    cpu.cpu_pt.lvl3.fill(0);

    use core::mem::size_of;
    const_assert!(size_of::<Cpu>() <= (1 << LVL2_SHIFT));

    let cpu_addr = cpu as *const _ as usize;
    let lvl2_addr = cpu.cpu_pt.lvl2.as_ptr() as usize;
    let lvl3_addr = cpu.cpu_pt.lvl3.as_ptr() as usize;
    cpu.cpu_pt.lvl1[pt_lvl1_idx(CPU_BANKED_ADDRESS)] = lvl2_addr | PTE_S1_NORMAL | PTE_TABLE;
    cpu.cpu_pt.lvl2[pt_lvl2_idx(CPU_BANKED_ADDRESS)] = lvl3_addr | PTE_S1_NORMAL | PTE_TABLE;

    let page_num = round_up(size_of::<Cpu>(), PAGE_SIZE) / PAGE_SIZE;
    let guard_page_index = offset_of!(Cpu, _guard_page) / PAGE_SIZE;
    for i in 0..page_num {
        let pte = if i == guard_page_index {
            PTE_S1_RO
        } else {
            PTE_S1_NORMAL
        } | PTE_PAGE;
        cpu.cpu_pt.lvl3[pt_lvl3_idx(CPU_BANKED_ADDRESS) + i] = (cpu_addr + i * PAGE_SIZE) | pte;
    }

    crate::arch::Arch::invalid_hypervisor_all();
    cpu.cpu_pt.lvl1.as_ptr() as usize
}

#[repr(transparent)]
#[derive(Copy, Clone, Debug)]
struct Aarch64PageTableEntry(usize);

impl ArchPageTableEntryTrait for Aarch64PageTableEntry {
    #[inline]
    fn from_pte(value: usize) -> Self {
        Aarch64PageTableEntry(value)
    }

    #[inline]
    fn from_pa(pa: usize) -> Self {
        Aarch64PageTableEntry(pa)
    }

    #[inline]
    fn to_pte(&self) -> usize {
        self.0
    }

    #[inline]
    fn to_pa(&self) -> usize {
        self.0 & 0x0000_FFFF_FFFF_F000
    }

    #[inline]
    fn valid(&self) -> bool {
        self.0 & 0b11 != 0
    }

    #[inline]
    fn entry(&self, index: usize) -> Aarch64PageTableEntry {
        Aarch64PageTableEntry(unsafe { (self.to_pa().pa2hva() as *const usize).add(index).read_volatile() })
    }

    #[inline]
    fn set_entry(&self, index: usize, value: Aarch64PageTableEntry) {
        unsafe { (self.to_pa().pa2hva() as *mut usize).add(index).write_volatile(value.0) }
    }

    #[inline]
    fn make_table(frame_pa: usize) -> Self {
        Aarch64PageTableEntry::from_pa(frame_pa | PTE_TABLE)
    }
}

impl Aarch64PageTableEntry {
    fn to_hva(self) -> usize {
        self.to_pa().pa2hva()
    }
}

#[derive(PartialEq, Eq)]
enum MmuStage {
    S1,
    S2,
}

pub struct PageTable {
    directory_pa: usize,
    stage: MmuStage,
    pages: Mutex<BTreeMap<usize, PageFrame>>,
}

const SIZE_2MB: usize = 1 << LVL2_SHIFT;
const SIZE_1GB: usize = 1 << LVL1_SHIFT;

impl PageTable {
    pub fn from_pa(directory: usize, is_stage2: bool) -> Self {
        Self {
            directory_pa: directory,
            stage: if is_stage2 { MmuStage::S2 } else { MmuStage::S1 },
            pages: Mutex::new(BTreeMap::new()),
        }
    }

    pub fn new(directory: PageFrame, is_stage2: bool) -> Self {
        let directory_pa = directory.pa();
        let mut map = BTreeMap::new();
        map.insert(directory.pa(), directory);
        Self {
            directory_pa,
            stage: if is_stage2 { MmuStage::S2 } else { MmuStage::S1 },
            pages: Mutex::new(map),
        }
    }

    pub fn base_pa(&self) -> usize {
        self.directory_pa
    }

    pub fn ipa2pa(&self, ipa: usize) -> Option<usize> {
        match self.stage {
            MmuStage::S1 => Arch::mem_translate(ipa),
            MmuStage::S2 => {
                let directory = Aarch64PageTableEntry::from_pa(self.directory_pa);
                let l1e = directory.entry(pt_lvl1_idx(ipa));
                if !l1e.valid() {
                    return None;
                } else if l1e.to_pte() & 0b11 == PTE_BLOCK {
                    return Some(l1e.to_pa() | pt_lvl1_offset(ipa));
                }
                let l2e = l1e.entry(pt_lvl2_idx(ipa));
                if !l2e.valid() {
                    return None;
                } else if l2e.to_pte() & 0b11 == PTE_BLOCK {
                    return Some(l2e.to_pa() | pt_lvl2_offset(ipa));
                }
                let l3e = l2e.entry(pt_lvl3_idx(ipa));
                if l3e.valid() {
                    return Some(l3e.to_pa() | pt_lvl3_offset(ipa));
                }
                None
            }
        }
    }

    fn map_2mb(&self, ipa: usize, pa: usize, pte: usize) {
        let directory = Aarch64PageTableEntry::from_pa(self.directory_pa);
        let mut l1e = directory.entry(pt_lvl1_idx(ipa));
        if !l1e.valid() {
            if let Ok(frame) = mem_page_alloc() {
                l1e = Aarch64PageTableEntry::make_table(frame.pa());
                let mut pages = self.pages.lock();
                let pf = pages.insert(frame.pa(), frame);
                debug_assert!(pf.is_none());
                directory.set_entry(pt_lvl1_idx(ipa), l1e);
            } else {
                panic!("map lv1 page failed");
            }
        }

        let l2e = l1e.entry(pt_lvl2_idx(ipa));
        if l2e.valid() {
            error!("map_2mb lvl 2 already mapped with {:#x}", l2e.to_pte());
        } else {
            l1e.set_entry(pt_lvl2_idx(ipa), Aarch64PageTableEntry::from_pa(pa | pte | PTE_BLOCK));
            // self.tlb_invalidate(ipa);
        }
    }

    fn unmap_2mb(&self, ipa: usize) {
        let directory = Aarch64PageTableEntry::from_pa(self.directory_pa);
        let l1e = directory.entry(pt_lvl1_idx(ipa));
        if l1e.valid() {
            let l2e = l1e.entry(pt_lvl2_idx(ipa));
            if l2e.valid() {
                l1e.set_entry(pt_lvl2_idx(ipa), Aarch64PageTableEntry(0));
                self.tlb_invalidate(ipa);
            }
        }
    }

    fn map(&self, ipa: usize, pa: usize, pte: usize) {
        let directory = Aarch64PageTableEntry::from_pa(self.directory_pa);
        let mut l1e = directory.entry(pt_lvl1_idx(ipa));
        if !l1e.valid() {
            if let Ok(frame) = mem_page_alloc() {
                l1e = Aarch64PageTableEntry::make_table(frame.pa());
                let mut pages = self.pages.lock();
                let pf = pages.insert(frame.pa(), frame);
                debug_assert!(pf.is_none());
                directory.set_entry(pt_lvl1_idx(ipa), l1e);
            } else {
                panic!("map lv1 page failed");
            }
        }

        let mut l2e = l1e.entry(pt_lvl2_idx(ipa));
        if !l2e.valid() {
            if let Ok(frame) = mem_page_alloc() {
                l2e = Aarch64PageTableEntry::make_table(frame.pa());
                let mut pages = self.pages.lock();
                let pf = pages.insert(frame.pa(), frame);
                debug_assert!(pf.is_none());
                l1e.set_entry(pt_lvl2_idx(ipa), l2e);
            } else {
                panic!("map lv2 page failed");
            }
        } else if l2e.to_pte() & 0b11 == PTE_BLOCK {
            error!("map lvl 2 already mapped with 2mb {:#x}", l2e.to_pte());
        }
        let l3e = l2e.entry(pt_lvl3_idx(ipa));
        if l3e.valid() {
            error!("map lvl 3 already mapped with {:#x}", l3e.to_pte());
        } else {
            l2e.set_entry(pt_lvl3_idx(ipa), Aarch64PageTableEntry::from_pa(pa | PTE_PAGE | pte));
            // self.tlb_invalidate(ipa);
        }
    }

    fn unmap(&self, ipa: usize) {
        let directory = Aarch64PageTableEntry::from_pa(self.directory_pa);
        let l1e = directory.entry(pt_lvl1_idx(ipa));
        if l1e.valid() {
            let l2e = l1e.entry(pt_lvl2_idx(ipa));
            if l2e.valid() {
                let l3e = l2e.entry(pt_lvl3_idx(ipa));
                if l3e.valid() {
                    l2e.set_entry(pt_lvl3_idx(ipa), Aarch64PageTableEntry::from_pa(0));
                    // invalidate tlbs
                    self.tlb_invalidate(ipa);
                }
            }
        }
    }

    fn map_range_2mb(&self, ipa: usize, len: usize, pa: usize, pte: usize) {
        let page_num = round_up(len, SIZE_2MB) / SIZE_2MB;

        for i in 0..page_num {
            self.map_2mb(ipa + i * SIZE_2MB, pa + i * SIZE_2MB, pte);
        }
    }

    fn map_range_1gb(&self, ipa: usize, len: usize, pa: usize, pte: usize) {
        let page_num = round_up(len, SIZE_1GB) / SIZE_1GB;
        for i in 0..page_num {
            let ipa = ipa + i * SIZE_1GB;
            let pa = pa + i * SIZE_1GB;
            let directory = Aarch64PageTableEntry::from_pa(self.directory_pa);
            let l1e = directory.entry(pt_lvl1_idx(ipa));
            if l1e.valid() {
                error!("map_range_1gb: map lv1 page failed");
                return;
            } else {
                directory.set_entry(pt_lvl1_idx(ipa), Aarch64PageTableEntry::from_pa(pa | pte | PTE_BLOCK));
            }
        }
    }

    fn unmap_range_2mb(&self, ipa: usize, len: usize) {
        let page_num = round_up(len, SIZE_2MB) / SIZE_2MB;

        for i in 0..page_num {
            self.unmap_2mb(ipa + i * SIZE_2MB);
        }
    }

    fn map_range(&self, ipa: usize, len: usize, pa: usize, pte: usize) {
        let page_num = round_up(len, PAGE_SIZE) / PAGE_SIZE;
        for i in 0..page_num {
            self.map(ipa + i * PAGE_SIZE, pa + i * PAGE_SIZE, pte);
        }
    }

    fn unmap_range(&self, ipa: usize, len: usize) {
        let page_num = round_up(len, PAGE_SIZE) / PAGE_SIZE;
        for i in 0..page_num {
            self.unmap(ipa + i * PAGE_SIZE);
        }
    }

    pub fn show_pt(&self, ipa: usize) {
        let directory = Aarch64PageTableEntry::from_pa(self.directory_pa);
        info!("1 {:x}", directory.to_pte());
        let l1e = directory.entry(pt_lvl1_idx(ipa));
        info!("2 {:x}", l1e.to_pte());
        let l2e = l1e.entry(pt_lvl2_idx(ipa));
        info!("3 {:x}", l2e.to_pte());
        if !l2e.valid() {
            info!("invalid ipa {:x} to l2 pte {:x}", ipa, l2e.to_pte());
        } else if l2e.to_pte() & 0b11 == PTE_BLOCK {
            info!("l2 ipa {:x} to pa {:x}", ipa, l2e.to_pte());
        } else {
            let l3e = l2e.entry(pt_lvl3_idx(ipa));
            info!("l3 ipa {:x} to pa {:x}", ipa, l3e.to_pte());
        }
    }

    pub fn pt_map_range(&self, ipa: usize, len: usize, pa: usize, pte: usize, map_block: bool) {
        if map_block && ipa % SIZE_1GB == 0 && len % SIZE_1GB == 0 && pa % SIZE_1GB == 0 {
            self.map_range_1gb(ipa, len, pa, pte);
        } else if map_block && ipa % SIZE_2MB == 0 && len % SIZE_2MB == 0 && pa % SIZE_2MB == 0 {
            self.map_range_2mb(ipa, len, pa, pte);
        } else {
            self.map_range(ipa, len, pa, pte);
        }
    }

    fn tlb_invalidate(&self, va: usize) {
        match self.stage {
            MmuStage::S1 => crate::arch::Arch::invalid_hypervisor_va(va),
            MmuStage::S2 => crate::arch::Arch::invalid_guest_ipa(va),
        }
    }

    pub fn pt_unmap_range(&self, ipa: usize, len: usize, map_block: bool) {
        if ipa % SIZE_2MB == 0 && len % SIZE_2MB == 0 && map_block {
            self.unmap_range_2mb(ipa, len);
        } else {
            self.unmap_range(ipa, len);
        }
        if self.stage == MmuStage::S1 {
            Arch::invalid_hypervisor_all();
        }
    }

    pub fn get_pte(&self, va: usize, lvl: usize) -> Option<usize> {
        if lvl == 1 {
            let directory = Aarch64PageTableEntry::from_pa(self.directory_pa);
            let l1e = directory.entry(pt_lvl1_idx(va));
            if l1e.valid() {
                Some(l1e.to_pte())
            } else {
                None
            }
        } else {
            None
        }
    }

    pub fn set_pte(&self, va: usize, lvl: usize, pte: usize) {
        if lvl == 1 {
            let directory = Aarch64PageTableEntry::from_pa(self.directory_pa);
            let l1e = directory.entry(pt_lvl1_idx(va));
            if l1e.valid() {
                warn!("set_pte: va {va:#x} is already mapped with {:#x}!", l1e.to_pte());
            }
            let table = Aarch64PageTableEntry(pte);
            assert!(table.valid());
            directory.set_entry(pt_lvl1_idx(va), table);
        } else {
            panic!("set_pte: not support lvl {lvl}");
        }
    }
}
