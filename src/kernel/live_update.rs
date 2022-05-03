use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use spin::Mutex;

use crate::arch::{
    emu_intc_handler, emu_intc_init, gic_maintenance_handler, GIC_PRIVINT_NUM, PageTable,
    partial_passthrough_intc_handler, partial_passthrough_intc_init,
};
use crate::config::{
    DEF_VM_CONFIG_TABLE, VmConfigEntry, VmConfigTable, VmDtbDevConfig, VMDtbDevConfigList, VmEmulatedDeviceConfig,
    VmEmulatedDeviceConfigList, VmMemoryConfig, VmPassthroughDeviceConfig,
};
use crate::device::{EMU_DEVS_LIST, EmuDevEntry, EmuDeviceType};
use crate::device::emu_virtio_mmio_handler;
use crate::kernel::{
    CPU, Cpu, CPU_IF_LIST, CPU_LIST, CpuIf, current_cpu, HEAP_REGION, HeapRegion, INTERRUPT_GLB_BITMAP,
    INTERRUPT_HANDLERS, INTERRUPT_HYPER_BITMAP, INTERRUPT_NUM_MAX, InterruptHandler, ipi_irq_handler,
    mem_heap_region_init, MemRegion, SchedType, SchedulerRR, timer_irq_handler, Vcpu, VCPU_LIST, VcpuInner, VcpuPool,
    VcpuState, Vm, VM_LIST, VM_REGION, VmInner, VmRegion,
};
use crate::lib::{BitAlloc256, BitMap};
use crate::mm::{heap_init, PageFrame};

pub struct HypervisorAddr {
    cpu: usize,
    cpu_if: usize,
    vcpu_list: usize,
    vm_config_table: usize,
    emu_dev_list: usize,
    interrupt_hyper_bitmap: usize,
    interrupt_glb_bitmap: usize,
    interrupt_handlers: usize,
    vm_region: usize,
    heap_region: usize,
    vm_list: usize,
}

pub fn update_request() {
    println!("src hypervisor send update request");
    extern "C" {
        pub fn update_request(address_list: &HypervisorAddr);
    }
    // unsafe {
    //     list = *(addr as *const _);
    // }
    // println!("cpuif len {}", list.lock().len());
    unsafe {
        let cpu = &CPU as *const _ as usize;
        let cpu_if = &CPU_IF_LIST as *const _ as usize;
        let vcpu_list = &VCPU_LIST as *const _ as usize;
        let vm_config_table = &DEF_VM_CONFIG_TABLE as *const _ as usize;
        let emu_dev_list = &EMU_DEVS_LIST as *const _ as usize;
        let interrupt_hyper_bitmap = &INTERRUPT_HYPER_BITMAP as *const _ as usize;
        let interrupt_glb_bitmap = &INTERRUPT_GLB_BITMAP as *const _ as usize;
        let interrupt_handlers = &INTERRUPT_HANDLERS as *const _ as usize;
        let vm_region = &VM_REGION as *const _ as usize;
        let heap_region = &HEAP_REGION as *const _ as usize;
        let vm_list = &VM_LIST as *const _ as usize;
        let addr_list = HypervisorAddr {
            cpu,
            cpu_if,
            vcpu_list,
            vm_config_table,
            emu_dev_list,
            interrupt_hyper_bitmap,
            interrupt_glb_bitmap,
            interrupt_handlers,
            vm_region,
            heap_region,
            vm_list,
        };
        update_request(&addr_list);
    }
}

