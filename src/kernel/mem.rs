use crate::arch::PAGE_SIZE;
use crate::board::*;
use crate::kernel::mem_shared_mem_init;
use crate::lib::{memset_safe, round_up};
use crate::mm::PageFrame;

use super::mem_region::*;

use self::AllocError::*;

pub const VM_MEM_REGION_MAX: usize = 4;

pub fn mem_init() {
    mem_heap_region_init();
    mem_vm_region_init();
    mem_shared_mem_init();
    println!("Mem init ok");
}

pub fn mem_heap_region_init() {
    extern "C" {
        // Note: link-time label, see aarch64.lds
        fn _image_end();
    }

    if PLAT_DESC.mem_desc.region_num == 0 {
        println!("Platform has no memory region!");
    }

    let base = round_up(_image_end as usize, PAGE_SIZE);
    let size = round_up(
        PLAT_DESC.mem_desc.regions[0].size as usize - (base - PLAT_DESC.mem_desc.base as usize),
        PAGE_SIZE,
    ) / PAGE_SIZE;

    println!("init memory, please waiting...");
    memset_safe(base as *mut u8, 0, size as usize * PAGE_SIZE);
    // core::intrinsics::volatile_set_memory(ptr, 0, size as usize * PAGE_SIZE);

    let mut heap_lock = HEAP_REGION.lock();
    (*heap_lock).region_init(base, size, size, 0);

    drop(heap_lock);

    println!(
        "Memory Heap: base 0x{:x}, size {} MB / {} pages",
        base,
        size * PAGE_SIZE / (1024 * 1024),
        size
    );
    println!("Memory Heap init ok");
}

/// Reserve Heap Memory from base_addr to base_addr + size
/// #Example
/// ```
/// mem_heap_region_reserve(0x8a000000, 0x8000000);
/// ```
pub fn mem_heap_region_reserve(base_addr: usize, size: usize) {
    let mut heap = HEAP_REGION.lock();
    heap.reserve_pages(base_addr, round_up(size, PAGE_SIZE) / PAGE_SIZE);
    println!(
        "Reserve Heap Region 0x{:x} ~ 0x{:x}",
        base_addr,
        base_addr + round_up(size, PAGE_SIZE)
    );
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

fn mem_heap_reset() {
    let heap = HEAP_REGION.lock();
    memset_safe(heap.region.base as *mut u8, 0, heap.region.size * PAGE_SIZE);
}

pub fn mem_heap_alloc(page_num: usize, _aligned: bool) -> Result<usize, AllocError> {
    if page_num == 0 {
        return Err(AllocZeroPage);
    }

    let mut heap = HEAP_REGION.lock();
    if page_num > heap.region.free {
        return Err(OutOfFrame);
    }

    heap.alloc_pages(page_num)
}

pub fn mem_heap_free(addr: usize, page_num: usize) -> bool {
    let mut heap = HEAP_REGION.lock();
    heap.free_pages(addr, page_num)
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
