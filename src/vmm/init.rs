use alloc::sync::Arc;
use alloc::vec::Vec;

use spin::Mutex;

use crate::arch::{emu_intc_handler, emu_intc_init, partial_passthrough_intc_handler, partial_passthrough_intc_init};
use crate::arch::PageTable;
use crate::arch::PTE_S2_DEVICE;
use crate::arch::PTE_S2_NORMAL;
use crate::board::*;
use crate::config::{DEF_VM_CONFIG_TABLE, VmCpuConfig, VmImageConfig, VmMemoryConfig};
use crate::config::VmConfigEntry;
use crate::config::VmEmulatedDeviceConfig;
use crate::config::VmPassthroughDeviceConfig;
use crate::device::{emu_register_dev, emu_virtio_mmio_handler, emu_virtio_mmio_init};
use crate::device::create_fdt;
use crate::device::EmuDeviceType::*;
use crate::kernel::{active_vm_id, current_cpu, shyper_init, VcpuState, VM_IF_LIST, vm_if_list_set_cpu_id, VmType};
use crate::kernel::{mem_page_alloc, mem_vm_region_alloc};
use crate::kernel::{Vm, vm, VM_LIST};
use crate::kernel::{active_vcpu_id, vcpu_idle, vcpu_run};
use crate::kernel::interrupt_vm_register;
use crate::kernel::VM_NUM_MAX;
use crate::lib::{barrier, trace};

pub static CPIO_RAMDISK: &'static [u8] = include_bytes!("../../image/rootfs.cpio");

