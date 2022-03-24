use alloc::sync::Arc;
use alloc::vec::Vec;

use spin::Mutex;

use crate::arch::{GICC_CTLR_EN_BIT, GICC_CTLR_EOIMODENS_BIT};
use crate::arch::PageTable;
use crate::arch::Vgic;
use crate::config::DEF_VM_CONFIG_TABLE;
use crate::config::VmConfigEntry;
use crate::device::EmuDevs;
use crate::lib::*;

use super::mem::VM_MEM_REGION_MAX;
use super::vcpu::Vcpu;

pub const VM_NUM_MAX: usize = 8;
pub static VM_IF_LIST: [Mutex<VmInterface>; VM_NUM_MAX] = [
    Mutex::new(VmInterface::default()),
    Mutex::new(VmInterface::default()),
    Mutex::new(VmInterface::default()),
    Mutex::new(VmInterface::default()),
    Mutex::new(VmInterface::default()),
    Mutex::new(VmInterface::default()),
    Mutex::new(VmInterface::default()),
    Mutex::new(VmInterface::default()),
];

pub fn vm_if_list_set_state(vm_id: usize, vm_state: VmState) {
    let mut vm_if = VM_IF_LIST[vm_id].lock();
    vm_if.state = vm_state;
}

pub fn vm_if_list_get_state(vm_id: usize) -> VmState {
    let vm_if = VM_IF_LIST[vm_id].lock();
    vm_if.state
}

pub fn vm_if_list_set_type(vm_id: usize, vm_type: VmType) {
    let mut vm_if = VM_IF_LIST[vm_id].lock();
    vm_if.vm_type = vm_type;
}

pub fn vm_if_list_get_type(vm_id: usize) -> VmType {
    let vm_if = VM_IF_LIST[vm_id].lock();
    vm_if.vm_type
}

pub fn vm_if_list_set_cpu_id(vm_id: usize, master_cpu_id: usize) {
    let mut vm_if = VM_IF_LIST[vm_id].lock();
    vm_if.master_cpu_id = master_cpu_id;
    println!("vm_if_list_set_cpu_id vm [{}] set master_cpu_id {}",vm_id,master_cpu_id);
}

pub fn vm_if_list_get_cpu_id(vm_id: usize) -> usize {
    let vm_if = VM_IF_LIST[vm_id].lock();
    vm_if.master_cpu_id
}

pub fn vm_if_list_cmp_mac(vm_id: usize, frame: &[u8]) -> bool {
    let vm_if = VM_IF_LIST[vm_id].lock();
    for i in 0..6 {
        if vm_if.mac[i] != frame[i] {
            return false;
        }
    }
    true
}

pub fn vm_if_list_set_ivc_arg(vm_id: usize, ivc_arg: usize) {
    let mut vm_if = VM_IF_LIST[vm_id].lock();
    vm_if.ivc_arg = ivc_arg;
}

pub fn vm_if_list_ivc_arg(vm_id: usize) -> usize {
    let vm_if = VM_IF_LIST[vm_id].lock();
    vm_if.ivc_arg
}

pub fn vm_if_list_set_ivc_arg_ptr(vm_id: usize, ivc_arg_ptr: usize) {
    let mut vm_if = VM_IF_LIST[vm_id].lock();
    vm_if.ivc_arg_ptr = ivc_arg_ptr;
}

pub fn vm_if_list_ivc_arg_ptr(vm_id: usize) -> usize {
    let vm_if = VM_IF_LIST[vm_id].lock();
    vm_if.ivc_arg_ptr
}

#[derive(Clone, Copy)]
pub enum VmState {
    VmInv = 0,
    VmPending = 1,
    VmActive = 2,
}

#[derive(Clone, Copy, PartialEq)]
pub enum VmType {
    VmTOs = 0,
    VmTBma = 1,
}

impl VmType {
    pub fn from_usize(value : usize) -> VmType {
        match value {
            0 => VmType::VmTOs,
            1 => VmType::VmTBma,
            _ => panic!("Unknown value: {}", value),
        }
    }
}

pub struct VmInterface {
    pub master_cpu_id: usize,
    pub state: VmState,
    pub vm_type: VmType,
    pub mac: [u8; 6],
    pub ivc_arg: usize,
    pub ivc_arg_ptr: usize,
}

