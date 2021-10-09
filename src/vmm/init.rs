use crate::arch::PageTable;
use crate::arch::PTE_S2_NORMAL;
use crate::config::{VmCpuConfig, VmImageConfig, VmMemoryConfig, DEF_VM_CONFIG_TABLE};
use crate::kernel::Vm;
use crate::kernel::{
    cpu_assigned, cpu_id, cpu_vcpu_pool_size, set_active_vcpu, set_cpu_assign,
    vm_if_list_set_cpu_id, CPU, VM_IF_LIST,
};
use crate::kernel::{mem_page_alloc, mem_vm_region_alloc, vcpu_pool_append, vcpu_pool_init};
use crate::kernel::{vm, VM_LIST};
use crate::lib::barrier;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

fn vmm_init_memory(config: &VmMemoryConfig, vm: Vm) -> bool {
    let result = mem_page_alloc();
    let vm_id;

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
            use crate::kernel::VmPa;
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

fn vmm_load_kernel(load_ipa: usize, vm: Vm) {
    let bin = include_bytes!("../../image/Image");
    let size = bin.len();
    let config = vm.config();
    for i in 0..config.memory.num {
        let idx = i as usize;
        let region = config.memory.region.as_ref().unwrap();
        if load_ipa < region[idx].ipa_start
            || load_ipa + size > region[idx].ipa_start + region[idx].length
        {
            continue;
        }

        let offset = load_ipa - region[idx].ipa_start;
        println!(
            "VM {} loads kernel: ipa=<0x{:x}>, pa=<0x{:x}>, size=<{}K>",
            vm.vm_id(),
            load_ipa,
            vm.pa_start(idx) + offset,
            size / 1024
        );
        let dst = unsafe {
            core::slice::from_raw_parts_mut((vm.pa_start(idx) + offset) as *mut u8, size)
        };
        dst.clone_from_slice(bin);
        // dst = bin;
        return;
    }
    panic!("vmm_load_image: Image config conflicts with memory config");
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
    vmm_load_kernel(config.kernel_load_ipa, vm.clone());

    match &vm.config().os_type {
        crate::kernel::VmType::VmTBma => return true,
        _ => {}
    }

    if config.device_tree_load_ipa != 0 {
        use crate::SYSTEM_FDT;

        // PLATFORM
        // #[cfg(feature = "qemu")]
        // vmm_load_image(
        //     config.device_tree_filename.unwrap(),
        //     config.device_tree_load_ipa,
        //     vm.clone(),
        // );
        // // END QEMU
        #[cfg(feature = "tx2")]
        {
            use crate::arch::PAGE_SIZE;
            let offset = config.device_tree_load_ipa
                - vm.config().memory.region.as_ref().unwrap()[0].ipa_start;
            unsafe {
                let src = SYSTEM_FDT.get().unwrap();
                let len = src.len();
                let dst =
                    core::slice::from_raw_parts_mut((vm.pa_start(0) + offset) as *mut u8, len);
                dst.clone_from_slice(&src);
            }
            vm.set_dtb((vm.pa_start(0) + offset) as *mut fdt::myctypes::c_void);
        }
    } else {
        println!(
            "VM {} id {} device tree not found",
            vm.vm_id(),
            vm.config().name.unwrap()
        );
    }

    if config.ramdisk_load_ipa != 0 {
        // vmm_load_image(
        //     config.ramdisk_filename.unwrap(),
        //     config.ramdisk_load_ipa,
        //     vm.clone(),
        // );
    } else {
        println!(
            "VM {} id {} ramdisk not found",
            vm.vm_id(),
            vm.config().name.unwrap()
        );
    }
    true
}

fn vmm_init_cpu(config: &VmCpuConfig, vm: Vm) -> bool {
    for i in 0..config.num {
        use crate::kernel::vcpu_alloc;
        if let Some(vcpu) = vcpu_alloc() {
            vm.push_vcpu(vcpu.clone());
            vcpu.init(vm.clone(), i);
        } else {
            println!("failed to allocte vcpu");
            return false;
        }
    }

    // remain to be init when assigning vcpu
    vm.set_cpu_num(0);
    vm.set_ncpu(0);
    println!(
        "VM {} init cpu: cores=<{}>, allocat_bits=<0b{:b}>",
        vm.vm_id(),
        config.num,
        config.allocate_bitmap
    );

    true
}

use crate::arch::{emu_intc_handler, emu_intc_init};
use crate::config::VmEmulatedDeviceConfig;
use crate::device::EmuDeviceType::*;
use crate::device::{emu_register_dev, emu_virtio_mmio_handler, emu_virtio_mmio_init};
fn vmm_init_emulated_device(config: &Option<Vec<VmEmulatedDeviceConfig>>, vm: Vm) -> bool {
    if config.is_none() {
        println!(
            "vmm_init_emulated_device: VM {} emu config is NULL",
            vm.vm_id()
        );
        return true;
    }

    for (idx, emu_dev) in config.as_ref().unwrap().iter().enumerate() {
        let dev_name;
        match emu_dev.emu_type {
            EmuDeviceTGicd => {
                dev_name = "interrupt controller";
                vm.set_intc_dev_id(idx);
                emu_register_dev(
                    vm.vm_id(),
                    idx,
                    emu_dev.base_ipa,
                    emu_dev.length,
                    emu_intc_handler,
                );
                emu_intc_init(vm.clone(), idx);
            }
            EmuDeviceTVirtioBlk => {
                dev_name = "virtio block";
                emu_register_dev(
                    vm.vm_id(),
                    idx,
                    emu_dev.base_ipa,
                    emu_dev.length,
                    emu_virtio_mmio_handler,
                );
                if !emu_virtio_mmio_init(vm.clone(), idx) {
                    return false;
                }
            }
            EmuDeviceTVirtioNet => {
                dev_name = "virtio net";
                emu_register_dev(
                    vm.vm_id(),
                    idx,
                    emu_dev.base_ipa,
                    emu_dev.length,
                    emu_virtio_mmio_handler,
                );
                if !emu_virtio_mmio_init(vm.clone(), idx) {
                    return false;
                }
                let mut vm_if_list = VM_IF_LIST[vm.vm_id()].lock();
                for i in 0..6 {
                    vm_if_list.mac[i] = emu_dev.cfg_list[i] as u8;
                }
                drop(vm_if_list);
            }
            _ => {
                println!("vmm_init_emulated_device: unknown emulated device");
                return false;
            }
        }
        println!(
            "VM {} registers emulated device: id=<{}>, name=\"{}\", ipa=<0x{:x}>",
            vm.vm_id(),
            idx,
            dev_name,
            emu_dev.base_ipa
        );
    }

    true
}

use crate::arch::PTE_S2_DEVICE;
use crate::config::VmPassthroughDeviceConfig;
use crate::kernel::interrupt_vm_register;
fn vmm_init_passthrough_device(config: &Option<VmPassthroughDeviceConfig>, vm: Vm) -> bool {
    match config {
        Some(cfg) => {
            for region in &cfg.regions {
                vm.pt_map_range(region.ipa, region.length, region.pa, PTE_S2_DEVICE);

                println!(
                    "VM {} registers passthrough device: ipa=<0x{:x}>, pa=<0x{:x}>",
                    vm.vm_id(),
                    region.ipa,
                    region.pa,
                );
            }
            for irq in &cfg.irqs {
                if !interrupt_vm_register(vm.clone(), *irq) {
                    return false;
                }
            }
        }
        None => {
            println!(
                "vmm_init_passthrough_device: VM {} passthrough config is NULL",
                vm.vm_id()
            );
            return true;
        }
    }

    true
}

use crate::config::VmConfigEntry;
use crate::kernel::VM_NUM_MAX;

unsafe fn vmm_setup_fdt(config: Arc<VmConfigEntry>, vm: Vm) {
    use fdt::*;
    match vm.dtb() {
        None => return,
        Some(dtb) => {
            let mut mr = Vec::new();
            for r in config.memory.region.as_ref().unwrap() {
                mr.push(region {
                    ipa_start: r.ipa_start as u64,
                    length: r.length as u64,
                });
            }
            fdt_set_memory(
                dtb,
                mr.len() as u64,
                mr.as_ptr(),
                "memory@90000000\0".as_ptr(),
            );
            fdt_add_timer(dtb, 0x8);
            fdt_set_bootcmd(dtb, config.cmdline.as_ptr());
            fdt_set_stdout_path(dtb, "/serial@3100000\0".as_ptr());
            fdt_setup_gic(
                dtb,
                0x8000000,
                0x8010000,
                "interrupt-controller@8000000\0".as_ptr(),
            );
        }
    }
}

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
        if vm.vm_id() != 0 {
            // init vm1 dtb
            todo!();
        } else {
            unsafe {
                vmm_setup_fdt(config.clone(), vm.clone());
            }
        }
    }

    // barrier

    if cpu_id == 0 {
        if !vmm_init_emulated_device(&config.vm_emu_dev_confg, vm.clone()) {
            panic!("vmm_setup_config: vmm_init_emulated_device failed");
        }
        if !vmm_init_passthrough_device(&config.vm_pt_dev_confg, vm.clone()) {
            panic!("vmm_setup_config: vmm_init_passthrough_device failed");
        }
        println!("VM {} id {} init ok", vm.vm_id(), vm.config().name.unwrap());
    }
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