#[no_mangle]
pub unsafe extern "C" fn rust_shyper_update(address_list: &HypervisorAddr) {
    // TODO: SHARED_MEM
    // TODO: vm0_dtb?
    // TODO: ipi_register
    // TODO: gic
    // TODO: vgic
    // TODO: cpu
    // TODO: vm
    // TODO: mediated dev
    heap_init();
    mem_heap_region_init();

    println!("in rust_shyper_update");
    println!("cpu if addr {:x}", address_list.cpu_if);
    println!("cpu addr {:x}", address_list.cpu);
    println!("vm_config_table addr {:x}", address_list.vm_config_table);
    unsafe {
        // DEF_VM_CONFIG_TABLE
        let vm_config_table = &*(address_list.vm_config_table as *const Mutex<VmConfigTable>);
        vm_config_table_update(vm_config_table);

        // EMU_DEVS_LIST
        let emu_dev_list = &*(address_list.emu_dev_list as *const Mutex<Vec<EmuDevEntry>>);
        emu_dev_list_update(emu_dev_list);

        // INTERRUPT_HYPER_BITMAP, INTERRUPT_GLB_BITMAP, INTERRUPT_HANDLERS
        let interrupt_hyper_bitmap = &*(address_list.interrupt_hyper_bitmap as *const Mutex<BitMap<BitAlloc256>>);
        let interrupt_glb_bitmap = &*(address_list.interrupt_glb_bitmap as *const Mutex<BitMap<BitAlloc256>>);
        let interrupt_handlers =
            &*(address_list.interrupt_hyper_bitmap as *const Mutex<[InterruptHandler; INTERRUPT_NUM_MAX]>);
        interrupt_update(interrupt_hyper_bitmap, interrupt_glb_bitmap, interrupt_handlers);

        let cpu = &*(address_list.cpu as *const Cpu);
        let cpu_if = &*(address_list.cpu_if as *const Mutex<Vec<CpuIf>>);

        // VCPU_LIST
        let vcpu_list = &*(address_list.vcpu_list as *const Mutex<Vec<Vcpu>>);
        vcpu_update(vcpu_list);

        // VM_REGION
        let vm_region = &*(address_list.vm_region as *const Mutex<VmRegion>);
        vm_region_update(vm_region);

        // HEAP_REGION
        let heap_region = &*(address_list.heap_region as *const Mutex<HeapRegion>);
        heap_region_update(heap_region);

        // VM_LIST
        let vm_list = &*(address_list.vm_list as *const Mutex<Vec<Vm>>);
    }
}

pub fn vm_list_update(src_vm_list: &Mutex<Vec<Vm>>) {
    let mut vm_list = VM_LIST.lock();
    vm_list.clear();
    for vm in src_vm_list.lock().iter() {
        let old_inner = vm.inner.lock();
        let pt = match &old_inner.pt {
            None => None,
            Some(page_table) => {
                let mut new_page_table = PageTable {
                    directory: PageFrame::new(page_table.directory.pa),
                    pages: Mutex::new(vec![]),
                };
                for page in page_table.pages.lock().iter() {
                    new_page_table.pages.lock().push(PageFrame::new(page.pa));
                }
            }
        };

        let new_inner = VmInner {
            id: old_inner.id,
            ready: old_inner.ready,
            config: None,
            dtb: old_inner.dtb, // maybe need to reset
            pt,
            mem_region_num: 0,
            pa_region: vec![],
            entry_point: 0,
            has_master: false,
            vcpu_list: vec![],
            cpu_num: 0,
            ncpu: 0,
            intc_dev_id: 0,
            int_bitmap: None,
            share_mem_base: 0,
            migrate_save_pf: vec![],
            migrate_restore_pf: vec![],
            emu_devs: vec![],
            med_blk_id: None,
        };
    }
}

pub fn heap_region_update(src_heap_region: &Mutex<HeapRegion>) {
    let mut heap_region = HEAP_REGION.lock();
    let mut src_region = src_heap_region.lock();
    heap_region.map = src_region.map;
    heap_region.region = src_region.region;
    assert_eq!(heap_region.region, src_region.region);
}

pub fn vm_region_update(src_vm_region: &Mutex<VmRegion>) {
    let mut vm_region = VM_REGION.lock();
    vm_region.region.clear();
    for mem_region in src_vm_region.lock().region.iter() {
        vm_region.region.push(*mem_region);
    }
    assert_eq!(vm_region.region, src_vm_region.lock().region);
}