impl VmInterface {
    const fn default() -> VmInterface {
        VmInterface {
            master_cpu_id: 0,
            state: VmState::VmPending,
            vm_type: VmType::VmTBma,
            mac: [0; 6],
            ivc_arg: 0,
            ivc_arg_ptr: 0,
        }
    }
}

pub struct VmPa {
    pub pa_start: usize,
    pub pa_length: usize,
    pub offset: isize,
}

impl VmPa {
    pub fn default() -> VmPa {
        VmPa {
            pa_start: 0,
            pa_length: 0,
            offset: 0,
        }
    }
}

// #[repr(align(4096))]
#[derive(Clone)]
pub struct Vm {
    pub inner: Arc<Mutex<VmInner>>,
}

impl Vm {
    pub fn inner(&self) -> Arc<Mutex<VmInner>> {
        self.inner.clone()
    }

    pub fn default() -> Vm {
        Vm {
            inner: Arc::new(Mutex::new(VmInner::default())),
        }
    }

    pub fn new(id: usize) -> Vm {
        Vm {
            inner: Arc::new(Mutex::new(VmInner::new(id))),
        }
    }

    pub fn init_intc_mode(&self, emu: bool) {
        let vm_inner = self.inner.lock();
        for vcpu in &vm_inner.vcpu_list {
            println!("vm {} vcpu {} set {} hcr", vm_inner.id, vcpu.id(), if emu { "emu" } else { "partial passthrough" });
            if !emu {
                vcpu.set_gich_ctlr((GICC_CTLR_EN_BIT) as u32);
                vcpu.set_hcr(0x80080001); // HCR_EL2_GIC_PASSTHROUGH_VAL
            } else {
                vcpu.set_gich_ctlr((GICC_CTLR_EN_BIT | GICC_CTLR_EOIMODENS_BIT) as u32);
                vcpu.set_hcr(0x80080019);
            }
        }
    }

    pub fn med_blk_id(&self) -> usize {
        let vm_inner = self.inner.lock();
        match vm_inner.config.as_ref().unwrap().med_blk_idx {
            None => { panic!("vm {} do not have mediated blk", vm_inner.id); }
            Some(idx) => { idx }
        }
    }

    pub fn dtb(&self) -> Option<*mut fdt::myctypes::c_void> {
        let vm_inner = self.inner.lock();
        vm_inner.dtb.map(|x| x as *mut fdt::myctypes::c_void)
    }
    pub fn set_dtb(&self, val: *mut fdt::myctypes::c_void) {
        let mut vm_inner = self.inner.lock();
        vm_inner.dtb = Some(val as usize);
    }

    pub fn get_vcpu(&self, index: usize) -> Option<Vcpu>{
        let mut vm_inner = self.inner.lock();
        if vm_inner.vcpu_list.get(index).is_some() {
            return Some(vm_inner.vcpu_list[index].clone());
        } else {
            return None;
        }
    }

    pub fn push_vcpu(&self, vcpu: Vcpu) {
        let mut vm_inner = self.inner.lock();
        vm_inner.vcpu_list.push(vcpu);
    }

    pub fn set_has_master_cpu(&self, has_master: bool){
        let mut vm_inner = self.inner.lock();
        vm_inner.has_master = has_master;
    }

    pub fn has_master_cpu(&self) -> bool {
        let vm_inner = self.inner.lock();
        vm_inner.has_master
    }

    pub fn get_ncpu(&self) -> usize {
        let vm_inner = self.inner.lock();
        vm_inner.ncpu
    }

    pub fn set_ncpu(&self, ncpu: usize) {
        let mut vm_inner = self.inner.lock();
        vm_inner.ncpu = ncpu;
    }

    pub fn get_cpu_num(&self) -> usize {
        let vm_inner = self.inner.lock();
        vm_inner.cpu_num
    }

    pub fn set_cpu_num(&self, cpu_num: usize) {
        let mut vm_inner = self.inner.lock();
        vm_inner.cpu_num = cpu_num;
    }

    pub fn set_entry_point(&self, entry_point: usize) {
        let mut vm_inner = self.inner.lock();
        vm_inner.entry_point = entry_point;
    }