static VM_ASSIGN: Mutex<Vec<Mutex<VmAssignment>>> = Mutex::new(Vec::new());

fn vmm_assign_vcpu() {
    vcpu_pool_init();

    let cpu_id = cpu_id();
    set_cpu_assign(false);
    let def_vm_config = DEF_VM_CONFIG_TABLE.lock();
    let vm_num = def_vm_config.vm_num;
    drop(def_vm_config);

    if cpu_id == 0 {
        let mut vm_assign_list = VM_ASSIGN.lock();
        for _ in 0..vm_num {
            vm_assign_list.push(Mutex::new(VmAssignment::default()));
        }
    }
    barrier();

    for i in 0..vm_num {
        let vm = vm(i);

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
                let vcpu_id = vcpu.id();

                // println!("core {} before vcpu_pool_append0", cpu_id);
                if !vcpu_pool_append(vcpu) {
                    panic!("core {} too many vcpu", cpu_id);
                }
                // println!("core {} after vcpu_pool_append0", cpu_id);
                vm_if_list_set_cpu_id(i, cpu_id);

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
                let vcpu_id = vcpu.id();

                println!("core {} before vcpu_pool_append1", cpu_id);
                if !vcpu_pool_append(vcpu) {
                    panic!("core {} too many vcpu", cpu_id);
                }
                println!("core {} after vcpu_pool_append1", cpu_id);
                set_cpu_assign(true);
                vm_assigned.cpu_num += 1;
                vm_assigned.cpus |= 1 << cpu_id;
                println!(
                    "Core {} is assigned => vm {}, vcpu {}",
                    cpu_id, vm_id, vcpu_id
                );
            }
        }
    }
    barrier();
    if cpu_assigned() {
        if let Some(vcpu_pool) = unsafe { &mut CPU.vcpu_pool } {
            for i in 0..vcpu_pool.content.len() {
                let vcpu = vcpu_pool.content[i].vcpu.clone();
                vcpu.set_phys_id(cpu_id);
                let vm_id = vcpu.vm_id();

                let vm_assign_list = VM_ASSIGN.lock();
                let vm_assigned = vm_assign_list[vm_id].lock();
                let vm = vm(vm_id);
                vm.set_ncpu(vm_assigned.cpus);
                vm.set_cpu_num(vm_assigned.cpu_num);
            }
        }
        let size = cpu_vcpu_pool_size();
        set_active_vcpu(size - 1);
    }
    barrier();

    if cpu_id == 0 {
        let mut vm_assign_list = VM_ASSIGN.lock();
        vm_assign_list.clear();
    }
}

