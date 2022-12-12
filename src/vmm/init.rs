use alloc::vec::Vec;

use crate::arch::{
    emu_intc_handler, emu_intc_init, emu_smmu_handler, partial_passthrough_intc_handler, partial_passthrough_intc_init,
};
use crate::arch::{PTE_S2_DEVICE, PTE_S2_NORMAL};
use crate::arch::PAGE_SIZE;
use crate::board::*;
use crate::config::vm_cfg_entry;
use crate::device::{emu_register_dev, emu_virtio_mmio_handler, emu_virtio_mmio_init};
use crate::device::create_fdt;
use crate::device::EmuDeviceType::*;
use crate::kernel::{
    add_async_used_info, cpu_idle, current_cpu, iommmu_vm_init, shyper_init, vm_if_init_mem_map, VM_IF_LIST, VmPa,
    VmType, iommu_add_device,
};
use crate::kernel::{mem_page_alloc, mem_vm_region_alloc};
use crate::kernel::{vm, Vm};
use crate::kernel::{active_vcpu_id, vcpu_run};
use crate::kernel::interrupt_vm_register;
use crate::kernel::VM_NUM_MAX;
use crate::lib::trace;

#[cfg(feature = "ramdisk")]
pub static CPIO_RAMDISK: &'static [u8] = include_bytes!("../../image/net_rootfs.cpio");
#[cfg(not(feature = "ramdisk"))]
pub static CPIO_RAMDISK: &'static [u8] = &[];

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
        vm.pt_map_range(vm_region.ipa_start, vm_region.length, pa, PTE_S2_NORMAL, vm_id == 0);

        vm.add_region(VmPa {
            pa_start: pa,
            pa_length: vm_region.length,
            offset: vm_region.ipa_start as isize - pa as isize,
        });
    }
    vm_if_init_mem_map(vm_id, (vm_mem_size + PAGE_SIZE - 1) / PAGE_SIZE);

    true
}

pub fn vmm_load_image(vm: Vm, bin: &[u8]) {
    let size = bin.len();
    let config = vm.config();
    let load_ipa = config.kernel_load_ipa();
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
        return;
    }
    panic!("vmm_load_image: Image config conflicts with memory config");
}

pub fn vmm_init_image(vm: Vm) -> bool {
    let vm_id = vm.id();
    let config = vm.config();

    if config.kernel_load_ipa() == 0 {
        println!("vmm_init_image: kernel load ipa is null");
        return false;
    }

    vm.set_entry_point(config.kernel_entry_point());

    // Only load MVM kernel image "L4T" from binding.
    // Load GVM kernel image from shyper-cli, you may check it for more information.
    if vm.id() == 0 && config.os_type == VmType::VmTOs {
        println!("MVM loading Image");
        #[cfg(feature = "tx2")]
        vmm_load_image(vm.clone(), include_bytes!("../../image/L4T"));
        #[cfg(feature = "pi4")]
        vmm_load_image(vm.clone(), include_bytes!("../../image/Image_pi4_5.4.83_tlb"));
        // vmm_load_image(vm.clone(), include_bytes!("../../image/Image_pi4_5.4.78"));
        // vmm_load_image(vm.clone(), include_bytes!("../../image/Image_pi4"));
    }

    if config.device_tree_load_ipa() != 0 {
        // Init dtb for Linux.
        if vm_id == 0 {
            // Init dtb for MVM.
            use crate::SYSTEM_FDT;
            let offset = config.device_tree_load_ipa() - config.memory_region()[0].ipa_start;
            println!("MVM[{}] dtb addr 0x{:x}", vm_id, vm.pa_start(0) + offset);
            vm.set_dtb((vm.pa_start(0) + offset) as *mut fdt::myctypes::c_void);
            unsafe {
                let src = SYSTEM_FDT.get().unwrap();
                let len = src.len();
                let dst = core::slice::from_raw_parts_mut((vm.pa_start(0) + offset) as *mut u8, len);
                dst.clone_from_slice(&src);
                vmm_setup_fdt(vm.clone());
            }
        } else {
            // Init dtb for GVM.
            match create_fdt(config.clone()) {
                Ok(dtb) => {
                    let offset = config.device_tree_load_ipa() - vm.config().memory_region()[0].ipa_start;
                    println!("GVM[{}] dtb addr 0x{:x}", vm.id(), vm.pa_start(0) + offset);
                    crate::lib::memcpy_safe((vm.pa_start(0) + offset) as *const u8, dtb.as_ptr(), dtb.len());
                }
                _ => {
                    panic!("vmm_setup_config: create fdt for vm{} fail", vm.id());
                }
            }
        }
    } else {
        println!(
            "VM {} id {} device tree load ipa is not set",
            vm_id,
            vm.config().vm_name()
        );
    }

    // ...
    // Todo: support loading ramdisk from MVM shyper-cli.
    // ...
    if config.ramdisk_load_ipa() != 0 {
        println!("VM {} use ramdisk CPIO_RAMDISK", vm_id);
        let offset = config.ramdisk_load_ipa() - config.memory_region()[0].ipa_start;
        let len = CPIO_RAMDISK.len();
        let dst = unsafe { core::slice::from_raw_parts_mut((vm.pa_start(0) + offset) as *mut u8, len) };
        dst.clone_from_slice(CPIO_RAMDISK);
    }

    true
}

