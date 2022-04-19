use alloc::vec::Vec;

use crate::arch::{emu_intc_handler, emu_intc_init, partial_passthrough_intc_handler, partial_passthrough_intc_init};
use crate::arch::PAGE_SIZE;
use crate::arch::PTE_S2_DEVICE;
use crate::arch::PTE_S2_NORMAL;
use crate::board::*;
use crate::config::{vm_cfg_entry, vm_type};
use crate::device::{emu_register_dev, emu_virtio_mmio_handler, emu_virtio_mmio_init};
use crate::device::create_fdt;
use crate::device::EmuDeviceType::*;
use crate::kernel::{
    active_vm_id, add_async_used_info, cpu_idle, current_cpu, push_vm, shyper_init, VcpuState, vm_if_init_mem_map,
    VM_IF_LIST, vm_if_set_cpu_id, VmPa, VmType,
};
use crate::kernel::{mem_page_alloc, mem_vm_region_alloc};
use crate::kernel::{vm, Vm};
use crate::kernel::{active_vcpu_id, vcpu_run};
use crate::kernel::interrupt_vm_register;
use crate::kernel::VM_NUM_MAX;
use crate::lib::{barrier, trace};

pub static CPIO_RAMDISK: &'static [u8] = include_bytes!("../../image/rootfs.cpio");

fn vmm_init_memory(vm: Vm) -> bool {
    let result = mem_page_alloc();
    let vm_id = vm.id();
    let config = vm.config();
    let mut vm_mem_size: usize = 0; // size for pages

    if let Ok(pt_dir_frame) = result {
        vm.set_pt(pt_dir_frame);
        vm.set_mem_region_num(config.memory_region().len());
    } else {
        println!("vmm_init_memory: page alloc failed");
        return false;
    }

    for vm_region in config.memory_region() {
        let pa = mem_vm_region_alloc(vm_region.length);
        vm_mem_size += vm_region.length;

        if pa == 0 {
            println!("vmm_init_memory: vm memory region is not large enough");
            return false;
        }

        println!(
            "VM {} memory region: ipa=<0x{:x}>, pa=<0x{:x}>, size=<0x{:x}>",
            vm_id, vm_region.ipa_start, pa, vm_region.length
        );
        vm.pt_map_range(vm_region.ipa_start, vm_region.length, pa, PTE_S2_NORMAL);

        vm.add_region(VmPa {
            pa_start: pa,
            pa_length: vm_region.length,
            offset: vm_region.ipa_start as isize - pa as isize,
        });
    }
    vm_if_init_mem_map(vm_id, (vm_mem_size + PAGE_SIZE - 1) / PAGE_SIZE);

    true
}

fn vmm_load_image(load_ipa: usize, vm: Vm, bin: &[u8]) {
    let size = bin.len();
    let config = vm.config();
    for (idx, region) in config.memory_region().iter().enumerate() {
        if load_ipa < region.ipa_start || load_ipa + size > region.ipa_start + region.length {
            continue;
        }

        let offset = load_ipa - region.ipa_start;
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
        let dst = unsafe { core::slice::from_raw_parts_mut((vm.pa_start(idx) + offset) as *mut u8, size) };
        dst.clone_from_slice(bin);
        // dst = bin;
        return;
    }
    panic!("vmm_load_image: Image config conflicts with memory config");
}

