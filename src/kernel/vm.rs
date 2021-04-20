use super::mem::VM_MEM_REGION_MAX;
use super::vcpu::Vcpu;
use crate::board::PLATFORM_VCPU_NUM_MAX;
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

pub enum VmState {
    VmInv = 0,
    VmPending = 1,
    VmActive = 2,
}

pub struct VmInterface {
    pub master_vcpu_id: usize,
    pub state: VmState,
}

impl VmInterface {
    const fn default() -> VmInterface {
        VmInterface {
            master_vcpu_id: 0,
            state: VmState::VmPending,
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

    pub fn cpu_num(&self) -> usize {
        let mut vm_inner = self.inner.lock();
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

    pub fn vcpu(&self, idx: usize) -> Arc<Mutex<Vcpu>> {
        let vm_inner = self.inner.lock();
        vm_inner.vcpu_list[idx].clone()
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
    pub vcpu_list: Vec<Arc<Mutex<Vcpu>>>,
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
            int_bitmap: None,
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
            int_bitmap: None,
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
