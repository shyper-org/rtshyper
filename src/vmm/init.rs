use alloc::vec::Vec;

use crate::arch::{
    emu_intc_handler, emu_intc_init, emu_smmu_handler, partial_passthrough_intc_handler, partial_passthrough_intc_init,
};
use crate::arch::{PTE_S2_DEVICE, PTE_S2_NORMAL};
use crate::arch::PAGE_SIZE;
use crate::board::{PlatOperation, Platform, PLATFORM_CPU_NUM_MAX};
use crate::config::VmRegion;
use crate::device::{emu_register_dev, emu_virtio_mmio_handler, emu_virtio_mmio_init};
use crate::dtb::create_fdt;
use crate::device::EmuDeviceType::*;
use crate::kernel::{
    cpu_idle, current_cpu, iommmu_vm_init, shyper_init, vm_if_init_mem_map, VmType, iommu_add_device,
    mem_region_alloc_colors, ColorMemRegion, count_missing_num, IpiVmmMsg, ipi_send_msg, IpiType, IpiInnerMsg,
};
use crate::kernel::mem_page_alloc;
use crate::kernel::{vm, Vm};
use crate::kernel::{active_vcpu_id, vcpu_run};
use crate::kernel::interrupt_vm_register;
use crate::kernel::access::copy_segment_to_vm;
use crate::util::sleep;
use crate::vmm::VmmEvent;
use crate::vmm::address::vmm_setup_ipa2hva;

#[cfg(feature = "ramdisk")]
pub static CPIO_RAMDISK: &'static [u8] = include_bytes!("../../image/net_rootfs.cpio");
#[cfg(not(feature = "ramdisk"))]
pub static CPIO_RAMDISK: &[u8] = &[];

fn vm_map_ipa2color_regions(vm: &Vm, vm_region: &VmRegion, color_regions: &[ColorMemRegion]) {
    // NOTE: continuous ipa should across colors, and the color_regions must be sorted by count
    let missing_list = count_missing_num(color_regions);
    for (i, region) in color_regions.iter().enumerate() {
        for j in 0..region.count {
            let missing_num = missing_list.get(j).unwrap();
            let page_idx = i + j * color_regions.len() - missing_num;
            let ipa = vm_region.ipa_start + page_idx * PAGE_SIZE;
            let pa = region.base + j * region.step;
            vm.pt_map_range(ipa, PAGE_SIZE, pa, PTE_S2_NORMAL, false);
        }
    }
}

fn vmm_init_memory(vm: &Vm) -> bool {
    let vm_id = vm.id();
    let config = vm.config();
    let vm_mem_size = config.memory_region().iter().map(|x| x.length).sum::<usize>();
    if let Ok(pt_dir_frame) = mem_page_alloc() {
        vm.set_pt(pt_dir_frame);
    } else {
        println!("vmm_init_memory: page alloc failed");
        return false;
    }

    for vm_region in config.memory_region().iter() {
        match mem_region_alloc_colors(vm_region.length, config.memory_color_bitmap()) {
            Ok(vm_color_regions) => {
                assert!(!vm_color_regions.is_empty());
                info!("{:x?}", vm_color_regions);
                vm_map_ipa2color_regions(vm, vm_region, &vm_color_regions);
                vm.append_color_regions(vm_color_regions);
            }
            Err(_) => {
                println!(
                    "vmm_init_memory: mem_vm_region_alloc_by_colors failed, length {}, color bitmap {:#x}",
                    vm_region.length,
                    config.memory_color_bitmap()
                );
                return false;
            }
        }
    }
    vmm_setup_ipa2hva(vm);
    vm_if_init_mem_map(vm_id, (vm_mem_size + PAGE_SIZE - 1) / PAGE_SIZE);

    true
}

fn vmm_load_image(vm: &Vm, bin: &[u8]) {
    copy_segment_to_vm(vm, vm.config().kernel_load_ipa(), bin);
}