pub fn interrupt_update(
    src_hyper_bitmap: &Mutex<BitMap<BitAlloc256>>,
    src_glb_bitmap: &Mutex<BitMap<BitAlloc256>>,
    src_handlers: &Mutex<[InterruptHandler; INTERRUPT_NUM_MAX]>,
) {
    let mut hyper_bitmap = INTERRUPT_HYPER_BITMAP.lock();
    hyper_bitmap = src_hyper_bitmap.lock();
    let mut glb_bitmap = INTERRUPT_GLB_BITMAP.lock();
    glb_bitmap = src_glb_bitmap.lock();
    let mut handlers = INTERRUPT_HANDLERS.lock();
    for (idx, handler) in src_handlers.lock().iter().enumerate() {
        if idx >= GIC_PRIVINT_NUM {
            break;
        }
        match handler {
            InterruptHandler::IpiIrqHandler(_) => {
                handlers[idx] = InterruptHandler::IpiIrqHandler(ipi_irq_handler);
            }
            InterruptHandler::GicMaintenanceHandler(_) => {
                handlers[idx] = InterruptHandler::GicMaintenanceHandler(gic_maintenance_handler);
            }
            InterruptHandler::TimeIrqHandler(_) => {
                handlers[idx] = InterruptHandler::TimeIrqHandler(timer_irq_handler);
            }
            InterruptHandler::None => {
                handlers[idx] = InterruptHandler::None;
            }
        }
    }
}

pub fn emu_dev_list_update(src_emu_dev_list: &Mutex<Vec<EmuDevEntry>>) {
    let mut emu_dev_list = EMU_DEVS_LIST.lock();
    emu_dev_list.clear();
    for emu_dev_entry in src_emu_dev_list.lock().iter() {
        let emu_handler = match emu_dev_entry.emu_type {
            EmuDeviceType::EmuDeviceTGicd => emu_intc_handler,
            EmuDeviceType::EmuDeviceTGPPT => partial_passthrough_intc_handler,
            EmuDeviceType::EmuDeviceTVirtioBlk => emu_virtio_mmio_handler,
            EmuDeviceType::EmuDeviceTVirtioNet => emu_virtio_mmio_handler,
            EmuDeviceType::EmuDeviceTVirtioConsole => emu_virtio_mmio_handler,
            _ => {
                panic!("not support emu dev entry type {:#?}", emu_dev_entry.emu_type);
            }
        };
        emu_dev_list.push(EmuDevEntry {
            emu_type: emu_dev_entry.emu_type,
            vm_id: emu_dev_entry.vm_id,
            id: emu_dev_entry.id,
            ipa: emu_dev_entry.ipa,
            size: emu_dev_entry.size,
            handler: emu_handler,
        });
    }
}

