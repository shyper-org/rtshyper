pub fn mem_init() {
    mem_heap_region_init();
    mem_vm_region_init();
    println!("Mem init ok");
}

fn mem_heap_region_init() {}

fn mem_vm_region_init() {}
