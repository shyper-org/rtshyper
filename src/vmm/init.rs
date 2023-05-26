use alloc::vec::Vec;

use crate::arch::{PTE_S2_DEVICE, PTE_S2_NORMAL};
use crate::arch::PAGE_SIZE;
use crate::board::{PlatOperation, Platform};
use crate::config::VmRegion;
use crate::dtb::create_fdt;
use crate::device::EmuDeviceType::*;
use crate::kernel::{
    cpu_idle, current_cpu, iommmu_vm_init, vm_if_init_mem_map, VmType, iommu_add_device, mem_region_alloc_colors,
    ColorMemRegion, count_missing_num, IpiVmmMsg, ipi_send_msg, IpiType, IpiInnerMsg,
};
use crate::kernel::{vm, Vm};
use crate::kernel::{active_vcpu_id, vcpu_run};
use crate::kernel::interrupt_vm_register;
use crate::kernel::access::copy_segment_to_vm;
use crate::vmm::VmmEvent;
use crate::vmm::address::vmm_setup_ipa2hva;

cfg_if::cfg_if! {
    if #[cfg(feature = "ramdisk")] {
        pub static CPIO_RAMDISK: & [u8] = include_bytes!("../../image/net_rootfs.cpio");
    } else {
        pub static CPIO_RAMDISK: &[u8] = &[];
    }
}

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
    // passthrough regions
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
    // normal memory regions
    let vm_memory_regions = config.memory_region();
    let vm_mem_size = vm_memory_regions.iter().map(|x| x.length).sum::<usize>();
    for vm_region in vm_memory_regions.iter() {
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

fn vmm_init_hardware(vm: &Vm) -> bool {
    // init passthrough irqs
    for irq in vm.config().passthrough_device_irqs() {
        if !interrupt_vm_register(vm, *irq, true) {
            return false;
        }
    }
    // init iommu
    for emu_cfg in vm.config().emulated_device_list().iter() {
        if emu_cfg.emu_type == EmuDeviceTIOMMU {
            if !iommmu_vm_init(vm) {
                return false;
            } else {
                break;
            }
        }
    }
    for stream_id in vm.config().passthrough_device_stread_ids() {
        if *stream_id == 0 {
            break;
        }
        if !iommu_add_device(vm, *stream_id) {
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
    // need ipi, must after push to global list
    vmm_init_cpu(&vm);
    // need ipi, must after push to global list
    if !vmm_init_memory(&vm) {
        panic!("vmm_setup_config: vmm_init_memory failed");
    }
    // need memory, must after init memory
    if !vmm_init_image(&vm) {
        panic!("vmm_setup_config: vmm_init_image failed");
    }
    if !vmm_init_hardware(&vm) {
        panic!("vmm_setup_config: vmm_init_hardware failed");
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

    for vcpu in vm.vcpu_list() {
        let target_cpu_id = vcpu.phys_id();
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
    println!("vmm_init_cpu: VM [{}] is ready", vm_id);
}

pub fn vmm_cpu_assign_vcpu(vm_id: usize) {
    let cpu_id = current_cpu().id;
    if current_cpu().assigned() {
        debug!("vmm_cpu_assign_vcpu vm[{}] cpu {} is assigned", vm_id, cpu_id);
    }

    let vm = vm(vm_id).unwrap();

    for vcpu in vm.vcpu_list() {
        if vcpu.phys_id() == current_cpu().id {
            if vcpu.id() == 0 {
                println!("* Core {} is assigned => vm {}, vcpu {}", cpu_id, vm_id, vcpu.id());
            } else {
                println!("Core {} is assigned => vm {}, vcpu {}", cpu_id, vm_id, vcpu.id());
            }
            current_cpu().vcpu_array.append_vcpu(vcpu.clone());
            break;
        }
    }
}

pub fn vm_init() {
    if current_cpu().id == 0 {
        // Set up basic config.
        if cfg!(feature = "unishyper") {
            #[cfg(feature = "tx2")]
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
