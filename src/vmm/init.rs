use crate::arch::PageTable;
use crate::arch::PTE_S2_NORMAL;
use crate::config::{VmCpuConfig, VmMemoryConfig, VmImageConfig, DEF_VM_CONFIG_TABLE};
use crate::kernel::VM_LIST;
use crate::kernel::{
    cpu_assigned, cpu_id, cpu_vcpu_pool_size, set_active_vcpu, set_cpu_assign, CPU,
};
use crate::kernel::{mem_page_alloc, mem_vm_region_alloc, vcpu_pool_append, vcpu_pool_init};
use crate::kernel::{Vm, VmInner};
use crate::lib::barrier;
use crate::mm::PageFrame;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

fn vmm_init_memory(config: &VmMemoryConfig, vm: Vm) -> bool {
    let result = mem_page_alloc();
    let mut vm_id = 0;

    if let Ok(pt_dir_frame) = result {
        let vm_inner_lock = vm.inner();
        let mut vm_inner = vm_inner_lock.lock();

        vm_id = vm_inner.id;
        vm_inner.pt = Some(PageTable::new(pt_dir_frame));
        vm_inner.mem_region_num = config.num as usize;
    } else {
        println!("vmm_init_memory: page alloc failed");
        return false;
    }

    for i in 0..config.num as usize {
        let vm_region = &config.region.as_ref().unwrap()[i];
        let pa = mem_vm_region_alloc(vm_region.length);

        if pa == 0 {
            println!("vmm_init_memory: vm memory region is not large enough");
            return false;
        }

        println!(
            "VM {} memory region: ipa=<0x{:x}>, pa=<0x{:x}>, size=<0x{:x}>",
            vm_id, vm_region.ipa_start, pa, vm_region.length
        );

        let vm_inner_lock = vm.inner();
        let mut vm_inner = vm_inner_lock.lock();

        match &vm_inner.pt {
            Some(pt) => pt.pt_map_range(vm_region.ipa_start, vm_region.length, pa, PTE_S2_NORMAL),
            None => {
                println!("vmm_inner_memory: VM page table is null!");
                return false;
            }
        }

        if vm_inner.pa_region.is_none() {
            use crate::kernel::{VmPa, VM_MEM_REGION_MAX};
            let mut pa_region = [
                VmPa::default(),
                VmPa::default(),
                VmPa::default(),
                VmPa::default(),
            ];
            pa_region[i].pa_start = pa;
            pa_region[i].pa_length = vm_region.length;
            pa_region[i].offset = vm_region.ipa_start as isize - pa as isize;
            vm_inner.pa_region = Some(pa_region);
        } else {
            let pa_region = vm_inner.pa_region.as_mut().unwrap();
            pa_region[i].pa_start = pa;
            pa_region[i].pa_length = vm_region.length;
            pa_region[i].offset = vm_region.ipa_start as isize - pa as isize;
        }
    }

    true
}

fn vmm_load_image(filename: &str, load_ipa: usize, vm: Vm) {
    use crate::lib::{fs_read_to_mem, fs_file_size};
    println!("filename: {}, load_ipa 0x{:x}", filename, load_ipa);
    let size = fs_file_size(filename);
    if size == 0 {
        println!("vmm_load_image: file {:#} is not exist", filename);
    }
    println!("file size is {}", size);
    let config = vm.config();
    for i in 0..config.memory.num {
        let region = config.memory.region.as_ref().unwrap();
        if load_ipa < region[i as usize].ipa_start  
            || load_ipa + size > region[i as usize].ipa_start + region[i as usize].length {
            continue;
        }

        // TODO: vmm_load_image
    }
}

fn vmm_init_image(config: &VmImageConfig, vm: Vm) -> bool {
    if config.kernel_name.is_none() {
        println!("vmm_init_image: filename is missed");
        return false;
    }

    if config.kernel_load_ipa == 0 {
        println!("vmm_init_image: kernel load ipa is null");
        return false;
    }

    vm.set_entry_point(config.kernel_entry_point);
    // TODO: vmm_load_image
    vmm_load_image(config.kernel_name.unwrap(), config.kernel_load_ipa, vm.clone());

    // PLATFORM QEMU
    // END PLATFORM

    true
}

