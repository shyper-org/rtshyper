use alloc::vec::Vec;
use spin::Mutex;

use crate::arch::{PAGE_SIZE, PAGE_SHIFT, cache_init, CPU_CACHE, CacheInfoTrait, PageTable};
use crate::board::*;
use crate::kernel::mem_shared_mem_init;
use crate::mm::PageFrame;

use super::mem_region::*;

pub const VM_MEM_REGION_MAX: usize = 4;

pub fn mem_init() {
    cache_init();
    mem_vm_region_init();
    mem_shared_mem_init();
    mem_vm_region_init_by_colors();
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

#[derive(Clone, Debug)]
pub struct ColorMemRegion {
    pub color: usize,
    pub base: usize,
    pub count: usize,
    pub step: usize,
    pub free: usize,
}

impl ColorMemRegion {
    fn new(color: usize, base: usize, count: usize, step: usize) -> Self {
        Self {
            color,
            base,
            count,
            step,
            free: count,
        }
    }

    fn left_neighbor(&self, other: &Self) -> bool {
        self.base + self.count * self.step == other.base
    }
}

static VM_REGION_BY_COLOR: Mutex<Vec<Vec<ColorMemRegion>>> = Mutex::new(Vec::new());

pub fn mem_vm_region_alloc_by_colors(size: usize, colors: usize) -> Result<Vec<ColorMemRegion>, ()> {
    info!("alloc {:#x}b in colors {:b}", size, colors);
    let count = colors.count_ones() as usize;
    if count == 0 {
        return Err(());
    }
    let page_num = size / PAGE_SIZE;

    // hold the lock until return
    let mut vm_region_by_color = VM_REGION_BY_COLOR.lock();
    // init a vec, contains color -> page_num, init value is the free page num
    let color2pages = {
        let mut color2pages = vec![];
        let mut index = 0;
        // get the color list, sum free space in these colors
        let mut free_pages = 0;
        while colors >= (1 << index) {
            if colors & (1 << index) != 0 {
                let free_list: Vec<usize> = vm_region_by_color
                    .get(index)
                    .unwrap()
                    .iter()
                    .map(|region| region.free)
                    .collect();
                let color_free = free_list.iter().sum::<usize>();
                free_pages += color_free;
                // here, we only use color and free to record a color's free page num
                color2pages.push(ColorMemRegion::new(1 << index, 0, color_free, 0));
                index += 1;
            }
        }
        // if free pages not satisfy, return error
        if free_pages < page_num {
            return Err(());
        }
        // free pages ascending order (small->large)
        // if equals, color ascending order
        color2pages.sort_by(|a, b| {
            if a.free.ne(&b.free) {
                a.free.cmp(&b.free)
            } else {
                a.color.cmp(&b.color)
            }
        });
        // determine to alloc how many pages in a color
        // **Greedy**, because color2pages ascending order by free pages
        let mut remaining_pages = page_num;
        for region in color2pages.iter_mut() {
            let color_size = remaining_pages / count;
            let remainder = remaining_pages % count;
            if region.free > color_size {
                region.free = usize::min(region.free, color_size + remainder);
            }
            remaining_pages -= region.free;
        }
        assert_eq!(remaining_pages, 0);
        color2pages
    };
    let mut vm_regions = vec![];

    for region in color2pages.iter() {
        let color = region.color;
        let size = region.free;
        let color_region_list = vm_region_by_color.get_mut(color).unwrap();

        let mut tmp = vec![];
        for exist_region in color_region_list.iter_mut() {
            if exist_region.free >= size {
                let start_addr = exist_region.base;
                if exist_region.count > size {
                    tmp.push(ColorMemRegion::new(
                        color,
                        start_addr + size * exist_region.step,
                        exist_region.count - size,
                        exist_region.step,
                    ));
                    exist_region.count = size;
                }
                // mark free to 0 means this region is allocated
                exist_region.free = 0;
                vm_regions.push(ColorMemRegion::new(color, start_addr, size, exist_region.step));
                break;
            }
        }
        color_region_list.append(&mut tmp);
    }

    Ok(vm_regions)
}

fn mem_color_region_free(vm_region: &ColorMemRegion) {
    info!(
        "free {:#x}b from {:#x} in color {}, step {}",
        vm_region.count * PAGE_SIZE,
        vm_region.base,
        vm_region.color,
        vm_region.step
    );
    let mut vm_region_by_color = VM_REGION_BY_COLOR.lock();
    let color_region_list = vm_region_by_color.get_mut(vm_region.color).unwrap();
    // free mem region
    let mut free_idx = None;
    for (idx, exist_region) in color_region_list.iter_mut().enumerate() {
        if exist_region.base == vm_region.base && exist_region.free == 0 {
            exist_region.free += vm_region.count;
            free_idx = Some(idx);
            break;
        }
    }
    // merge
    while let Some(merge_idx) = free_idx {
        free_idx = None;
        let tmp = color_region_list.get(merge_idx).unwrap().clone();
        for (idx, exist_region) in color_region_list.iter_mut().enumerate() {
            if exist_region.free != 0 {
                if exist_region.left_neighbor(&tmp) {
                    exist_region.count += tmp.count;
                    exist_region.free += tmp.count;
                    free_idx = Some(if idx < merge_idx { idx } else { idx - 1 });
                    color_region_list.remove(merge_idx);
                    break;
                } else if tmp.left_neighbor(exist_region) {
                    let count = exist_region.count;
                    let mut_tmp = color_region_list.get_mut(merge_idx).unwrap();
                    mut_tmp.count += count;
                    mut_tmp.free += count;
                    free_idx = Some(if merge_idx < idx { merge_idx } else { merge_idx - 1 });
                    color_region_list.remove(idx);
                    break;
                }
            }
        }
    }
}

// TODO: get color region freeing information from pagetable or from self-defined structure?
pub fn mem_vm_color_region_free(vm_regions: &Vec<ColorMemRegion>) {
    for region in vm_regions.iter() {
        mem_color_region_free(region);
    }
}

fn mem_vm_region_init_by_colors() {
    if PLAT_DESC.mem_desc.region_num - 1 > TOTAL_MEM_REGION_MAX {
        panic!("Platform memory regions overrun!");
    } else if PLAT_DESC.mem_desc.region_num == 0 {
        panic!("Platform Vm Memory Regions Overrun!");
    }

    if PLAT_DESC.mem_desc.region_num <= 1 {
        panic!("Platform has no VM memory region!");
    }

    let mut vm_region_by_color = VM_REGION_BY_COLOR.lock();
    let cpu_cache_info = CPU_CACHE.get().unwrap().lock();
    let last_level = cpu_cache_info.min_share_level;
    let num_colors = cpu_cache_info.info_list[last_level - 1].num_colors();
    for _ in 0..num_colors {
        vm_region_by_color.push(Vec::<ColorMemRegion>::new());
    }

    let vm_region_num = PLAT_DESC.mem_desc.region_num;

    let step = num_colors * PAGE_SIZE;
    // region[0] is used for hypervisor memory heap
    for i in 1..vm_region_num {
        let plat_mem_region_base = PLAT_DESC.mem_desc.regions[i].base;
        let plat_mem_region_size = PLAT_DESC.mem_desc.regions[i].size;
        if plat_mem_region_size == 0 {
            println!("PLAT_DESC.mem_desc.regions[{}] is empty.", i);
            continue;
        }
        for color in 0..num_colors {
            let base = plat_mem_region_base & (color << PAGE_SHIFT);
            let count = plat_mem_region_size / step;
            if count > 0 {
                let region = ColorMemRegion::new(color, base, count, step);
                vm_region_by_color.get_mut(color).unwrap().push(region);
            }
        }
    }

    println!("mem_vm_region_init_by_colors:");
    // for (color, color_region_list) in vm_region_by_color.iter().enumerate() {
    for color in 0..num_colors {
        let color_region_list = vm_region_by_color.get(color).unwrap();
        let pages = color_region_list.iter().map(|x| x.count).sum::<usize>();
        println!(
            "  Color {:#02x}: {} regions, total {} pages",
            color,
            color_region_list.len(),
            pages,
        );
    }
}

pub enum AddreSpaceType {
    Hypervisor = 0,
    VM = 1,
    HypervisorCopy = 2,
}

pub struct AddrSpace {
    pub pt: Option<PageTable>,
    pub as_type: AddreSpaceType,
    pub colors: usize,
}

impl AddrSpace {
    pub const fn new() -> Self {
        Self {
            pt: None,
            as_type: AddreSpaceType::VM,
            colors: 0,
        }
    }
}
