use buddy_system_allocator::LockedHeap;

// rCore buddy system allocator
use crate::arch::PAGE_SIZE;
use crate::util::{memset_safe, round_up, round_down};
use crate::board::PLAT_DESC;

#[global_allocator]
pub static HEAP_ALLOCATOR: LockedHeap<32> = LockedHeap::empty();

pub fn heap_init() {
    println!("init buddy system");
    extern "C" {
        // Note: link-time label, see aarch64.lds
        fn _image_end();
    }

    if PLAT_DESC.mem_desc.region_num == 0 {
        println!("Platform has no memory region!");
    }

    let base = round_up(_image_end as usize, PAGE_SIZE);
    let size = round_down(
        PLAT_DESC.mem_desc.regions[0].size as usize - (base - PLAT_DESC.mem_desc.base as usize),
        PAGE_SIZE,
    );

    println!("init memory, please waiting...");
    memset_safe(base as *mut u8, 0, size);

    println!(
        "Memory Heap: base {:#x}, size {} MB / {} pages",
        base,
        size >> 20,
        size / PAGE_SIZE
    );
    println!("Memory Heap init ok");
    unsafe {
        HEAP_ALLOCATOR.lock().init(base, size);
    }
}

#[alloc_error_handler]
fn alloc_error_handler(layout: core::alloc::Layout) -> ! {
    println!("Heap allocation error, layout = {:?}", layout);
    panic!("alloc_error_handler: heap Out Of Memory");
}
