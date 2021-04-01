use spin::Mutex;
use crate::board::PLATFORM_CPU_NUM_MAX;
use crate::arch::pt_map_banked_cpu;


pub const CPU_MASTER: usize = 0;

#[derive(Copy, Clone)]
pub enum CpuState {
    CpuInv = 0,
    CpuIdle = 1,
    CpuRun = 2,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Cpu {
    pub id: usize,
    pub assigned: bool,
    pub cpu_state: CpuState,
    // TODO
}

impl Cpu {
    const fn default() -> Cpu {
        Cpu {
            id: 0,
            assigned: false,
            cpu_state: CpuState::CpuInv,
        }
    }
}

#[no_mangle]
#[link_section = ".cpu_private"]
static CPU: Cpu = Cpu {
    id: 0,
    assigned: false,
    cpu_state: CpuState::CpuInv,
};

pub fn cpu_init() {
    // println!("{:x}", CPU as usize);
}

static CPU_LIST: Mutex<[Cpu; PLATFORM_CPU_NUM_MAX]> = Mutex::new([Cpu::default(); PLATFORM_CPU_NUM_MAX]); 

#[no_mangle]
#[link_section = ".text.boot"]
pub extern "C" fn cpu_map_self(cpu_id: usize) {
    let mut cpu_lock = CPU_LIST.lock();
    let mut cpu =  &mut (*cpu_lock)[cpu_id];
    (*cpu).id = cpu_id;

    pt_map_banked_cpu(cpu);
    println!("current cpu id: {}", (*cpu_lock)[cpu_id].id);
    
    drop(cpu_lock);
}
