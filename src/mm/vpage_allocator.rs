use core::borrow::Borrow;
use core::cmp::Ordering;
use core::fmt;
use core::ops::{RangeInclusive, Deref, DerefMut};

use spin::Mutex;
use intrusive_collections::Bound;

use crate::arch::{LVL1_SHIFT, PAGE_SIZE};
use crate::kernel::AllocError;
use crate::util::round_up;

use super::_image_end;
use super::page::{Page, VAddr};
use super::util::static_array_rb_tree::{StaticArrayRBTree, Inner, ValueRefMut};

pub const CPU_BANKED_ADDRESS: usize = 0x4_0000_0000; // 0x400000000 = 16GB

const MAX_VIRTUAL_ADDRESS: usize = crate::mm::vpage_allocator::CPU_BANKED_ADDRESS - 1;
pub const MAX_PAGE_NUMBER: usize = MAX_VIRTUAL_ADDRESS / PAGE_SIZE;

const MIN_PAGE: Page = Page::containing_address(VAddr::zero());
const MAX_PAGE: Page = Page::containing_address(VAddr::new(MAX_VIRTUAL_ADDRESS));

static PAGES_UPPER_BOUND: Page = Page::containing_address(VAddr::new(MAX_VIRTUAL_ADDRESS));

static FREE_PAGE_LIST: Mutex<StaticArrayRBTree<Chunk>> = Mutex::new(StaticArrayRBTree::empty());

pub fn init() {
    extern "C" {
        fn CPU_BASE();
    }
    assert_eq!(CPU_BANKED_ADDRESS, CPU_BASE as usize);

    let image_end_align_gb = round_up(_image_end as usize, 1 << LVL1_SHIFT);
    let va_range = Page::containing_address(VAddr::new(image_end_align_gb))..=PAGES_UPPER_BOUND;
    // info!("VAddr range: {:#x}..{:#x}", va_range.start(), va_range.end());

    let mut initial_free_chunks: [Option<Chunk>; 32] = Default::default();
    initial_free_chunks[0] = Some(Chunk {
        pages: PageRange(va_range),
    });

    *FREE_PAGE_LIST.lock() = StaticArrayRBTree::new(initial_free_chunks);
    FREE_PAGE_LIST.lock().convert_to_heap_allocated();
}

fn page_num(size: usize) -> usize {
    round_up(size, PAGE_SIZE) / PAGE_SIZE
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct PageRange(RangeInclusive<Page>);

impl PageRange {
    const fn new(start: Page, end: Page) -> Self {
        Self(start..=end)
    }

    pub const fn empty() -> Self {
        Self(Page { number: 1 }..=Page { number: 0 })
    }

    const fn size_in_pages(&self) -> usize {
        (self.0.end().number() + 1).saturating_sub(self.0.start().number())
    }

    fn size_in_bytes(&self) -> usize {
        self.size_in_pages() * PAGE_SIZE
    }

    pub fn as_range_incluesive(&self) -> RangeInclusive<usize> {
        self.0.start().start_address().value()..=self.0.start().start_address().value() + self.size_in_bytes() - 1
    }
}

impl Ord for PageRange {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.start().number().cmp(&other.0.start().number())
    }
}

impl PartialOrd for PageRange {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Deref for PageRange {
    type Target = RangeInclusive<Page>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for PageRange {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Clone)]
struct Chunk {
    pages: PageRange,
}

impl Chunk {
    fn as_allocated_pages(&self) -> AllocatedPages {
        AllocatedPages {
            pages: self.pages.clone(),
        }
    }

    fn empty() -> Self {
        Self {
            pages: PageRange::empty(),
        }
    }
}

impl Deref for Chunk {
    type Target = PageRange;
    fn deref(&self) -> &Self::Target {
        &self.pages
    }
}

impl Borrow<Page> for &'_ Chunk {
    fn borrow(&self) -> &Page {
        self.pages.start()
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub struct VPage {
    inner: PageRange,
}

pub struct AllocatedPages {
    pages: PageRange,
}

// AllocatedPages must not be Cloneable, and it must not expose its inner pages as mutable.
assert_not_impl_any!(AllocatedPages: DerefMut, Clone);

impl Deref for AllocatedPages {
    type Target = PageRange;
    fn deref(&self) -> &PageRange {
        &self.pages
    }
}

impl fmt::Debug for AllocatedPages {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "AllocatedPages({:?})", self.pages)
    }
}

#[allow(dead_code)]
impl AllocatedPages {
    pub fn merge(&mut self, ap: AllocatedPages) -> Result<(), AllocatedPages> {
        // make sure the pages are contiguous
        if *ap.start() != (*self.end() + 1) {
            return Err(ap);
        }
        self.pages = PageRange::new(*self.start(), *ap.end());
        // ensure the now-merged AllocatedPages doesn't run its drop handler and free its pages.
        core::mem::forget(ap);
        Ok(())
    }