use crate::board::PLATFORM_VCPU_NUM_MAX;
use crate::kernel::Vcpu;
fn vmm_init_cpu(config: &VmCpuConfig, vm_arc: &Vm) -> bool {
    let vm_lock = vm_arc.inner();

    for i in 0..config.num {
        use crate::kernel::vcpu_alloc;
        if let Some(vcpu_arc_mutex) = vcpu_alloc() {
            let mut vm = vm_lock.lock();
            let mut vcpu = vcpu_arc_mutex.lock();
            vm.vcpu_list.push(vcpu_arc_mutex.clone());
            drop(vm);
            crate::kernel::vcpu_init(vm_arc, &mut *vcpu, i);
        } else {
            println!("failed to allocte vcpu");
            return false;
        }
    }

    // remain to be init when assigning vcpu
    let mut vm = vm_lock.lock();
    vm.cpu_num = 0;
    vm.ncpu = 0;
    println!(
        "VM {} init cpu: cores=<{}>, allocat_bits=<0b{:b}>",
        vm.id, config.num, config.allocate_bitmap
    );

    true
}

struct VmAssignment {
    has_master: bool,
    cpu_num: usize,
    cpus: usize,
}

impl VmAssignment {
    fn default() -> VmAssignment {
        VmAssignment {
            has_master: false,
            cpu_num: 0,
            cpus: 0,
        }
    }
}

use crate::config::VmConfigEntry;
use crate::kernel::VM_NUM_MAX;
fn vmm_setup_config(config: Arc<VmConfigEntry>, vm: Vm) {
    let cpu_id = cpu_id();

    if cpu_id == 0 {
        if vm.vm_id() >= VM_NUM_MAX {
            panic!("vmm_setup_config: out of vm");
        }
        if !vmm_init_memory(&config.memory, vm.clone()) {
            panic!("vmm_setup_config: vmm_init_memory failed");
        }

        if !vmm_init_image(&config.image, vm.clone()) {
            panic!("vmm_setup_config: vmm_init_image failed");
        }
    }
}

static VM_ASSIGN: Mutex<Vec<Mutex<VmAssignment>>> = Mutex::new(Vec::new());

