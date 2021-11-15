use crate::arch::PAGE_SIZE;
use crate::board::*;
use crate::lib::{memset_safe, round_up};
use crate::mm::PageFrame;

use super::mem_region::*;

use self::AllocError::*;

pub const VM_MEM_REGION_MAX: usize = 4;

pub fn mem_init() {
    mem_heap_region_init();
    mem_vm_region_init();
    println!("Mem init ok");
}

fn mem_heap_region_init() {
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

    let mut heap_lock = HEAPREGION.lock();
    (*heap_lock).region_init(0, base, size, size, 0);

    drop(heap_lock);

    println!(
        "Memory Heap: base 0x{:x}, size {} MB / {} pages",
        base,
        size * PAGE_SIZE / (1024 * 1024),
        size
    );
    println!("Memory Heap init ok");
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
            i,
            PLAT_DESC.mem_desc.regions[i + 1].base,
            PLAT_DESC.mem_desc.regions[i + 1].size / PAGE_SIZE,
            PLAT_DESC.mem_desc.regions[i + 1].size / PAGE_SIZE,
            0,
        );
        pages += PLAT_DESC.mem_desc.regions[i + 1].size / PAGE_SIZE;

        let mut vm_region_lock = VMREGION.lock();
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

pub enum AllocError {
    AllocZeroPage,
    OutOfFrame,
}

pub fn mem_heap_reset() {
    let mut heap = HEAPREGION.lock();
    memset_safe(heap.region.base as *mut u8, 0, heap.region.size * PAGE_SIZE);
}

pub fn mem_heap_alloc(page_num: usize, _aligned: bool) -> Result<PageFrame, AllocError> {
    if page_num == 0 {
        return Err(AllocZeroPage);
    }

    let mut heap = HEAPREGION.lock();
    if page_num > heap.region.free {
        return Err(OutOfFrame);
    }

    if page_num == 1 {
        return heap.alloc_page();
    }

    heap.alloc_pages(page_num)
}

pub fn mem_page_alloc() -> Result<PageFrame, AllocError> {
    mem_heap_alloc(1, false)
}

pub fn mem_pages_alloc(page_num: usize) -> Result<PageFrame, AllocError> {
    mem_heap_alloc(page_num, false)
}

pub fn mem_pages_free(addr: usize, page_num: usize) -> bool {
    if page_num == 1 {
        let mut heap = HEAPREGION.lock();
        return heap.free_page(addr);
    } else {
        println!(
            "mem_pages_free: multiple pages free occured at address 0x{:x}, {} pages",
            addr, page_num
        );
        return false;
    }
}

pub fn mem_vm_region_alloc(size: usize) -> usize {
    let mut vm_region = VMREGION.lock();
    for i in 0..vm_region.region.len() {
        if vm_region.region[i].free >= size / PAGE_SIZE {
            let start_addr = vm_region.region[i].base + vm_region.region[i].last * PAGE_SIZE;
            vm_region.region[i].last += size / PAGE_SIZE;
            vm_region.region[i].free -= size / PAGE_SIZE;
            return start_addr;
        }
    }

    0
}