    pub fn set_emu_devs(&self, idx: usize, emu: EmuDevs) {
        let mut vm_inner = self.inner.lock();
        if idx < vm_inner.emu_devs.len() {
            if let EmuDevs::None = vm_inner.emu_devs[idx] {
                println!("set_emu_devs: cover a None emu dev");
                vm_inner.emu_devs[idx] = emu;
                return;
            } else {
                panic!("set_emu_devs: set an exsit emu dev");
            }
        }
        while idx > vm_inner.emu_devs.len() {
            println!("set_emu_devs: push a None emu dev");
            vm_inner.emu_devs.push(EmuDevs::None);
        }
        vm_inner.emu_devs.push(emu);
    }

    pub fn set_intc_dev_id(&self, intc_dev_id: usize) {
        let mut vm_inner = self.inner.lock();
        vm_inner.intc_dev_id = intc_dev_id;
    }

    pub fn set_int_bit_map(&self, int_id: usize) {
        let mut vm_inner = self.inner.lock();
        vm_inner.int_bitmap.as_mut().unwrap().set(int_id);
        // match vm_inner.int_bitmap {
        //     Some(mut bitmap) => {
        //         bitmap.set(int_id);
        //     }
        //     None => {
        //         panic!("vm {} bitmap is None", self.vm_id());
        //     }
        // }
    }

    pub fn pt_map_range(&self, ipa: usize, len: usize, pa: usize, pte: usize) {
        let vm_inner = self.inner.lock();
        match &vm_inner.pt {
            Some(pt) => pt.pt_map_range(ipa, len, pa, pte),
            None => {
                panic!("Vm::pt_map_range: vm{} pt is empty", vm_inner.id);
            }
        }
    }

    pub fn pt_dir(&self) -> usize {
        let vm_inner = self.inner.lock();
        match &vm_inner.pt {
            Some(pt) => return pt.base_pa(),
            None => {
                panic!("Vm::pt_dir: vm{} pt is empty", vm_inner.id);
            }
        }
    }

    pub fn cpu_num(&self) -> usize {
        let vm_inner = self.inner.lock();
        vm_inner.cpu_num
    }

    pub fn id(&self) -> usize {
        let vm_inner = self.inner.lock();
        vm_inner.id
    }

    pub fn config(&self) -> Arc<VmConfigEntry> {
        let vm_inner = self.inner.lock();
        vm_inner.config.as_ref().unwrap().clone()
    }

    pub fn pa_start(&self, idx: usize) -> usize {
        let vm_inner = self.inner.lock();
        vm_inner.pa_region.as_ref().unwrap()[idx].pa_start
    }

    pub fn pa_length(&self, idx: usize) -> usize {
        let vm_inner = self.inner.lock();
        vm_inner.pa_region.as_ref().unwrap()[idx].pa_length
    }

    pub fn pa_offset(&self, idx: usize) -> usize {
        let vm_inner = self.inner.lock();
        vm_inner.pa_region.as_ref().unwrap()[idx].offset as usize
    }

    pub fn mem_region_num(&self) -> usize {
        let vm_inner = self.inner.lock();
        vm_inner.mem_region_num
    }

    pub fn vcpu(&self, idx: usize) -> Vcpu {
        let vm_inner = self.inner.lock();
        vm_inner.vcpu_list[idx].clone()
    }

    pub fn vgic(&self) -> Arc<Vgic> {
        let vm_inner = self.inner.lock();
        match &vm_inner.emu_devs[vm_inner.intc_dev_id] {
            EmuDevs::Vgic(vgic) => {
                return vgic.clone();
            }
            _ => {
                panic!("vm{} cannot find vgic", vm_inner.id);
            }
        }
    }

    pub fn has_vgic(&self) -> bool {
        let vm_inner = self.inner.lock();
        if vm_inner.intc_dev_id >= vm_inner.emu_devs.len() {
            return false;
        }
        match &vm_inner.emu_devs[vm_inner.intc_dev_id] {
            EmuDevs::Vgic(vgic) => { true }
            _ => { false }
        }
    }

    pub fn emu_dev(&self, dev_id: usize) -> EmuDevs {
        let vm_inner = self.inner.lock();
        vm_inner.emu_devs[dev_id].clone()
    }

