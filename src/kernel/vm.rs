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

enum VmState {
    VmInv = 0,
    VmPending = 1,
    VmActive = 2,
}

pub struct VmInterface {
    pub master_vcpu_id: usize,
}

impl VmInterface {
    const fn default() -> VmInterface {
        VmInterface { master_vcpu_id: 0 }
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
}

#[repr(align(4096))]
pub struct VmInner {
    pub id: usize,
    pub config: Option<Arc<VmConfigEntry>>,

    // memory config
    pub pt_dir: usize,
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
    pub int_bitmap: Option<BitMap<BitAlloc256>>, // TODO emul devs
}

impl VmInner {
    pub const fn default() -> VmInner {
        VmInner {
            id: 0,
            config: None,
            pt_dir: 0,
            mem_region_num: 0,
            pa_region: None,
            entry_point: 0,
            vcpu_list: Vec::new(),
            cpu_num: 0,
            ncpu: 0,

            intc_dev_id: 0,
            int_bitmap: None,
        }
    }

    pub fn new(id: usize) -> VmInner {
        VmInner {
            id,
            config: None,
            pt_dir: 0,
            mem_region_num: 0,
            pa_region: None,
            entry_point: 0,
            vcpu_list: Vec::new(),
            cpu_num: 0,
            ncpu: 0,

            intc_dev_id: 0,
            int_bitmap: None,
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