fn vmm_init_memory(config: &VmMemoryConfig, vm: Vm) -> bool {
    let result = mem_page_alloc();
    let vm_id;

    if let Ok(pt_dir_frame) = result {
        let vm_inner_lock = vm.inner();
        let mut vm_inner = vm_inner_lock.lock();

        vm_id = vm_inner.id;
        println!("vm{} pt {:x}", vm_id, pt_dir_frame.pa());
        vm_inner.pt = Some(PageTable::new(pt_dir_frame));
        vm_inner.mem_region_num = config.region.len() as usize;
    } else {
        println!("vmm_init_memory: page alloc failed");
        return false;
    }

    for i in 0..config.region.len() as usize {
        let vm_region = &config.region[i];
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

fn vmm_load_image(load_ipa: usize, vm: Vm, bin: &[u8]) {
    let size = bin.len();
    let config = vm.config();
    for i in 0..config.memory.region.len() {
        let idx = i as usize;
        let region = &config.memory.region;
        if load_ipa < region[idx].ipa_start
            || load_ipa + size > region[idx].ipa_start + region[idx].length
        {
            continue;
        }

        let offset = load_ipa - region[idx].ipa_start;
        println!(
            "VM {} loads kernel: ipa=<0x{:x}>, pa=<0x{:x}>, size=<{}K>",
            vm.id(),
            load_ipa,
            vm.pa_start(idx) + offset,
            size / 1024
        );
        if trace() && vm.pa_start(idx) + offset < 0x1000 {
            panic!("illegal addr {:x}", vm.pa_start(idx) + offset);
        }
        let dst = unsafe {
            core::slice::from_raw_parts_mut((vm.pa_start(idx) + offset) as *mut u8, size)
        };
        dst.clone_from_slice(bin);
        // dst = bin;
        return;
    }
    panic!("vmm_load_image: Image config conflicts with memory config");
}

pub fn vmm_init_image(config: &VmImageConfig, vm: Vm) -> bool {
    // if config.kernel_name.is_none() {
    //     println!("vmm_init_image: filename is missed");
    //     return false;
    // }

    if config.kernel_load_ipa == 0 {
        println!("vmm_init_image: kernel load ipa is null");
        return false;
    }

    vm.set_entry_point(config.kernel_entry_point);

    match &vm.config().os_type {
        VmType::VmTBma => {
            vmm_load_image(
                config.kernel_load_ipa,
                vm.clone(),
                include_bytes!("../../image/BMA"),
            );
            return true;
        }
        VmType::VmTOs => {
            if vm.id() == 0 {
                println!("vm0 load L4T");
                vmm_load_image(
                    config.kernel_load_ipa,
                    vm.clone(),
                    include_bytes!("../../image/L4T"),
                );
            } else {
                println!("gvm load vanilla");
                vmm_load_image(
                    config.kernel_load_ipa,
                    vm.clone(),
                    // include_bytes!("../../image/vm1_arch_Image"),
                    include_bytes!("../../image/Image_vanilla"),
                );
            }
        }
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
                let offset = config.device_tree_load_ipa
                    - vm.config().memory.region[0].ipa_start;
                unsafe {
                    let src = SYSTEM_FDT.get().unwrap();
                    let len = src.len();
                    let dst =
                        core::slice::from_raw_parts_mut((vm.pa_start(0) + offset) as *mut u8, len);
                    dst.clone_from_slice(&src);
                }
                println!("vm {} dtb addr 0x{:x}", vm.id(), vm.pa_start(0) + offset);
                vm.set_dtb((vm.pa_start(0) + offset) as *mut fdt::myctypes::c_void);
            }
    } else {
        println!(
            "VM {} id {} device tree not found",
            vm.id(),
            vm.config().name.unwrap()
        );
    }

    if config.ramdisk_load_ipa != 0 {
        println!(
            "VM {} id {} load ramdisk initrd.gz",
            vm.id(),
            vm.config().name.unwrap()
        );
        vmm_load_image(
            config.ramdisk_load_ipa,
            vm.clone(),
            CPIO_RAMDISK,
            // include_bytes!("../../image/rootfs.cpio"),
        );
    } else {
        println!(
            "VM {} id {} ramdisk not found",
            vm.id(),
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
        vm.id(),
        config.num,
        config.allocate_bitmap
    );

    true
}

fn vmm_init_emulated_device(config: &Option<Vec<VmEmulatedDeviceConfig>>, vm: Vm) -> bool {
    if config.is_none() {
        println!(
            "vmm_init_emulated_device: VM {} emu config is NULL",
            vm.id()
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
                    vm.id(),
                    idx,
                    emu_dev.base_ipa,
                    emu_dev.length,
                    emu_intc_handler,
                );
                emu_intc_init(vm.clone(), idx);
            }
            EmuDeviceTGPPT => {
                dev_name = "partial passthrough interrupt controller";
                vm.set_intc_dev_id(idx);
                emu_register_dev(
                    vm.id(),
                    idx,
                    emu_dev.base_ipa,
                    emu_dev.length,
                    partial_passthrough_intc_handler,
                );
                partial_passthrough_intc_init(vm.clone());
            }
            EmuDeviceTVirtioBlk => {
                dev_name = "virtio block";
                emu_register_dev(
                    vm.id(),
                    idx,
                    emu_dev.base_ipa,
                    emu_dev.length,
                    emu_virtio_mmio_handler,
                );
                if !emu_virtio_mmio_init(vm.clone(), idx, emu_dev.mediated) {
                    return false;
                }
            }
            EmuDeviceTVirtioNet => {
                dev_name = "virtio net";
                emu_register_dev(
                    vm.id(),
                    idx,
                    emu_dev.base_ipa,
                    emu_dev.length,
                    emu_virtio_mmio_handler,
                );
                if !emu_virtio_mmio_init(vm.clone(), idx, emu_dev.mediated) {
                    return false;
                }
                let mut vm_if_list = VM_IF_LIST[vm.id()].lock();
                for i in 0..6 {
                    vm_if_list.mac[i] = emu_dev.cfg_list[i] as u8;
                }
                drop(vm_if_list);
            }
            EmuDeviceTVirtioConsole => {
                dev_name = "virtio console";
                emu_register_dev(
                    vm.id(),
                    idx,
                    emu_dev.base_ipa,
                    emu_dev.length,
                    emu_virtio_mmio_handler,
                );
                if !emu_virtio_mmio_init(vm.clone(), idx, emu_dev.mediated) {
                    return false;
                }
            }
            EmuDeviceTShyper => {
                dev_name = "shyper";
                if !shyper_init(vm.clone(), emu_dev.base_ipa, emu_dev.length) {
                    return false;
                }
            }
            _ => {
                println!("vmm_init_emulated_device: unknown emulated device");
                return false;
            }
        }
        println!(
            "VM {} registers emulated device: id=<{}>, name=\"{}\", ipa=<0x{:x}>",
            vm.id(),
            idx,
            dev_name,
            emu_dev.base_ipa
        );
    }

    true
}