    pub fn split(self, at_page: Page) -> Result<(AllocatedPages, AllocatedPages), AllocatedPages> {
        let end_of_first = at_page - 1;

        let (first, second) = if at_page == *self.start() && at_page <= *self.end() {
            let first = PageRange::empty();
            let second = PageRange::new(at_page, *self.end());
            (first, second)
        } else if at_page == (*self.end() + 1) && end_of_first >= *self.start() {
            let first = PageRange::new(*self.start(), *self.end());
            let second = PageRange::empty();
            (first, second)
        } else if at_page > *self.start() && end_of_first <= *self.end() {
            let first = PageRange::new(*self.start(), end_of_first);
            let second = PageRange::new(at_page, *self.end());
            (first, second)
        } else {
            return Err(self);
        };

        // ensure the original AllocatedPages doesn't run its drop handler and free its pages.
        core::mem::forget(self);
        Ok((AllocatedPages { pages: first }, AllocatedPages { pages: second }))
    }
}

// impl Drop for AllocatedPages {
//     fn drop(&mut self) {
//         if self.size_in_pages() == 0 {
//             return;
//         }
//         info!("page_allocator: deallocating {:?}", self);
//         // Simply add the newly-deallocated chunk to the free pages list.
//         let mut locked_list = FREE_PAGE_LIST.lock();
//         let res = locked_list.insert(Chunk {
//             pages: self.pages.clone(),
//         });
//         match res {
//             Ok(_inserted_free_chunk) => return,
//             Err(c) => error!("BUG: couldn't insert deallocated chunk {:?} into free page list", c),
//         }
//     }
// }

struct DeferredAllocAction {
    free1: Chunk,
    free2: Chunk,
}

impl DeferredAllocAction {
    fn new<F1, F2>(free1: F1, free2: F2) -> DeferredAllocAction
    where
        F1: Into<Option<Chunk>>,
        F2: Into<Option<Chunk>>,
    {
        // let free_list = &FREE_PAGE_LIST;
        let free1 = free1.into().unwrap_or_else(Chunk::empty);
        let free2 = free2.into().unwrap_or_else(Chunk::empty);
        DeferredAllocAction {
            // free_list,
            free1,
            free2,
        }
    }
}

fn adjust_chosen_chunk(
    start_page: Page,
    num_pages: usize,
    chosen_chunk: &Chunk,
    mut chosen_chunk_ref: ValueRefMut<Chunk>,
) -> Result<(AllocatedPages, DeferredAllocAction), AllocError> {
    // The new allocated chunk might start in the middle of an existing chunk,
    // so we need to break up that existing chunk into 3 possible chunks: before, newly-allocated, and after.
    //
    // Because Pages and VirtualAddresses use saturating add and subtract, we need to double-check that we're not creating
    // an overlapping duplicate Chunk at either the very minimum or the very maximum of the address space.
    let new_allocation = Chunk {
        // The end page is an inclusive bound, hence the -1. Parentheses are needed to avoid overflow.
        pages: PageRange::new(start_page, start_page + (num_pages - 1)),
    };
    let before = if start_page == MIN_PAGE {
        None
    } else {
        Some(Chunk {
            pages: PageRange::new(*chosen_chunk.start(), *new_allocation.start() - 1),
        })
    };
    let after = if new_allocation.end() == &MAX_PAGE {
        None
    } else {
        Some(Chunk {
            pages: PageRange::new(*new_allocation.end() + 1, *chosen_chunk.end()),
        })
    };

    // some sanity checks -- these can be removed or disabled for better performance
    if let Some(ref b) = before {
        assert!(!new_allocation.contains(b.end()));
        assert!(!b.contains(new_allocation.start()));
    }
    if let Some(ref a) = after {
        assert!(!new_allocation.contains(a.start()));
        assert!(!a.contains(new_allocation.end()));
    }

    // Remove the chosen chunk from the free page list.
    let _removed_chunk = chosen_chunk_ref.remove();
    assert_eq!(Some(chosen_chunk), _removed_chunk.as_ref()); // sanity check

    // TODO: Re-use the allocated wrapper if possible, rather than allocate a new one entirely.
    // if let RemovedValue::RBTree(Some(wrapper_adapter)) = _removed_chunk { ... }

    Ok((
        new_allocation.as_allocated_pages(),
        DeferredAllocAction::new(before, after),
    ))
}

