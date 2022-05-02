use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;

use spin::Mutex;

use crate::config::{
    DEF_VM_CONFIG_TABLE, VmConfigEntry, VmConfigTable, VmDtbDevConfig, VMDtbDevConfigList, VmEmulatedDeviceConfig,
    VmEmulatedDeviceConfigList, VmMemoryConfig, VmPassthroughDeviceConfig,
};
use crate::kernel::{
    CPU, Cpu, CPU_IF_LIST, CPU_LIST, CpuIf, current_cpu, mem_heap_region_init, SchedType, SchedulerRR, Vcpu, VCPU_LIST,
    VcpuInner, VcpuPool, VcpuState,
};
use crate::mm::heap_init;

pub struct HypervisorAddr {
    cpu: usize,
    cpu_if: usize,
    vcpu_list: usize,
    vm_config_table: usize,
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

        let addr_list = HypervisorAddr {
            cpu,
            cpu_if,
            vcpu_list,
            vm_config_table,
        };
        update_request(&addr_list);
    }
}

#[no_mangle]
pub unsafe extern "C" fn rust_shyper_update(address_list: &HypervisorAddr) {
    // TODO: PLAT_DESC
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
        // VmConfig
        let vm_config_table = &*(address_list.vm_config_table as *const Mutex<VmConfigTable>);
        vm_config_table_update(vm_config_table);
        let cpu = &*(address_list.cpu as *const Cpu);
        let cpu_if = &*(address_list.cpu_if as *const Mutex<Vec<CpuIf>>);
        // vcpu_list
        let vcpu_list = &*(address_list.vcpu_list as *const Mutex<Vec<Vcpu>>);
        vcpu_update(vcpu_list);
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
