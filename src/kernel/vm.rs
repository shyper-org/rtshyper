use super::mem::VM_MEM_REGION_MAX;
use super::vcpu::Vcpu;
use crate::config::DEF_VM_CONFIG_TABLE;
use crate::lib::*;
use alloc::sync::Arc;
use alloc::vec::Vec;
use spin::Mutex;

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

pub fn vm_if_list_set_type(vm_id: usize, vm_type: VmType) {
    let mut vm_if = VM_IF_LIST[vm_id].lock();
    vm_if.vm_type = vm_type;
}

pub fn vm_if_list_get_type(vm_id: usize) -> VmType {
    let vm_if = VM_IF_LIST[vm_id].lock();
    vm_if.vm_type
}

pub enum VmState {
    VmInv = 0,
    VmPending = 1,
    VmActive = 2,
}

#[derive(Clone, Copy)]
pub enum VmType {
    VmTOs = 0,
    VmTBma = 1,
}

pub struct VmInterface {
    pub master_vcpu_id: usize,
    pub state: VmState,
    pub vm_type: VmType,
}

impl VmInterface {
    const fn default() -> VmInterface {
        VmInterface {
            master_vcpu_id: 0,
            state: VmState::VmPending,
            vm_type: VmType::VmTBma,
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

use crate::config::VmConfigEntry;

#[repr(align(4096))]
#[derive(Clone)]
pub struct Vm {
    pub inner: Arc<Mutex<VmInner>>,
}

use crate::arch::Vgic;
impl Vm {
    pub fn inner(&self) -> Arc<Mutex<VmInner>> {
        self.inner.clone()
    }

    #[allow(dead_code)]
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

    pub fn push_vcpu(&self, vcpu: Vcpu) {
        let mut vm_inner = self.inner.lock();
        vm_inner.vcpu_list.push(vcpu);
    }

    pub fn set_ncpu(&self, ncpu: usize) {
        let mut vm_inner = self.inner.lock();
        vm_inner.ncpu = ncpu;
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
                panic!("Vm::pt_map_range: vm pt is empty");
            }
        }
    }

    pub fn pt_dir(&self) -> usize {
        let vm_inner = self.inner.lock();
        match &vm_inner.pt {
            Some(pt) => return pt.base_pa(),
            None => {
                panic!("Vm::pt_map_range: vm pt is empty");
            }
        }
    }

    pub fn cpu_num(&self) -> usize {
        let vm_inner = self.inner.lock();
        vm_inner.cpu_num
    }

    pub fn vm_id(&self) -> usize {
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
                panic!("cannot find vgic");
            }
        }
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
        let vm_id = self.vm_id();
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
        let vm_inner = self.inner.lock();
        if vcpuid < vm_inner.cpu_num {
            return Ok(vm_inner.vcpu_list[vcpuid].phys_id());
        } else {
            return Err(());
        }
    }

    #[allow(dead_code)]
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
            let shift = self.vcpuid_to_pcpuid(i);
            if mask & (1 << i) != 0 && !shift.is_err() {
                pmask |= 1 << shift.unwrap();
            }
        }
        return pmask;
    }
}

use crate::arch::PageTable;
use crate::device::EmuDevs;
#[repr(align(4096))]
pub struct VmInner {
    pub id: usize,
    pub config: Option<Arc<VmConfigEntry>>,

    // memory config
    pub pt: Option<PageTable>,
    pub mem_region_num: usize,
    pub pa_region: Option<[VmPa; VM_MEM_REGION_MAX]>,

    // image config
    pub entry_point: usize,

    // vcpu config
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
            config: None,
            pt: None,
            mem_region_num: 0,
            pa_region: None,
            entry_point: 0,
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
            config: None,
            pt: None,
            mem_region_num: 0,
            pa_region: None,
            entry_point: 0,
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
