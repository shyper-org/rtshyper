use core::ops::Range;
use buddy_system_allocator::LockedHeap;

#[global_allocator]
pub static HEAP_ALLOCATOR: LockedHeap<32> = LockedHeap::empty();

pub fn heap_init() {
    #[repr(align(4096))]
    struct HeapRegion([u8; HEAP_SIZE]);
    const HEAP_SIZE: usize = (1 << 20) * 40;
    static mut HEAP_REGION: HeapRegion = HeapRegion([0; HEAP_SIZE]);

    println!("init buddy system");
    unsafe {
        HEAP_ALLOCATOR.lock().init(HEAP_REGION.0.as_ptr() as usize, HEAP_SIZE);
    }
}

// make sure that the va ranges has corresponding physical pages
pub fn heap_expansion(va_regions: &[Range<usize>]) {
    for region in va_regions {
        info!("heap_expansion: {:?}", region);
        unsafe {
            HEAP_ALLOCATOR.lock().add_to_heap(region.start, region.end);
        }
    }
}

#[alloc_error_handler]
fn alloc_error_handler(layout: core::alloc::Layout) -> ! {
    panic!("Out Of Memory: Heap allocation error, layout = {:?}", layout);
}
