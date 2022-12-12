use crate::arch::PAGE_SIZE;
use crate::board::*;
use crate::kernel::mem_shared_mem_init;
use crate::mm::PageFrame;

use super::mem_region::*;

pub const VM_MEM_REGION_MAX: usize = 4;

pub fn mem_init() {
    mem_vm_region_init();
    mem_shared_mem_init();
    println!("Mem init ok");
}

fn mem_vm_region_init() {
    if PLAT_DESC.mem_desc.region_num - 1 > TOTAL_MEM_REGION_MAX {
        panic!("Platform memory regions overrun!");
    } else if PLAT_DESC.mem_desc.region_num == 0 {
        panic!("Platform Vm Memory Regions Overrun!");
    }

    if PLAT_DESC.mem_desc.region_num <= 1 {
        panic!("Platform has no VM memory region!");
    }

    let mut pages: usize = 0;
    let vm_region_num = PLAT_DESC.mem_desc.region_num - 1;

    for i in 0..vm_region_num {
        let mut mem_region = MemRegion::new();
        mem_region.init(
            PLAT_DESC.mem_desc.regions[i + 1].base,
            PLAT_DESC.mem_desc.regions[i + 1].size / PAGE_SIZE,
            PLAT_DESC.mem_desc.regions[i + 1].size / PAGE_SIZE,
            0,
        );
        pages += PLAT_DESC.mem_desc.regions[i + 1].size / PAGE_SIZE;

        let mut vm_region_lock = VM_REGION.lock();
        (*vm_region_lock).push(mem_region);
    }

    println!(
        "Memory VM regions: total {} region, size {} MB / {} pages",
        vm_region_num,
        pages * PAGE_SIZE / (1024 * 1024),
        pages
    );
    println!("Memory VM regions init ok!");
}

#[derive(Debug)]
pub enum AllocError {
    AllocZeroPage,
    OutOfFrame,
}

pub fn mem_page_alloc() -> Result<PageFrame, AllocError> {
    PageFrame::alloc_pages(1)
}

pub fn mem_pages_alloc(page_num: usize) -> Result<PageFrame, AllocError> {
    PageFrame::alloc_pages(page_num)
}

pub fn mem_vm_region_alloc(size: usize) -> usize {
    let mut vm_region = VM_REGION.lock();
    for i in 0..vm_region.region.len() {
        if vm_region.region[i].free >= size / PAGE_SIZE {
            let start_addr = vm_region.region[i].base;
            let region_size = vm_region.region[i].size;
            if vm_region.region[i].size > size / PAGE_SIZE {
                vm_region.push(MemRegion {
                    base: start_addr + size,
                    size: region_size - size / PAGE_SIZE,
                    free: region_size - size / PAGE_SIZE,
                    last: 0, // never use in vm mem region
                });
                vm_region.region[i].size = size / PAGE_SIZE;
            }
            vm_region.region[i].free = 0;

            return start_addr;
        }
    }

    0
}

pub fn mem_vm_region_free(start: usize, size: usize) {
    let mut vm_region = VM_REGION.lock();
    let mut free_idx = None;
    // free mem region
    for (idx, region) in vm_region.region.iter_mut().enumerate() {
        if start == region.base && region.free == 0 {
            region.free += size / PAGE_SIZE;
            free_idx = Some(idx);
            break;
        }
    }
    // merge mem region
    while free_idx.is_some() {
        let merge_idx = free_idx.unwrap();
        let base = vm_region.region[merge_idx].base;
        let size = vm_region.region[merge_idx].size;
        free_idx = None;
        for (idx, region) in vm_region.region.iter_mut().enumerate() {
            if region.free != 0 && base == region.base + region.size * PAGE_SIZE {
                // merge free region into curent region
                region.size += size;
                region.free += size;
                free_idx = Some(if idx < merge_idx { idx } else { idx - 1 });
                vm_region.region.remove(merge_idx);
                break;
            } else if region.free != 0 && base + size * PAGE_SIZE == region.base {
                // merge curent region into free region
                let size = region.size;
                vm_region.region[merge_idx].size += size;
                vm_region.region[merge_idx].free += size;
                free_idx = Some(if merge_idx < idx { merge_idx } else { merge_idx - 1 });
                vm_region.region.remove(idx);
                break;
            }
        }
    }
    println!("Free mem from pa 0x{:x} to 0x{:x}", start, start + size);
}
