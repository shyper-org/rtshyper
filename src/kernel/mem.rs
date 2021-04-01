use crate::arch::PAGE_SIZE;
use crate::board::*;
use crate::lib::round_up;

use super::mem_region::*;

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

    let base = round_up((_image_end as usize), PAGE_SIZE);
    let size = (PLAT_DESC.mem_desc.regions[0].size as usize
        - (base - PLAT_DESC.mem_desc.base as usize))
        / PAGE_SIZE;

    println!("init memory, please waiting...");
    unsafe {
        rlibc::memset(base as *mut u8, 0, size as usize * PAGE_SIZE);
        // core::intrinsics::volatile_set_memory(ptr, 0, size as usize * PAGE_SIZE);
    }

    let mut heap_lock = HEAPREGION.lock();
    (*heap_lock).region_init(0, base, size, size, 0);
    // use crate::lib::*;
    // let mut v = heap_lock.map.get(155);
    // println!("v is {}", v);
    // heap_lock.map.set(155);
    // v = heap_lock.map.get(155);
    // println!("v is {}", v);
    // heap_lock.map.clear(155);
    // v = heap_lock.map.get(155);
    // println!("v is {}", v);
    drop(heap_lock);

    println!("Memory Heap: base 0x{:x}, size {} MB / {} pages", base,
        size * PAGE_SIZE / (1024 * 1024), size);
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
    let vm_region_num = PLAT_DESC.mem_desc.region_num -1;

    for i in 0..vm_region_num {
        let mut mem_region = MemRegion::new();
        mem_region.init(
            i, 
            PLAT_DESC.mem_desc.regions[i + 1].base,
            PLAT_DESC.mem_desc.regions[i + 1].size / PAGE_SIZE,
            PLAT_DESC.mem_desc.regions[i + 1].size / PAGE_SIZE,
            0
        );
        pages += PLAT_DESC.mem_desc.regions[i + 1].size / PAGE_SIZE;

        let mut vm_region_lock = VMREGION.lock();
        (*vm_region_lock).push(mem_region);
        drop(vm_region_lock);
    }

    println!("Memory VM regions: total {} region, size {} MB / {} pages", 
        vm_region_num, pages * PAGE_SIZE / (1024 * 1024), pages);
    println!("Memory VM regions init ok!");
}
