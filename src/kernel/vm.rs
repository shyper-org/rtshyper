use super::mem::VM_MEM_REGION_MAX;
use super::vcpu::Vcpu;
use crate::board::PLATFORM_VCPU_NUM_MAX;
use crate::lib::*;
use spin::Mutex;

pub const VM_NUM_MAX: usize = 8;

enum VmState {
    VmInv = 0,
    VmPending = 1,
    VmActive = 2,
}

#[derive(Copy, Clone)]
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

#[repr(align(4096))]
#[derive(Copy, Clone)]
pub struct Vm {
    // memory config
    id: usize,
    pt_dir: usize,
    mem_region_num: usize,
    pa_region: Option<[VmPa; VM_MEM_REGION_MAX]>,

    // image config
    entry_point: u64,

    // vcpu config
    vcpu_list: Option<[Vcpu; PLATFORM_VCPU_NUM_MAX]>,
    cpu_num: u64,
    ncpu: u64,

    // interrupt
    intc_dev_id: u64,
    int_bitmap: Option<BitMap<BitAlloc256>>, // TODO emul devs
}

impl Vm {
    pub const fn default() -> Vm {
        Vm {
            id: 0,
            pt_dir: 0,
            mem_region_num: 0,
            pa_region: None,
            entry_point: 0,
            vcpu_list: None,
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
pub static VM_LIST: Mutex<[Vm; VM_NUM_MAX]> = Mutex::new([Vm::default(); VM_NUM_MAX]);