pub(super) fn vmm_init_image(vm: &Vm) -> bool {
    let vm_id = vm.id();
    let config = vm.config();

    if config.kernel_load_ipa() == 0 {
        println!("vmm_init_image: kernel load ipa is null");
        return false;
    }

    // Only load MVM kernel image "L4T" from binding.
    // Load GVM kernel image from shyper-cli, you may check it for more information.
    if config.os_type == VmType::VmTOs {
        match vm.config().kernel_img_name() {
            Some(name) => {
                #[cfg(feature = "tx2")]
                {
                    if name == "L4T" {
                        println!("MVM {} loading Image", vm.id());
                        vmm_load_image(vm, include_bytes!("../../image/L4T"));
                    } else {
                        cfg_if::cfg_if! {
                            if #[cfg(feature = "static-config")] {
                                if name == "Image_vanilla" {
                                    println!("VM {} loading default Linux Image", vm.id());
                                    vmm_load_image(vm, include_bytes!("../../image/Image_vanilla"));
                                } else {
                                    warn!("Image {} is not supported", name);
                                }
                            } else if #[cfg(feature = "unishyper")] {
                                if name == "Image_Unishyper" {
                                    vmm_load_image(vm, include_bytes!("../../image/Image_Unishyper"));
                                } else {
                                    warn!("Image {} is not supported", name);
                                }
                            } else {
                                warn!("Image {} is not supported", name);
                            }
                        }
                    }
                }
                #[cfg(feature = "pi4")]
                if name.is_empty() {
                    panic!("kernel image name empty")
                } else {
                    vmm_load_image(vm, include_bytes!("../../image/Image_pi4_5.4.83_tlb"));
                }
                // vmm_load_image(vm, include_bytes!("../../image/Image_pi4_5.4.78"));
                // vmm_load_image(vm, include_bytes!("../../image/Image_pi4"));
                #[cfg(feature = "qemu")]
                if name.is_empty() {
                    panic!("kernel image name empty")
                } else {
                    vmm_load_image(vm, include_bytes!("../../image/Image_vanilla"));
                }
            }
            None => {
                info!("VM[{}] is a dynamic configuration", vm_id);
            }
        }
    }

    if config.device_tree_load_ipa() != 0 {
        // Init dtb for Linux.
        if vm_id == 0 {
            // Init dtb for MVM.
            let mut dtb = crate::dtb::SYSTEM_FDT.get().unwrap().clone();
            // enlarge the size of dtb, because vmm_setup_fdt_vm0 will enlarge it unsafely!
            dtb.resize(dtb.len() << 1, 0);
            let size = unsafe { vmm_setup_fdt_vm0(vm, dtb.as_ptr() as *mut _) };
            if size >= dtb.len() {
                panic!("unsafe dtb editing!!");
            }
            dtb.resize(size, 0);
            copy_segment_to_vm(vm, config.device_tree_load_ipa(), dtb.as_slice());
        } else {
            // Init dtb for GVM.
            match create_fdt(config) {
                Ok(dtb) => {
                    copy_segment_to_vm(vm, config.device_tree_load_ipa(), dtb.as_slice());
                }
                _ => {
                    panic!("vmm_setup_config: create fdt for vm{} fail", vm.id());
                }
            }
        }
    } else {
        println!("VM {} id {} device tree load ipa is not set", vm_id, vm.config().name);
    }

    // ...
    // Todo: support loading ramdisk from MVM shyper-cli.
    // ...
    #[cfg(feature = "ramdisk")]
    if config.ramdisk_load_ipa() != 0 {
        println!("VM {} use ramdisk CPIO_RAMDISK", vm_id);
        copy_segment_to_vm(vm, config.ramdisk_load_ipa(), CPIO_RAMDISK);
    }

    true
}

fn vmm_init_emulated_device(vm: &Vm) -> bool {
    let config = vm.config().emulated_device_list();

    for (idx, emu_cfg) in config.iter().enumerate() {
        match emu_cfg.emu_type {
            EmuDeviceTGicd => {
                vm.set_intc_dev_id(idx);
                emu_register_dev(vm.id(), idx, emu_cfg.base_ipa, emu_cfg.length, emu_intc_handler);
                let emu_dev = emu_intc_init(vm).unwrap();
                vm.set_emu_devs(idx, emu_dev);
            }
            EmuDeviceTGPPT => {
                vm.set_intc_dev_id(idx);
                emu_register_dev(
                    vm.id(),
                    idx,
                    emu_cfg.base_ipa,
                    emu_cfg.length,
                    partial_passthrough_intc_handler,
                );
                partial_passthrough_intc_init(vm);
            }
            EmuDeviceTVirtioBlk | EmuDeviceTVirtioConsole | EmuDeviceTVirtioNet => {
                emu_register_dev(vm.id(), idx, emu_cfg.base_ipa, emu_cfg.length, emu_virtio_mmio_handler);
                if let Ok(emu_dev) = emu_virtio_mmio_init(emu_cfg) {
                    vm.set_emu_devs(idx, emu_dev);
                } else {
                    return false;
                }
                if emu_cfg.emu_type == EmuDeviceTVirtioNet {
                    let mac = emu_cfg.cfg_list.iter().take(6).map(|x| *x as u8).collect::<Vec<_>>();
                    crate::kernel::set_mac_vmid(&mac, vm.id());
                }
            }
            EmuDeviceTIOMMU => {
                emu_register_dev(vm.id(), idx, emu_cfg.base_ipa, emu_cfg.length, emu_smmu_handler);
                if !iommmu_vm_init(vm) {
                    return false;
                }
            }
            EmuDeviceTShyper => {
                if !shyper_init(vm, emu_cfg.base_ipa, emu_cfg.length) {
                    return false;
                }
            }
            _ => {
                warn!("vmm_init_emulated_device: unknown emulated device");
                return false;
            }
        }
        if !interrupt_vm_register(vm, emu_cfg.irq_id, false) {
            return false;
        }
        info!(
            "VM {} registers emulated device: id=<{}>, name=\"{:?}\", ipa=<{:#x}>",
            vm.id(),
            idx,
            emu_cfg.emu_type,
            emu_cfg.base_ipa
        );
    }

    true
}