    pub fn emu_net_dev(&self, id: usize) -> EmuDevs {
        let vm_inner = self.inner.lock();
        let mut dev_num = 0;

        for i in 0..vm_inner.emu_devs.len() {
            match vm_inner.emu_devs[i] {
                EmuDevs::VirtioNet(_) => {
                    if dev_num == id {
                        return vm_inner.emu_devs[i].clone();
                    }
                    dev_num += 1;
                }
                _ => {}
            }
        }
        return EmuDevs::None;
    }

    pub fn emu_blk_dev(&self, id: usize) -> EmuDevs {
        let vm_inner = self.inner.lock();
        let mut dev_num = 0;

        for i in 0..vm_inner.emu_devs.len() {
            match vm_inner.emu_devs[i] {
                EmuDevs::VirtioBlk(_) => {
                    if dev_num == id {
                        return vm_inner.emu_devs[i].clone();
                    }
                    dev_num += 1;
                }
                _ => {}
            }
        }
        return EmuDevs::None;
    }

    // Get console dev by ipa.
    pub fn emu_console_dev(&self, ipa: u64) -> EmuDevs {
        let mut emu_dev_id = -1;
        for idx in 0..self.config().vm_emu_dev_confg.as_ref().unwrap().len() {
            if self.config().vm_emu_dev_confg.as_ref().unwrap()[idx].base_ipa == ipa as usize {
                emu_dev_id = idx as i32;
            }
        }
        if emu_dev_id > 0 {
            let vm_inner = self.inner.lock();
            return vm_inner.emu_devs[emu_dev_id as usize].clone();
        }
        return EmuDevs::None;
    }

    pub fn ncpu(&self) -> usize {
        let vm_inner = self.inner.lock();
        vm_inner.ncpu
    }

    pub fn has_interrupt(&self, int_id: usize) -> bool {
        let mut vm_inner = self.inner.lock();
        vm_inner.int_bitmap.as_mut().unwrap().get(int_id) != 0
        // match vm_inner.int_bitmap {
        //     Some(mut bitmap) => {
        //         if int_id == 27 {
        //             println!("bitmap 27 is {}", bitmap.get(int_id));
        //         }
        //         return bitmap.get(int_id) != 0;
        //     }
        //     None => {
        //         panic!("vm {} bitmap is None", self.vm_id());
        //     }
        // }
    }

    pub fn emu_has_interrupt(&self, int_id: usize) -> bool {
        let vm_config = DEF_VM_CONFIG_TABLE.lock();
        let vm_id = self.id();
        match &vm_config.entries[vm_id].vm_emu_dev_confg {
            Some(emu_devs) => {
                for emu_dev in emu_devs {
                    if int_id == emu_dev.irq_id {
                        return true;
                    }
                }
            }
            None => return false,
        }
        false
    }

    pub fn vcpuid_to_pcpuid(&self, vcpuid: usize) -> Result<usize, ()> {
        // println!("vcpuid_to_pcpuid");
        let vm_inner = self.inner.lock();
        if vcpuid < vm_inner.cpu_num {
            let vcpu = vm_inner.vcpu_list[vcpuid].clone();
            drop(vm_inner);
            return Ok(vcpu.phys_id());
        } else {
            return Err(());
        }
    }

    pub fn pcpuid_to_vcpuid(&self, pcpuid: usize) -> Result<usize, ()> {
        let vm_inner = self.inner.lock();
        for vcpuid in 0..vm_inner.cpu_num {
            if vm_inner.vcpu_list[vcpuid].phys_id() == pcpuid {
                return Ok(vcpuid);
            }
        }
        return Err(());
    }

    pub fn vcpu_to_pcpu_mask(&self, mask: usize, len: usize) -> usize {
        let mut pmask = 0;
        for i in 0..len {
            let shift = self.vcpuid_to_pcpuid(i);
            if mask & (1 << i) != 0 && !shift.is_err() {
                pmask |= 1 << shift.unwrap();
            }
        }
        return pmask;
    }

    pub fn pcpu_to_vcpu_mask(&self, mask: usize, len: usize) -> usize {
        let mut pmask = 0;
        for i in 0..len {
            let shift = self.pcpuid_to_vcpuid(i);
            if mask & (1 << i) != 0 && !shift.is_err() {
                pmask |= 1 << shift.unwrap();
            }
        }
        return pmask;
    }