fn vmm_init_emulated_device(vm: Vm) -> bool {
    let config = vm.config().emulated_device_list();

    for (idx, emu_dev) in config.iter().enumerate() {
        match emu_dev.emu_type {
            EmuDeviceTGicd => {
                vm.set_intc_dev_id(idx);
                emu_register_dev(
                    EmuDeviceTGicd,
                    vm.id(),
                    idx,
                    emu_dev.base_ipa,
                    emu_dev.length,
                    emu_intc_handler,
                );
                emu_intc_init(vm.clone(), idx);
            }
            EmuDeviceTGPPT => {
                vm.set_intc_dev_id(idx);
                emu_register_dev(
                    EmuDeviceTGPPT,
                    vm.id(),
                    idx,
                    emu_dev.base_ipa,
                    emu_dev.length,
                    partial_passthrough_intc_handler,
                );
                partial_passthrough_intc_init(vm.clone());
            }
            EmuDeviceTVirtioBlk => {
                emu_register_dev(
                    EmuDeviceTVirtioBlk,
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
                emu_register_dev(
                    EmuDeviceTVirtioNet,
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
                emu_register_dev(
                    EmuDeviceTVirtioConsole,
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
            EmuDeviceTIOMMU => {
                emu_register_dev(
                    EmuDeviceTIOMMU,
                    vm.id(),
                    idx,
                    emu_dev.base_ipa,
                    emu_dev.length,
                    emu_smmu_handler,
                );
                if !iommmu_vm_init(vm.clone()) {
                    return false;
                }
            }
            EmuDeviceTShyper => {
                if !shyper_init(vm.clone(), emu_dev.base_ipa, emu_dev.length) {
                    return false;
                }
            }
            _ => {
                warn!("vmm_init_emulated_device: unknown emulated device");
                return false;
            }
        }
        info!(
            "VM {} registers emulated device: id=<{}>, name=\"{}\", ipa=<0x{:x}>",
            vm.id(),
            idx,
            emu_dev.emu_type,
            emu_dev.base_ipa
        );
    }

    true
}

fn vmm_init_passthrough_device(vm: Vm) -> bool {
    for region in vm.config().passthrough_device_regions() {
        if region.dev_property {
            vm.pt_map_range(region.ipa, region.length, region.pa, PTE_S2_DEVICE, true);
        } else {
            vm.pt_map_range(region.ipa, region.length, region.pa, PTE_S2_NORMAL, true);
        }

        debug!(
            "VM {} registers passthrough device: ipa=<0x{:x}>, pa=<0x{:x}>, size=<0x{:x}>, {}",
            vm.id(),
            region.ipa,
            region.pa,
            region.length,
            if region.dev_property { "device" } else { "normal" }
        );
    }
    for irq in vm.config().passthrough_device_irqs() {
        if !interrupt_vm_register(vm.clone(), irq) {
            return false;
        }
    }
    true
}

fn vmm_init_iommu_device(vm: Vm) -> bool {
    for stream_id in vm.config().passthrough_device_stread_ids() {
        if stream_id == 0 {
            break;
        }
        if !iommu_add_device(vm.clone(), stream_id) {
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
            #[cfg(feature = "tx2")]
            fdt_set_memory(dtb, mr.len() as u64, mr.as_ptr(), "memory@90000000\0".as_ptr());
            #[cfg(feature = "pi4")]
            fdt_set_memory(dtb, mr.len() as u64, mr.as_ptr(), "memory@200000\0".as_ptr());
            // FDT+TIMER
            fdt_add_timer(dtb, 0x8);
            // FDT+BOOTCMD
            fdt_set_bootcmd(dtb, config.cmdline.as_ptr());
            #[cfg(feature = "tx2")]
            fdt_set_stdout_path(dtb, "/serial@3100000\0".as_ptr());
            // #[cfg(feature = "pi4")]
            // fdt_set_stdout_path(dtb, "/serial@fe340000\0".as_ptr());

            if config.emulated_device_list().len() > 0 {
                for emu_cfg in config.emulated_device_list() {
                    match emu_cfg.emu_type {
                        EmuDeviceTGicd => {
                            #[cfg(feature = "tx2")]
                            fdt_setup_gic(
                                dtb,
                                PLATFORM_GICD_BASE as u64,
                                PLATFORM_GICC_BASE as u64,
                                emu_cfg.name.unwrap().as_ptr(),
                            );
                            #[cfg(feature = "pi4")]
                                let r = fdt_setup_gic(
                                dtb,
                                (PLATFORM_GICD_BASE | 0xF_0000_0000) as u64,
                                (PLATFORM_GICC_BASE | 0xF_0000_0000) as u64,
                                emu_cfg.name.unwrap().as_ptr(),
                            );
                        }
                        EmuDeviceTVirtioNet | EmuDeviceTVirtioConsole => {
                            #[cfg(feature = "tx2")]
                            fdt_add_virtio(
                                dtb,
                                emu_cfg.name.unwrap().as_ptr(),
                                emu_cfg.irq_id as u32 - 0x20,
                                emu_cfg.base_ipa as u64,
                            );
                        }
                        EmuDeviceTShyper => {
                            #[cfg(feature = "tx2")]
                            fdt_add_vm_service(
                                dtb,
                                emu_cfg.irq_id as u32 - 0x20,
                                emu_cfg.base_ipa as u64,
                                emu_cfg.length as u64,
                            );
                        }
                        EmuDeviceTIOMMU => {
                            #[cfg(feature = "tx2")]
                            trace!("EmuDeviceTIOMMU");
                        }
                        _ => {
                            todo!();
                        }
                    }
                }
            }
            println!("after dtb size {}", fdt_size(dtb));
        }
        None => {
            println!("None dtb");
        }
    }
}

/* Setup VM Configuration before boot.
 * Only VM0 will call this function.
 * This func should run 1 time for each vm.
 *
 * @param[in] vm_id: target VM id to set up config.
 */
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

    if !vmm_init_emulated_device(vm.clone()) {
        panic!("vmm_setup_config: vmm_init_emulated_device failed");
    }
    if !vmm_init_passthrough_device(vm.clone()) {
        panic!("vmm_setup_config: vmm_init_passthrough_device failed");
    }
    if !vmm_init_iommu_device(vm.clone()) {
        panic!("vmm_setup_config: vmm_init_iommu_device failed");
    }

    add_async_used_info(vm_id);
    info!("VM {} id {} init ok", vm.id(), vm.config().name.unwrap());
}

pub fn vmm_cpu_assign_vcpu(vm_id: usize) {
    let cpu_id = current_cpu().id;
    if current_cpu().assigned() {
        debug!("vmm_cpu_assign_vcpu vm[{}] cpu {} is assigned", vm_id, cpu_id);
    }

    // let cpu_config = vm(vm_id).config().cpu;
    let vm = vm(vm_id).unwrap();
    let cfg_master = vm.config().cpu_master();
    let cfg_cpu_num = vm.config().cpu_num();
    let cfg_cpu_allocate_bitmap = vm.config().cpu_allocated_bitmap();

    if cfg_cpu_num != cfg_cpu_allocate_bitmap.count_ones() as usize {
        panic!(
            "vmm_cpu_assign_vcpu: VM[{}] cpu_num {} not match cpu_allocated_bitmap {:#b}",
            vm_id, cfg_cpu_num, cfg_cpu_allocate_bitmap
        );
    }

    info!(
        "vmm_cpu_assign_vcpu: vm[{}] cpu {} cfg_master {} cfg_cpu_num {} cfg_cpu_allocate_bitmap {:#b}",
        vm_id, cpu_id, cfg_master, cfg_cpu_num, cfg_cpu_allocate_bitmap
    );

    // Judge if current cpu is allocated.
    if (cfg_cpu_allocate_bitmap & (1 << cpu_id)) != 0 {
        let vcpu = match vm.select_vcpu2assign(cpu_id) {
            None => panic!("core {} vm {} cannot find proper vcpu to assign", cpu_id, vm_id),
            Some(vcpu) => vcpu,
        };
        if vcpu.id() == 0 {
            println!("* Core {} is assigned => vm {}, vcpu {}", cpu_id, vm_id, vcpu.id());
        } else {
            println!("Core {} is assigned => vm {}, vcpu {}", cpu_id, vm_id, vcpu.id());
        }
        current_cpu().vcpu_array.append_vcpu(vcpu);
    }

    if cfg_cpu_num == vm.cpu_num() {
        vm.set_ready(true);
    }
}

pub fn mvm_init() {
    if current_cpu().id == 0 {
        // Set up basic config.
        crate::config::mvm_config_init();
        // Add VM 0
        super::vmm_init_gvm(0);
    }
}

pub fn vmm_boot() {
    if current_cpu().assigned() && active_vcpu_id() == 0 {
        // active_vm().unwrap().set_migration_state(false);
        info!("Core {} start running", current_cpu().id);
        vcpu_run(false);
    } else {
        // If there is no available vm(vcpu), just go idle
        info!("Core {} idle", current_cpu().id);
        cpu_idle();
    }
}

pub fn vmm_migrate_boot() {
    let vcpu = current_cpu().active_vcpu.clone().unwrap();
    vcpu.reset_vmpidr();
    vcpu.reset_vtimer_offset();

    // println!("Core[{}] start running", current_cpu().id);
    vcpu_run(true);
}