pub fn vmm_init() {
    barrier();

    if cpu_id() == 0 {
        super::vmm_init_config();

        let vm_cfg_table = DEF_VM_CONFIG_TABLE.lock();
        let vm_num = vm_cfg_table.vm_num;

        for i in 0..vm_num {
            let mut vm_list = VM_LIST.lock();
            let vm = Vm::new(i);
            vm_list.push(vm);

            let vm_arc = vm_list[i].inner();
            let mut vm = vm_arc.lock();

            vm.config = Some(vm_cfg_table.entries[i].clone());
            let vm_type = vm.config.as_ref().unwrap().os_type;
            drop(vm);

            if !vmm_init_cpu(&vm_cfg_table.entries[i].cpu, vm_list[i].clone()) {
                println!("vmm_init: vmm_init_cpu failed");
            }

            use crate::kernel::vm_if_list_set_type;
            vm_if_list_set_type(i, vm_type);
        }
    }
    barrier();
    vmm_assign_vcpu();
    let vm_cfg_table = DEF_VM_CONFIG_TABLE.lock();
    let vm_num = vm_cfg_table.vm_num;

    for i in 0..vm_num {
        let config = vm_cfg_table.entries[i].clone();
        let vm = vm(i);

        vmm_setup_config(config, vm.clone());
        // TODO: vmm_setup_contact_config
    }
    drop(vm_cfg_table);

    if cpu_id() == 0 {
        println!("Sybilla Hypervisor init ok\n\nStart booting VMs ...");
    }

    barrier();
}

use crate::kernel::{active_vcpu_id, cpu_vcpu_pool, vcpu_idle, vcpu_run};
pub fn vmm_boot() {
    if cpu_assigned() {
        if active_vcpu_id() == 0 {
            let vcpu_pool = cpu_vcpu_pool();
            for i in 0..cpu_vcpu_pool_size() {
                let vcpu = vcpu_pool.content[i].vcpu.clone();
                // Before running, every vcpu need to reset context state
                vcpu.reset_state();
            }
            vcpu_run();

            // // test
            // for i in 0..0x1000 {}
            // println!("send ipi");
            // crate::kernel::interrupt_cpu_ipi_send(4, 1);
            // // end test
        } else {
            // if the vcpu is not the master, just go idle and wait for wokening up
            vcpu_idle();
        }
    } else {
        // If there is no available vm(vcpu), just go idle
        println!("Core {} idle", cpu_id());
        vcpu_idle();
    }
}
