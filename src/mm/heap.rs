use buddy_system_allocator::LockedHeap;
use core::ops::RangeInclusive;

use crate::arch::HYP_VA_SIZE;

const HYP_VA_SIZE_USIZE: usize = HYP_VA_SIZE as usize;

#[global_allocator]
pub static HEAP_ALLOCATOR: LockedHeap<HYP_VA_SIZE_USIZE> = LockedHeap::empty();

pub fn heap_init() {
    #[repr(align(4096))]
    struct HeapRegion([u8; HEAP_SIZE]);
    const HEAP_SIZE: usize = (1 << 20) * 4;
    static mut HEAP_REGION: HeapRegion = HeapRegion([0; HEAP_SIZE]);

    unsafe {
        info!(
            "init buddy system: {:#p}..{:#p}",
            HEAP_REGION.0.as_ptr_range().start,
            HEAP_REGION.0.as_ptr_range().end
        );
        HEAP_ALLOCATOR.lock().init(HEAP_REGION.0.as_ptr() as usize, HEAP_SIZE);
    }
}

// make sure that the va ranges has corresponding physical pages
pub fn heap_expansion(region: RangeInclusive<usize>) {
    info!("heap_expansion: {:#x}..={:#x}", region.start(), region.end());
    unsafe {
        HEAP_ALLOCATOR.lock().add_to_heap(*region.start(), *region.end() + 1);
    }
}

#[alloc_error_handler]
fn alloc_error_handler(layout: core::alloc::Layout) -> ! {
    panic!("Out Of Memory: Heap allocation error, layout = {:x?}", layout);
}