fn vmm_init_passthrough_device(config: &Option<VmPassthroughDeviceConfig>, vm: Vm) -> bool {
    match config {
        Some(cfg) => {
            for region in &cfg.regions {
                vm.pt_map_range(region.ipa, region.length, region.pa, PTE_S2_DEVICE);

                println!(
                    "VM {} registers passthrough device: ipa=<0x{:x}>, pa=<0x{:x}>",
                    vm.id(),
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
                vm.id()
            );
            return true;
        }
    }

    true
}


pub unsafe fn vmm_setup_fdt(config: Arc<VmConfigEntry>, vm: Vm) {
    use fdt::*;
    match vm.dtb() {
        Some(dtb) => {
            let mut mr = Vec::new();
            for r in &config.memory.region {
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

            if config.vm_emu_dev_confg.is_some() {
                for emu_cfg in config.vm_emu_dev_confg.as_ref().unwrap() {
                    match emu_cfg.emu_type {
                        EmuDeviceTGicd => {
                            fdt_setup_gic(
                                dtb,
                                PLATFORM_GICD_BASE as u64,
                                PLATFORM_GICC_BASE as u64,
                                emu_cfg.name.unwrap().as_ptr(),
                            );
                        }
                        EmuDeviceTVirtioNet | EmuDeviceTVirtioConsole => {
                            fdt_add_virtio(dtb, emu_cfg.name.unwrap().as_ptr(), emu_cfg.irq_id as u32 - 0x20, emu_cfg.base_ipa as u64);
                        }
                        EmuDeviceTShyper => {
                            fdt_add_vm_service(dtb, emu_cfg.irq_id as u32 - 0x20, emu_cfg.base_ipa as u64, emu_cfg.length as u64);
                        }
                        _ => {
                            todo!();
                        }
                    }
                }
            }
        }
        None => {}
    }
}

fn vmm_setup_config(config: Arc<VmConfigEntry>, vm: Vm) {
    let cpu_id = current_cpu().id;
    let vm_id = vm.id();

    if cpu_id == 0 {
        if vm_id >= VM_NUM_MAX {
            panic!("vmm_setup_config: out of vm");
        }
        if !vmm_init_memory(&config.memory, vm.clone()) {
            panic!("vmm_setup_config: vmm_init_memory failed");
        }

        if !vmm_init_image(&config.image, vm.clone()) {
            panic!("vmm_setup_config: vmm_init_image failed");
        }
        if let VmType::VmTOs = config.os_type {
            if vm_id != 0 {
                // init gvm dtb
                match create_fdt(config.clone()) {
                    Ok(dtb) => {
                        let offset = config.image.device_tree_load_ipa
                            - vm.config().memory.region[0].ipa_start;
                        crate::lib::memcpy_safe(
                            (vm.pa_start(0) + offset) as *const u8,
                            dtb.as_ptr(),
                            dtb.len(),
                        );
                    }
                    _ => {
                        panic!("vmm_setup_config: create fdt for vm{} fail", vm_id);
                    }
                }
            } else {
                unsafe {
                    vmm_setup_fdt(config.clone(), vm.clone());
                }
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
        println!("VM {} id {} init ok", vm.id(), vm.config().name.unwrap());
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
    let cpu_id = current_cpu().id;
    let assigned = false;
    current_cpu().assigned = assigned;
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

                // only vm0 vcpu state should set to pend here
                if current_cpu().vcpu_pool().running() == 0 && i == 0 {
                    vcpu.set_state(VcpuState::VcpuPend);
                    current_cpu().vcpu_pool().add_running();
                }
                if !current_cpu().vcpu_pool().append_vcpu(vcpu) {
                    panic!("core {} too many vcpu", cpu_id);
                }
                // println!("core {} after vcpu_pool_append0", cpu_id);
                vm_if_list_set_cpu_id(i, cpu_id);

                vm_assigned.has_master = true;
                vm_assigned.cpu_num += 1;
                vm_assigned.cpus |= 1 << cpu_id;
                let assigned = true;
                current_cpu().assigned = assigned;
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

                // println!("core {} before vcpu_pool_append1", cpu_id);
                if !current_cpu().vcpu_pool().append_vcpu(vcpu) {
                    panic!("core {} too many vcpu", cpu_id);
                }
                // println!("core {} after vcpu_pool_append1", cpu_id);
                let assigned = true;
                current_cpu().assigned = assigned;
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
    if current_cpu().assigned {
        let vcpu_pool = current_cpu().vcpu_pool();
        for i in 0..vcpu_pool.vcpu_num() {
            let vcpu = vcpu_pool.vcpu(i);
            vcpu.set_phys_id(cpu_id);
            let vm_id = vcpu.vm_id();

            let vm_assign_list = VM_ASSIGN.lock();
            let vm_assigned = vm_assign_list[vm_id].lock();
            let vm = vm(vm_id);
            vm.set_ncpu(vm_assigned.cpus);
            vm.set_cpu_num(vm_assigned.cpu_num);

            if let Some(mvm) = vcpu.vm() {
                if mvm.id() == 0 {
                    current_cpu().set_active_vcpu(vcpu.clone());
                    println!("vm0 elr {:x}", vcpu.elr());
                }
            }

            vcpu.arch_reset();
        }
    }
    barrier();

    if cpu_id == 0 {
        let mut vm_assign_list = VM_ASSIGN.lock();
        vm_assign_list.clear();
    }
}

pub fn vmm_init() {
    barrier();

    if current_cpu().id == 0 {
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

    if current_cpu().id == 0 {
        println!("Sybilla Hypervisor init ok\n\nStart booting VMs ...");
    }

    barrier();
}

pub fn vmm_boot() {
    if current_cpu().assigned && active_vcpu_id() == 0 {
        // let vcpu_pool = current_cpu().vcpu_pool.as_ref().unwrap();
        let vcpu_pool = current_cpu().vcpu_pool();
        for i in 0..vcpu_pool.vcpu_num() {
            let vcpu = vcpu_pool.vcpu(i);
            if vcpu.vm_id() == active_vm_id() {
                // Before running, every vcpu need to reset context state
                vcpu.reset_context();
            }
        }
        println!("Core {} start running", current_cpu().id);
        vcpu_run();

        // // test
        // for i in 0..0x1000 {}
        // println!("send ipi");
        // crate::kernel::interrupt_cpu_ipi_send(4, 1);
        // // end test
    } else {
        // If there is no available vm(vcpu), just go idle
        println!("Core {} idle", current_cpu().id);
        vcpu_idle();
    }
}