pub fn vmm_init_image(vm: Vm) -> bool {
    // if config.kernel_name.is_none() {
    //     println!("vmm_init_image: filename is missed");
    //     return false;
    // }
    let config = vm.config();

    if config.kernel_load_ipa() == 0 {
        println!("vmm_init_image: kernel load ipa is null");
        return false;
    }

    vm.set_entry_point(config.kernel_entry_point());

    match &config.os_type {
        VmType::VmTBma => {
            vmm_load_image(config.kernel_load_ipa(), vm.clone(), include_bytes!("../../image/BMA"));
            return true;
        }
        VmType::VmTOs => {
            if vm.id() == 0 {
                println!("vm0 load L4T");
                vmm_load_image(config.kernel_load_ipa(), vm.clone(), include_bytes!("../../image/L4T"));
            } else {
                println!("gvm load vanilla");
                vmm_load_image(
                    config.kernel_load_ipa(),
                    vm.clone(),
                    // include_bytes!("../../image/vm1_arch_Image"),
                    include_bytes!("../../image/Image_vanilla"),
                );
            }
        }
    }

    if config.device_tree_load_ipa() != 0 {
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
            let offset = config.device_tree_load_ipa() - config.memory_region()[0].ipa_start;
            unsafe {
                let src = SYSTEM_FDT.get().unwrap();
                let len = src.len();
                let dst = core::slice::from_raw_parts_mut((vm.pa_start(0) + offset) as *mut u8, len);
                dst.clone_from_slice(&src);
            }
            println!("vm {} dtb addr 0x{:x}", vm.id(), vm.pa_start(0) + offset);
            vm.set_dtb((vm.pa_start(0) + offset) as *mut fdt::myctypes::c_void);
        }
    } else {
        println!("VM {} id {} device tree not found", vm.id(), vm.config().name.unwrap());
    }

    if config.ramdisk_load_ipa() != 0 {
        println!("VM {} id {} load ramdisk initrd.gz", vm.id(), vm.config().name.unwrap());
        vmm_load_image(
            config.ramdisk_load_ipa(),
            vm.clone(),
            CPIO_RAMDISK,
            // include_bytes!("../../image/rootfs.cpio"),
        );
    } else {
        println!("VM {} id {} ramdisk not found", vm.id(), vm.config().name.unwrap());
    }
    true
}

fn vmm_init_cpu(vm: Vm) -> bool {
    for i in 0..vm.config().cpu_num() {
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
        vm.config().cpu_num(),
        vm.config().cpu_allocated_bitmap()
    );

    true
}