use crate::kernel::VM_IF_LIST;
fn vmm_assign_vcpu() {
    vcpu_pool_init();

    let cpu_id = cpu_id();
    set_cpu_assign(false);
    let def_vm_config = DEF_VM_CONFIG_TABLE.lock();
    let vm_num = def_vm_config.vm_num;
    drop(def_vm_config);

    if (cpu_id == 0) {
        let mut vm_assign_list = VM_ASSIGN.lock();
        for i in 0..vm_num {
            vm_assign_list.push(Mutex::new(VmAssignment::default()));
        }
    }
    barrier();

    for i in 0..vm_num {
        let vm_list = VM_LIST.lock();
        let vm = vm_list[i].clone();

        drop(vm_list);
        let vm_inner_lock = vm.inner();
        let vm_inner = vm_inner_lock.lock();
        let vm_id = vm_inner.id;

        let config = vm_inner.config.as_ref().unwrap();

        if (config.cpu.allocate_bitmap & (1 << cpu_id)) != 0 {
            let vm_assign_list = VM_ASSIGN.lock();
            let mut vm_assigned = vm_assign_list[i].lock();
            let cfg_master = config.cpu.master as usize;
            let cfg_cpu_num = config.cpu.num;

            if cpu_id == cfg_master
                || (!vm_assigned.has_master && vm_assigned.cpu_num == cfg_cpu_num - 1)
            {
                let vcpu = vm_inner.vcpu_list[0].clone();
                let vcpu_inner = vcpu.lock();
                let vcpu_id = vcpu_inner.id;
                drop(vcpu_inner);

                if !vcpu_pool_append(vcpu) {
                    panic!("core {} too many vcpu", cpu_id);
                }
                // TODO: vm_if_list.master_vcpu_id
                let mut vm_if = VM_IF_LIST[i].lock();
                vm_if.master_vcpu_id = cpu_id;

                vm_assigned.has_master = true;
                vm_assigned.cpu_num += 1;
                vm_assigned.cpus |= 1 << cpu_id;
                set_cpu_assign(true);
                println!(
                    "* Core {} is assigned => vm {}, vcpu {}",
                    cpu_id, vm_id, vcpu_id
                );
                // The remain core become secondary vcpu
            } else if vm_assigned.cpu_num < cfg_cpu_num {
                let mut trgt_id = cfg_cpu_num - vm_assigned.cpu_num - 1;
                if vm_assigned.has_master {
                    trgt_id += 1;
                }

                let vcpu = vm_inner.vcpu_list[trgt_id].clone();
                let vcpu_inner = vcpu.lock();
                let vcpu_id = vcpu_inner.id;
                drop(vcpu_inner);

                if !vcpu_pool_append(vcpu) {
                    panic!("core {} too many vcpu", cpu_id);
                }
                set_cpu_assign(true);
                vm_assigned.cpu_num += 1;
                vm_assigned.cpus |= 1 << cpu_id;
                println!(
                    "* Core {} is assigned => vm {}, vcpu {}",
                    cpu_id, vm_id, vcpu_id
                );
            }
        }
    }
    barrier();

    if cpu_assigned() {
        if let Some(vcpu_pool) = unsafe { &mut CPU.vcpu_pool } {
            for i in 0..vcpu_pool.content.len() {
                let vcpu_arc = vcpu_pool.content[i].vcpu.clone();
                let mut vcpu = vcpu_arc.lock();
                vcpu.phys_id = cpu_id;
                let vm_id = vcpu.vm_id();

                let vm_assign_list = VM_ASSIGN.lock();
                let mut vm_assigned = vm_assign_list[vm_id].lock();
                let vm_list = VM_LIST.lock();
                let vm = vm_list[vm_id].clone();
                drop(vm_list);
                vm.set_ncpu(vm_assigned.cpus);
                vm.set_cpu_num(vm_assigned.cpu_num);
            }
        }
        let size = cpu_vcpu_pool_size();
        set_active_vcpu(size - 1);
    }
    barrier();
}

pub fn vmm_init() {
    barrier();

    if cpu_id() == 0 {
        super::vmm_init_config();

        use crate::config::{VmConfigTable, DEF_VM_CONFIG_TABLE};
        let vm_cfg_table = DEF_VM_CONFIG_TABLE.lock();
        let vm_num = vm_cfg_table.vm_num;

        for i in 0..vm_num {
            let mut vm_list = VM_LIST.lock();
            let vm = Vm::new(i);
            vm_list.push(vm);

            let vm_arc = vm_list[i].inner();
            let mut vm = vm_arc.lock();

            vm.config = Some(vm_cfg_table.entries[i].clone());
            drop(vm);

            vmm_init_cpu(&vm_cfg_table.entries[i].cpu, &vm_list[i]);
        }
        drop(vm_cfg_table);
    }

    barrier();
    vmm_assign_vcpu();
    let vm_cfg_table = DEF_VM_CONFIG_TABLE.lock();
    let vm_num = vm_cfg_table.vm_num;

    for i in 0..vm_num {
        let config = vm_cfg_table.entries[i].clone();
        let mut vm_list = VM_LIST.lock();
        let vm = vm_list[i].clone();

        // TODO: vmm_setup_config
        vmm_setup_config(config, vm);

        // TODO: vmm_setup_contact_config
    }
    drop(vm_cfg_table);

    if cpu_id() == 0 {
        println!("Sybilla Hypervisor init ok\n\nStart booting VMs ...");
    }

    barrier();
}