fn find_alignment_chunk<'list>(
    list: &'list mut StaticArrayRBTree<Chunk>,
    alignment: usize,
    num_pages: usize,
) -> Result<(AllocatedPages, DeferredAllocAction), AllocError> {
    // trace!("find alignment chunk");
    // During the first pass, we ignore designated regions.
    match list.0 {
        Inner::Array(ref mut arr) => {
            for elem in arr.iter_mut() {
                if let Some(chunk) = elem {
                    // Skip chunks that are too-small or in the designated regions.
                    if chunk.size_in_pages() < num_pages {
                        continue;
                    } else {
                        let start = *chunk.start();
                        let start_addr = crate::util::round_up(start.start_address().value(), alignment);
                        let start_page = Page::containing_address(VAddr::new(start_addr));
                        let requested_end_page = start_page + (num_pages - 1);
                        if requested_end_page <= *chunk.end() {
                            return adjust_chosen_chunk(
                                *chunk.start(),
                                num_pages,
                                &chunk.clone(),
                                ValueRefMut::Array(elem),
                            );
                        }
                    }
                }
            }
        }
        Inner::RBTree(ref mut tree) => {
            // NOTE: if RBTree had a `range_mut()` method, we could simply do the following:
            // ```
            // let eligible_chunks = tree.range(
            // 	Bound::Excluded(&DESIGNATED_PAGES_LOW_END),
            // 	Bound::Excluded(&DESIGNATED_PAGES_HIGH_START)
            // );
            // for c in eligible_chunks { ... }
            // ```
            //
            // However, RBTree doesn't have a `range_mut()` method, so we use cursors for manual iteration.
            //
            // Because we allocate new pages by peeling them off from the beginning part of a chunk,
            // it's MUCH faster to start the search for free pages from higher addresses moving down.
            // This results in an O(1) allocation time in the general case, until all address ranges are already in use.
            // let mut cursor = tree.cursor_mut();
            let mut cursor = tree.upper_bound_mut(Bound::Excluded(&PAGES_UPPER_BOUND));
            while let Some(chunk) = cursor.get().map(|w| w.deref()) {
                if num_pages < chunk.size_in_pages() {
                    let start = *chunk.start();
                    let start_addr = crate::util::round_up(start.start_address().value(), alignment);
                    let start_page = Page::containing_address(VAddr::new(start_addr));
                    let requested_end_page = start_page + (num_pages - 1);
                    if requested_end_page <= *chunk.end() {
                        return adjust_chosen_chunk(start_page, num_pages, &chunk.clone(), ValueRefMut::RBTree(cursor));
                    }
                }
                cursor.move_prev();
            }
        }
    }
    warn!("find alignment chunk, AllocationError::OutOfAddressSpace");
    Err(AllocError::OutOfFrame(num_pages))
}

fn find_any_chunk<'a>(
    list: &'a mut StaticArrayRBTree<Chunk>,
    num_pages: usize,
) -> Result<(AllocatedPages, DeferredAllocAction), AllocError> {
    // trace!("find any chunk");
    // During the first pass, we ignore designated regions.
    match list.0 {
        Inner::Array(ref mut arr) => {
            for elem in arr.iter_mut() {
                if let Some(chunk) = elem {
                    // Skip chunks that are too-small or in the designated regions.
                    if chunk.size_in_pages() < num_pages {
                        continue;
                    } else {
                        return adjust_chosen_chunk(
                            *chunk.start(),
                            num_pages,
                            &chunk.clone(),
                            ValueRefMut::Array(elem),
                        );
                    }
                }
            }
        }
        Inner::RBTree(ref mut tree) => {
            // NOTE: if RBTree had a `range_mut()` method, we could simply do the following:
            // ```
            // let eligible_chunks = tree.range(
            // 	Bound::Excluded(&DESIGNATED_PAGES_LOW_END),
            // 	Bound::Excluded(&DESIGNATED_PAGES_HIGH_START)
            // );
            // for c in eligible_chunks { ... }
            // ```
            //
            // However, RBTree doesn't have a `range_mut()` method, so we use cursors for manual iteration.
            //
            // Because we allocate new pages by peeling them off from the beginning part of a chunk,
            // it's MUCH faster to start the search for free pages from higher addresses moving down.
            // This results in an O(1) allocation time in the general case, until all address ranges are already in use.
            // let mut cursor = tree.cursor_mut();
            let mut cursor = tree.upper_bound_mut(Bound::Excluded(&PAGES_UPPER_BOUND));
            while let Some(chunk) = cursor.get().map(|w| w.deref()) {
                if num_pages < chunk.size_in_pages() {
                    return adjust_chosen_chunk(*chunk.start(), num_pages, &chunk.clone(), ValueRefMut::RBTree(cursor));
                }
                warn!("Page allocator: unlikely scenario: had to search multiple chunks while trying to allocate {} pages at any address.", num_pages);
                cursor.move_prev();
            }
        }
    }

    Err(AllocError::OutOfFrame(num_pages))
}

pub fn vpage_alloc(len: usize, align: Option<usize>) -> Result<AllocatedPages, AllocError> {
    let num_pages = page_num(len);
    if num_pages == 0 {
        error!("vpage_alloc: alloc 0 pages");
        return Err(AllocError::AllocZeroPage);
    }

    let mut locked_list = FREE_PAGE_LIST.lock();

    if let Some(align) = align {
        find_alignment_chunk(&mut locked_list, align, num_pages)
    } else {
        find_any_chunk(&mut locked_list, num_pages)
    }
    .map(|(ap, action)| {
        if action.free1.size_in_pages() > 0 {
            info!("DeferredAllocAction insert free1 {:?}", action.free1);
            locked_list.insert(action.free1).unwrap();
        }
        if action.free2.size_in_pages() > 0 {
            info!("DeferredAllocAction insert free2 {:?}", action.free2);
            locked_list.insert(action.free2).unwrap();
        }
        ap
    })
}

#[allow(unused)]
pub fn vpage_dealloc(page: AllocatedPages) -> () {
    todo!()
}