    pub fn show_pagetable(&self, ipa: usize) {
        let vm_inner = self.inner.lock();
        vm_inner.pt.as_ref().unwrap().show_pt(ipa);
    }

    pub fn ready(&self) -> bool {
        let vm_inner = self.inner.lock();
        vm_inner.ready
    }

    pub fn set_ready(&self, _ready: bool) {
        let mut vm_inner = self.inner.lock();
        vm_inner.ready = _ready;
    }
}

#[repr(align(4096))]
pub struct VmInner {
    pub id: usize,
    pub ready: bool,
    pub config: Option<Arc<VmConfigEntry>>,
    pub dtb: Option<usize>,
    // memory config
    pub pt: Option<PageTable>,
    pub mem_region_num: usize,
    pub pa_region: Option<[VmPa; VM_MEM_REGION_MAX]>,

    // image config
    pub entry_point: usize,

    // vcpu config
    pub has_master: bool,
    pub vcpu_list: Vec<Vcpu>,
    pub cpu_num: usize,
    pub ncpu: usize,

    // interrupt
    pub intc_dev_id: usize,
    pub int_bitmap: Option<BitMap<BitAlloc256>>,

    // emul devs
    pub emu_devs: Vec<EmuDevs>,
}

impl VmInner {
    pub const fn default() -> VmInner {
        VmInner {
            id: 0,
            ready: false,
            config: None,
            dtb: None,
            pt: None,
            mem_region_num: 0,
            pa_region: None,
            entry_point: 0,

            has_master: false,
            vcpu_list: Vec::new(),
            cpu_num: 0,
            ncpu: 0,

            intc_dev_id: 0,
            int_bitmap: Some(BitAlloc4K::default()),
            emu_devs: Vec::new(),
        }
    }

    pub fn new(id: usize) -> VmInner {
        VmInner {
            id,
            ready: false,
            config: None,
            dtb: None,
            pt: None,
            mem_region_num: 0,
            pa_region: None,
            entry_point: 0,

            has_master: false,
            vcpu_list: Vec::new(),
            cpu_num: 0,
            ncpu: 0,

            intc_dev_id: 0,
            int_bitmap: Some(BitAlloc4K::default()),
            emu_devs: Vec::new(),
        }
    }
}

// static VM_LIST: Mutex<[Vm; VM_NUM_MAX]> = Mutex::new([Vm::default(); VM_NUM_MAX]);
// lazy_static! {
//     pub static ref VM_LIST: Mutex<[Vm; VM_NUM_MAX]> = Mutex::new([Vm::default(); VM_NUM_MAX]);
// }
// pub static VM_LIST: Mutex<[Vm; VM_NUM_MAX]> = Mutex::new([
//     Vm::default(),
//     Vm::default(),
//     Vm::default(),
//     Vm::default(),
//     Vm::default(),
//     Vm::default(),
//     Vm::default(),
//     Vm::default(),
// ]);
pub static VM_LIST: Mutex<Vec<Vm>> = Mutex::new(Vec::new());

pub fn vm(id: usize) -> Vm {
    let vm_list = VM_LIST.lock();
    vm_list[id].clone()
}

pub fn get_vm_by_index(id: usize) -> Option<Vm> {
    let vm_list = VM_LIST.lock();
    if vm_list.get(id).is_none() {
        return None;
    } else {
        return Some(vm_list[id].clone());
    }
}

pub fn vm_list_size() -> usize {
    let vm_list = VM_LIST.lock();
    vm_list.len()
}

pub fn vm_ipa2pa(vm: Vm, ipa: usize) -> usize {
    // let vm = match active_vm() {
    //     Some(vm) => vm,
    //     None => {
    //         panic!("vm_ipa2pa: current vcpu.vm is none");
    //     }
    // };

    if ipa == 0 {
        return 0;
    }

    for i in 0..vm.mem_region_num() {
        if in_range(
            (ipa as isize - vm.pa_offset(i) as isize) as usize,
            vm.pa_start(i),
            vm.pa_length(i),
        ) {
            return (ipa as isize - vm.pa_offset(i) as isize) as usize;
        }
    }

    println!("vm_ipa2pa: VM {} access invalid ipa {:x}", vm.id(), ipa);
    return 0;
}