fn vmm_init_emulated_device(vm: Vm) -> bool {
    let config = vm.config().emulated_device_list();

    for (idx, emu_dev) in config.iter().enumerate() {
        let dev_name;
        match emu_dev.emu_type {
            EmuDeviceTGicd => {
                dev_name = "interrupt controller";
                vm.set_intc_dev_id(idx);
                emu_register_dev(vm.id(), idx, emu_dev.base_ipa, emu_dev.length, emu_intc_handler);
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
                emu_register_dev(vm.id(), idx, emu_dev.base_ipa, emu_dev.length, emu_virtio_mmio_handler);
                if !emu_virtio_mmio_init(vm.clone(), idx, emu_dev.mediated) {
                    return false;
                }
            }
            EmuDeviceTVirtioNet => {
                dev_name = "virtio net";
                emu_register_dev(vm.id(), idx, emu_dev.base_ipa, emu_dev.length, emu_virtio_mmio_handler);
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
                emu_register_dev(vm.id(), idx, emu_dev.base_ipa, emu_dev.length, emu_virtio_mmio_handler);
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

fn vmm_init_passthrough_device(vm: Vm) -> bool {
    for region in vm.config().passthrough_device_regions() {
        vm.pt_map_range(region.ipa, region.length, region.pa, PTE_S2_DEVICE);

        println!(
            "VM {} registers passthrough device: ipa=<0x{:x}>, pa=<0x{:x}>",
            vm.id(),
            region.ipa,
            region.pa,
        );
    }
    for irq in vm.config().passthrough_device_irqs() {
        if !interrupt_vm_register(vm.clone(), irq) {
            return false;
        }
    }
    true
}

pub unsafe fn vmm_setup_fdt(vm: Vm) {
    use fdt::*;
    let config = vm.config();
    match vm.dtb() {
        Some(dtb) => {
            let mut mr = Vec::new();
            for r in config.memory_region() {
                mr.push(region {
                    ipa_start: r.ipa_start as u64,
                    length: r.length as u64,
                });
            }
            fdt_set_memory(dtb, mr.len() as u64, mr.as_ptr(), "memory@90000000\0".as_ptr());
            fdt_add_timer(dtb, 0x8);
            fdt_set_bootcmd(dtb, config.cmdline.as_ptr());
            fdt_set_stdout_path(dtb, "/serial@3100000\0".as_ptr());

            if config.emulated_device_list().len() > 0 {
                for emu_cfg in config.emulated_device_list() {
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
                            fdt_add_virtio(
                                dtb,
                                emu_cfg.name.unwrap().as_ptr(),
                                emu_cfg.irq_id as u32 - 0x20,
                                emu_cfg.base_ipa as u64,
                            );
                        }
                        EmuDeviceTShyper => {
                            fdt_add_vm_service(
                                dtb,
                                emu_cfg.irq_id as u32 - 0x20,
                                emu_cfg.base_ipa as u64,
                                emu_cfg.length as u64,
                            );
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

// This func should run 1 time for each vm.
pub fn vmm_setup_config(vm_id: usize) {
    let vm = match vm(vm_id) {
        Some(vm) => vm,
        None => {
            panic!("vmm_setup_config vm id {} doesn't exist", vm_id);
        }
    };

    let config = match vm_cfg_entry(vm_id) {
        Some(config) => config,
        None => {
            panic!("vmm_setup_config vm id {} config doesn't exist", vm_id);
        }
    };

    println!(
        "vmm_setup_config VM[{}] name {:?} current core {}",
        vm_id,
        config.name.clone().unwrap(),
        current_cpu().id
    );

    if vm_id >= VM_NUM_MAX {
        panic!("vmm_setup_config: out of vm");
    }
    if !vmm_init_memory(vm.clone()) {
        panic!("vmm_setup_config: vmm_init_memory failed");
    }

    if !vmm_init_image(vm.clone()) {
        panic!("vmm_setup_config: vmm_init_image failed");
    }
    if let VmType::VmTOs = config.os_type {
        if vm_id != 0 {
            // Init GVM dtb.
            match create_fdt(config.clone()) {
                Ok(dtb) => {
                    let offset = config.device_tree_load_ipa() - vm.config().memory_region()[0].ipa_start;
                    crate::lib::memcpy_safe((vm.pa_start(0) + offset) as *const u8, dtb.as_ptr(), dtb.len());
                }
                _ => {
                    panic!("vmm_setup_config: create fdt for vm{} fail", vm_id);
                }
            }
        } else {
            unsafe {
                vmm_setup_fdt(vm.clone());
            }
        }
    }

    if !vmm_init_emulated_device(vm.clone()) {
        panic!("vmm_setup_config: vmm_init_emulated_device failed");
    }
    if !vmm_init_passthrough_device(vm.clone()) {
        panic!("vmm_setup_config: vmm_init_passthrough_device failed");
    }
    add_async_used_info(vm_id);
    println!("VM {} id {} init ok", vm.id(), vm.config().name.unwrap());
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

pub fn vmm_assign_vcpu(vm_id: usize) {
    let cpu_id = current_cpu().id;
    if current_cpu().assigned {
        println!("vmm_assign_vcpu vm[{}] cpu {} is assigned", vm_id, cpu_id);
    } else {
        current_cpu().assigned = false;
        // println!("vmm_assign_vcpu vm[{}] cpu {} hasn't been assigned",vm_id,cpu_id);
    }

    // let cpu_config = vm(vm_id).config().cpu;
    let vm = vm(vm_id).unwrap();
    let cfg_master = vm.config().cpu_master();
    let cfg_cpu_num = vm.config().cpu_num();
    let cfg_cpu_allocate_bitmap = vm.config().cpu_allocated_bitmap();

    println!(
        "vmm_assign_vcpu vm[{}] cpu {} cfg_master {}  cfg_cpu_num {} cfg_cpu_allocate_bitmap {:#b}",
        vm_id, cpu_id, cfg_master, cfg_cpu_num, cfg_cpu_allocate_bitmap
    );

    // barrier();
    // Judge if current cpu is allocated.
    if (cfg_cpu_allocate_bitmap & (1 << cpu_id)) != 0 {
        if cpu_id == cfg_master || (!vm.has_master_cpu() && vm.cpu_num() == cfg_cpu_num - 1) {
            let vcpu = match vm.vcpu(0) {
                None => {
                    panic!("core {} vm {} don't have vcpu 0", cpu_id, vm_id);
                }
                Some(vcpu) => vcpu,
            };
            let vcpu_id = vcpu.id();

            // only vm0 vcpu state should set to pend here
            if current_cpu().vcpu_pool().running() == 0 && vm_id == 0 {
                vcpu.set_state(VcpuState::VcpuPend);
                current_cpu().vcpu_pool().add_running();
            }
            if !current_cpu().vcpu_pool().append_vcpu(vcpu) {
                panic!("core {} too many vcpu", cpu_id);
            }

            vm_if_set_cpu_id(vm_id, cpu_id);

            vm.set_has_master_cpu(true);
            vm.set_cpu_num(vm.cpu_num() + 1);
            vm.set_ncpu(vm.ncpu() | 1 << cpu_id);

            current_cpu().assigned = true;
            println!("* Core {} is assigned => vm {}, vcpu {}", cpu_id, vm_id, vcpu_id);
            // The remain core become secondary vcpu
        } else if vm.cpu_num() < cfg_cpu_num {
            let mut trgt_id = cfg_cpu_num - vm.cpu_num() - 1;
            if vm.has_master_cpu() {
                trgt_id += 1;
            }

            let vcpu = match vm.vcpu(trgt_id) {
                None => {
                    panic!("core {} vm {} don't have vcpu {}", cpu_id, vm_id, trgt_id);
                    return;
                }
                Some(vcpu) => vcpu,
            };
            let vcpu_id = vcpu.id();

            if !current_cpu().vcpu_pool().append_vcpu(vcpu) {
                panic!("core {} too many vcpu", cpu_id);
            }

            current_cpu().assigned = true;
            vm.set_cpu_num(vm.cpu_num() + 1);
            vm.set_ncpu(vm.ncpu() | 1 << cpu_id);
            println!("Core {} is assigned => vm {}, vcpu {}", cpu_id, vm_id, vcpu_id);
        }
    }

    if current_cpu().assigned {
        let vcpu_pool = current_cpu().vcpu_pool();
        for i in 0..vcpu_pool.vcpu_num() {
            let vcpu = vcpu_pool.vcpu(i);
            vcpu.set_phys_id(cpu_id);
            if let Some(mvm) = vcpu.vm() {
                if mvm.id() == 0 {
                    vcpu_pool.set_active_vcpu(i);
                    current_cpu().set_active_vcpu(Some(vcpu.clone()));
                }
            }
            println!("core {} i {} vcpu_num {} arch_reset", cpu_id, i, vcpu_pool.vcpu_num());

            vcpu.arch_reset();
        }
    }

    if vm_id != 0 {
        if cfg_cpu_num == vm.cpu_num() {
            vm.set_ready(true);
            println!(
                "vmm_assign_vcpu: core {} vm[{}] is ready cfg_cpu_num {} cur_cpu_num {}",
                cpu_id,
                vm_id,
                cfg_cpu_num,
                vm.cpu_num()
            );
        } else {
            println!(
                "vmm_assign_vcpu: core {} vm[{}] cfg_cpu_num {} cur_cpu_num {}",
                cpu_id,
                vm_id,
                cfg_cpu_num,
                vm.cpu_num()
            );
        }
    }
    // barrier();
}

/* Generate VM structure and push it to VM.
 *
 * @param[in]  vm_id: new added VM id.
 */
pub fn vmm_add_vm(vm_id: usize) {
    println!("vmm_add_vm: add vm {} on cpu {}", vm_id, current_cpu().id);
    if push_vm(vm_id).is_err() {
        return;
    }
    let vm = vm(vm_id).unwrap();
    let vm_cfg = match vm_cfg_entry(vm_id) {
        Some(vm_cfg) => vm_cfg,
        None => {
            println!("vmm_add_vm: failed to find config for vm {}", vm_id);
            return;
        }
    };
    vm.set_config_entry(Some(vm_cfg));

    if !vmm_init_cpu(vm.clone()) {
        println!("vmm_add_vm: vmm_init_cpu failed");
    }
    use crate::kernel::vm_if_set_type;
    vm_if_set_type(vm_id, vm_type(vm_id));
}

pub fn vmm_init() {
    barrier();

    if current_cpu().id == 0 {
        // Set up basic config.
        super::vmm_init_config();
        // Add VM 0
        vmm_add_vm(0);
    }
    barrier();

    println!("core {} init vm 0", current_cpu().id);

    vmm_assign_vcpu(0);
    barrier();

    if current_cpu().id == 0 {
        // TODO: vmm_setup_contact_config
        vmm_setup_config(0);
        println!("Sybilla Hypervisor init ok\n\nStart booting Monitor VM ...");
    }
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
        cpu_idle();
    }
}