pub fn vm_config_table_update(src_vm_config_table: &Mutex<VmConfigTable>) {
    let mut vm_config_table = DEF_VM_CONFIG_TABLE.lock();
    let src_config_table = src_vm_config_table.lock();
    vm_config_table.name = src_config_table.name;
    vm_config_table.vm_bitmap = src_config_table.vm_bitmap;
    vm_config_table.vm_num = src_config_table.vm_num;
    vm_config_table.entries.clear();
    for entry in src_config_table.entries.iter() {
        let image = *entry.image.lock();
        let memory = VmMemoryConfig {
            region: {
                let mut region = vec![];
                for mem in entry.memory.lock().region.iter() {
                    region.push(*mem);
                }
                assert_eq!(region, entry.memory.lock().region);
                region
            },
        };
        let cpu = *entry.cpu.lock();
        // emu dev config
        let mut vm_emu_dev_confg = VmEmulatedDeviceConfigList { emu_dev_list: vec![] };
        let src_emu_dev_confg_list = entry.vm_emu_dev_confg.lock();
        for emu_config in &src_emu_dev_confg_list.emu_dev_list {
            vm_emu_dev_confg.emu_dev_list.push(VmEmulatedDeviceConfig {
                name: Some(String::from(emu_config.name.as_ref().unwrap())),
                base_ipa: emu_config.base_ipa,
                length: emu_config.length,
                irq_id: emu_config.irq_id,
                cfg_list: {
                    let mut cfg_list = vec![];
                    for cfg in emu_config.cfg_list.iter() {
                        cfg_list.push(*cfg);
                    }
                    assert_eq!(cfg_list, emu_config.cfg_list);
                    cfg_list
                },
                emu_type: emu_config.emu_type,
                mediated: emu_config.mediated,
            })
        }
        // passthrough dev config
        let src_pt = entry.vm_pt_dev_confg.lock();
        let mut vm_pt_dev_confg = VmPassthroughDeviceConfig {
            regions: vec![],
            irqs: vec![],
            streams_ids: vec![],
        };
        for region in src_pt.regions.iter() {
            vm_pt_dev_confg.regions.push(*region);
        }
        for irq in src_pt.irqs.iter() {
            vm_pt_dev_confg.irqs.push(*irq);
        }
        for streams_id in src_pt.streams_ids.iter() {
            vm_pt_dev_confg.streams_ids.push(*streams_id);
        }
        assert_eq!(vm_pt_dev_confg.regions, src_pt.regions);
        assert_eq!(vm_pt_dev_confg.irqs, src_pt.irqs);
        assert_eq!(vm_pt_dev_confg.streams_ids, src_pt.streams_ids);

        // dtb config
        let mut vm_dtb_devs = VMDtbDevConfigList {
            dtb_device_list: vec![],
        };
        let src_dtb_confg_list = entry.vm_dtb_devs.lock();
        for dtb_config in src_dtb_confg_list.dtb_device_list.iter() {
            vm_dtb_devs.dtb_device_list.push(VmDtbDevConfig {
                name: String::from(&dtb_config.name),
                dev_type: dtb_config.dev_type,
                irqs: {
                    let mut irqs = vec![];
                    for irq in dtb_config.irqs.iter() {
                        irqs.push(*irq);
                    }
                    assert_eq!(irqs, dtb_config.irqs);
                    irqs
                },
                addr_region: dtb_config.addr_region,
            });
        }

        vm_config_table.entries.push(VmConfigEntry {
            id: entry.id,
            name: Some(String::from(entry.name.as_ref().unwrap())),
            os_type: entry.os_type,
            cmdline: String::from(&entry.cmdline),
            image: Arc::new(Mutex::new(image)),
            memory: Arc::new(Mutex::new(memory)),
            cpu: Arc::new(Mutex::new(cpu)),
            vm_emu_dev_confg: Arc::new(Mutex::new(vm_emu_dev_confg)),
            vm_pt_dev_confg: Arc::new(Mutex::new(vm_pt_dev_confg)),
            vm_dtb_devs: Arc::new(Mutex::new(vm_dtb_devs)),
        });
    }
    assert_eq!(vm_config_table.entries.len(), src_config_table.entries.len());
    assert_eq!(vm_config_table.vm_num, src_config_table.vm_num);
    assert_eq!(vm_config_table.vm_bitmap, src_config_table.vm_bitmap);
    assert_eq!(vm_config_table.name, src_config_table.name);
    println!("Update {} VM to DEF_VM_CONFIG_TABLE", vm_config_table.vm_num);
}

// TODO: set vcpu.vm later
pub fn vcpu_update(src_vcpu_list: &Mutex<Vec<Vcpu>>) {
    let mut vcpu_list = VCPU_LIST.lock();
    vcpu_list.clear();
    for vcpu in src_vcpu_list.lock().iter() {
        let src_inner = vcpu.inner.lock();
        let mut vcpu_inner = VcpuInner {
            id: src_inner.id,
            phys_id: src_inner.phys_id,
            state: src_inner.state,
            vm: None,
            int_list: vec![],
            vcpu_ctx: src_inner.vcpu_ctx,
            vm_ctx: src_inner.vm_ctx,
        };
        // need to check
        for int in src_inner.int_list.iter() {
            vcpu_inner.int_list.push(*int);
        }
        assert_eq!(vcpu_inner.int_list, src_inner.int_list);

        vcpu_list.push(Vcpu {
            inner: Arc::new((Mutex::new(vcpu_inner))),
        })
    }
    assert_eq!(vcpu_list.len(), src_vcpu_list.lock().len());
    println!("Update {} Vcpu to VCPU_LIST", vcpu_list.len());
}

pub fn cpu_update(src_cpu: &Cpu) {
    // only need to alloc a new VcpuPool from heap, other props all map at 0x400000000
    // current_cpu().sched = src_cpu.sched;
    match &src_cpu.sched {
        SchedType::SchedRR(rr) => {
            let new_rr = SchedulerRR {
                pool: VcpuPool::default(),
            };
            current_cpu().sched = SchedType::SchedRR(new_rr);
        }
        SchedType::None => {
            current_cpu().sched = SchedType::None;
        }
    }
}