fn vmm_init_passthrough_device(vm: &Vm) -> bool {
    for region in vm.config().passthrough_device_regions() {
        if region.dev_property {
            vm.pt_map_range(region.ipa, region.length, region.pa, PTE_S2_DEVICE, true);
        } else {
            vm.pt_map_range(region.ipa, region.length, region.pa, PTE_S2_NORMAL, true);
        }

        debug!(
            "VM {} registers passthrough device: ipa=<{:#x}>, pa=<{:#x}>, size=<{:#x}>, {}",
            vm.id(),
            region.ipa,
            region.pa,
            region.length,
            if region.dev_property { "device" } else { "normal" }
        );
    }
    for irq in vm.config().passthrough_device_irqs() {
        if !interrupt_vm_register(vm, irq, true) {
            return false;
        }
    }
    true
}

fn vmm_init_iommu_device(vm: &Vm) -> bool {
    for stream_id in vm.config().passthrough_device_stread_ids() {
        if stream_id == 0 {
            break;
        }
        if !iommu_add_device(vm, stream_id) {
            return false;
        }
    }
    true
}

unsafe fn vmm_setup_fdt_vm0(vm: &Vm, dtb: *mut core::ffi::c_void) -> usize {
    use fdt::*;
    let config = vm.config();
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
    #[cfg(feature = "qemu")]
    fdt_set_memory(dtb, mr.len() as u64, mr.as_ptr(), "memory@50000000\0".as_ptr());
    // FDT+TIMER
    fdt_add_timer(dtb, 0x8);
    // FDT+BOOTCMD
    fdt_set_bootcmd(dtb, config.cmdline.as_ptr());
    #[cfg(feature = "tx2")]
    fdt_set_stdout_path(dtb, "/serial@3100000\0".as_ptr());
    // #[cfg(feature = "pi4")]
    // fdt_set_stdout_path(dtb, "/serial@fe340000\0".as_ptr());

    for emu_cfg in config.emulated_device_list() {
        match emu_cfg.emu_type {
            EmuDeviceTGicd => {
                #[cfg(any(feature = "tx2", feature = "qemu"))]
                fdt_setup_gic(
                    dtb,
                    Platform::GICD_BASE as u64,
                    Platform::GICC_BASE as u64,
                    emu_cfg.name.as_ptr(),
                );
                #[cfg(feature = "pi4")]
                fdt_setup_gic(
                    dtb,
                    (Platform::GICD_BASE | 0xF_0000_0000) as u64,
                    (Platform::GICC_BASE | 0xF_0000_0000) as u64,
                    emu_cfg.name.as_ptr(),
                );
            }
            EmuDeviceTVirtioNet | EmuDeviceTVirtioConsole => {
                #[cfg(any(feature = "tx2", feature = "qemu"))]
                fdt_add_virtio(
                    dtb,
                    emu_cfg.name.as_ptr(),
                    emu_cfg.irq_id as u32 - 0x20,
                    emu_cfg.base_ipa as u64,
                );
            }
            EmuDeviceTShyper => {
                #[cfg(any(feature = "tx2", feature = "qemu"))]
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
    let size = fdt_size(dtb) as usize;
    println!("after dtb size {:#x}", size);
    size
}

/* Setup VM Configuration before boot.
 * Only VM0 will call this function.
 * This func should run 1 time for each vm.
 *
 * @param[in] vm_id: target VM id to set up config.
 */
pub fn vmm_setup_config(vm: Vm) {
    println!(
        "vmm_setup_config VM[{}] name {:?} current core {}",
        vm.id(),
        vm.config().name,
        current_cpu().id
    );

    vmm_init_cpu(&vm);
    if !vmm_init_memory(&vm) {
        panic!("vmm_setup_config: vmm_init_memory failed");
    }

    if !vmm_init_image(&vm) {
        panic!("vmm_setup_config: vmm_init_image failed");
    }

    if !vmm_init_emulated_device(&vm) {
        panic!("vmm_setup_config: vmm_init_emulated_device failed");
    }
    if !vmm_init_passthrough_device(&vm) {
        panic!("vmm_setup_config: vmm_init_passthrough_device failed");
    }
    if !vmm_init_iommu_device(&vm) {
        panic!("vmm_setup_config: vmm_init_iommu_device failed");
    }

    info!("VM {} id {} init ok", vm.id(), vm.config().name);
}

fn vmm_init_cpu(vm: &Vm) {
    let vm_id = vm.id();
    println!("vmm_init_cpu: set up vm {} on cpu {}", vm_id, current_cpu().id);
    println!(
        "VM {} init cpu: cores=<{}>, allocat_bits=<{:#b}>",
        vm.id(),
        vm.config().cpu_num(),
        vm.config().cpu_allocated_bitmap()
    );

    let mut cpu_allocate_bitmap = vm.config().cpu_allocated_bitmap();
    let mut target_cpu_id = 0;
    let mut cpu_num = 0;
    while cpu_allocate_bitmap != 0 && target_cpu_id < PLATFORM_CPU_NUM_MAX {
        if cpu_allocate_bitmap & 1 != 0 {
            println!("vmm_init_cpu: vm {} physical cpu id {}", vm_id, target_cpu_id);
            cpu_num += 1;

            if target_cpu_id != current_cpu().id {
                let m = IpiVmmMsg {
                    vmid: vm_id,
                    event: VmmEvent::VmmAssignCpu,
                };
                if !ipi_send_msg(target_cpu_id, IpiType::IpiTVMM, IpiInnerMsg::VmmMsg(m)) {
                    println!("vmm_init_cpu: failed to send ipi to Core {}", target_cpu_id);
                }
            } else {
                vmm_cpu_assign_vcpu(vm_id);
            }
        }
        cpu_allocate_bitmap >>= 1;
        target_cpu_id += 1;
    }
    println!(
        "vmm_init_cpu: vm {} total physical cpu num {} bitmap {:#b}",
        vm_id,
        cpu_num,
        vm.config().cpu_allocated_bitmap()
    );

    // Waiting till others set up.
    println!(
        "vmm_init_cpu: on core {}, waiting VM [{}] to be set up",
        current_cpu().id,
        vm_id
    );
    while !vm.ready() {
        sleep(10);
    }
    println!("vmm_init_cpu: VM [{}] is ready", vm_id);
}

pub fn vmm_cpu_assign_vcpu(vm_id: usize) {
    let cpu_id = current_cpu().id;
    if current_cpu().assigned() {
        debug!("vmm_cpu_assign_vcpu vm[{}] cpu {} is assigned", vm_id, cpu_id);
    }

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
        vm_id, cpu_id, cfg_master as isize, cfg_cpu_num, cfg_cpu_allocate_bitmap
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

pub fn vm_init() {
    if current_cpu().id == 0 {
        // Set up basic config.
        if cfg!(feature = "unishyper") {
            crate::config::unishyper_config_init();
        } else {
            crate::config::mvm_config_init();
        }
        // Add VM 0
        super::vmm_init_gvm(0);
        #[cfg(feature = "static-config")]
        {
            crate::config::init_tmp_config_for_vm1();
            crate::config::init_tmp_config_for_vm2();
            super::vmm_init_gvm(1);
            super::vmm_init_gvm(2);
        }
    }
}

pub fn vmm_boot() {
    if current_cpu().assigned() && active_vcpu_id() == 0 {
        // active_vm().unwrap().set_migration_state(false);
        info!("Core {} start running", current_cpu().id);
        vcpu_run(false);
    } else if !current_cpu().assigned() {
        // If there is no available vm(vcpu), just go idle
        info!("Core {} idle", current_cpu().id);
        cpu_idle();
    }
}
