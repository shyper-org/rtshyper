use crate::arch::{pt_map_banked_cpu, PAGE_SIZE, PTE_PER_PAGE};
use crate::board::PLATFORM_CPU_NUM_MAX;
use spin::Mutex;

pub const CPU_MASTER: usize = 0;
pub const CPU_STACK_SIZE: usize = PAGE_SIZE * 32;

#[repr(C)]
#[repr(align(4096))]
#[derive(Copy, Clone)]
pub struct CpuPt {
    pub lvl1: [usize; PTE_PER_PAGE],
    pub lvl2: [usize; PTE_PER_PAGE],
    pub lvl3: [usize; PTE_PER_PAGE],
}

#[derive(Copy, Clone)]
pub enum CpuState {
    CpuInv = 0,
    CpuIdle = 1,
    CpuRun = 2,
}

#[repr(C)]
#[repr(align(4096))]
#[derive(Copy, Clone)]
pub struct Cpu {
    pub id: usize,
    pub assigned: bool,
    pub cpu_state: CpuState,

    pub current_irq: u64,
    pub cpu_pt: CpuPt, // TODO
    pub stack: [u8; CPU_STACK_SIZE],
}

impl Cpu {
    const fn default() -> Cpu {
        Cpu {
            id: 0,
            assigned: false,
            cpu_state: CpuState::CpuInv,

            current_irq: 0,
            cpu_pt: CpuPt {
                lvl1: [0; PTE_PER_PAGE],
                lvl2: [0; PTE_PER_PAGE],
                lvl3: [0; PTE_PER_PAGE],
            },
            stack: [0; CPU_STACK_SIZE],
        }
    }
}

#[no_mangle]
#[link_section = ".cpu_private"]
pub static mut CPU: Cpu = Cpu {
    id: 0,
    assigned: false,
    cpu_state: CpuState::CpuInv,

    current_irq: 0,
    cpu_pt: CpuPt {
        lvl1: [0; PTE_PER_PAGE],
        lvl2: [0; PTE_PER_PAGE],
        lvl3: [0; PTE_PER_PAGE],
    },
    stack: [0; CPU_STACK_SIZE],
};

pub fn cpu_id() -> usize {
    unsafe { CPU.id }
}

pub fn set_cpu_state(state: CpuState) {
    unsafe {
        CPU.cpu_state = state;
    }
}

pub fn cpu_init() {
    let cpu_id = cpu_id();
    // println!("cpu id {}", cpu_id);
    if cpu_id == 0 {
        use crate::board::{platform_power_on_secondary_cores, power_arch_init};
        platform_power_on_secondary_cores();
        power_arch_init();
    }

    set_cpu_state(CpuState::CpuIdle);
    println!("Core {} init ok", cpu_id);

    use crate::lib::barrier;
    barrier();

    use crate::board::PLAT_DESC;
    if cpu_id == 0 {
        println!("Bring up {} cores", PLAT_DESC.cpu_desc.num);
        println!("Cpu init ok");
    }
}

static CPU_LIST: Mutex<[Cpu; PLATFORM_CPU_NUM_MAX]> =
    Mutex::new([Cpu::default(); PLATFORM_CPU_NUM_MAX]);

// static mut CPU_LIST: [Cpu; PLATFORM_CPU_NUM_MAX] = [Cpu::default(); PLATFORM_CPU_NUM_MAX];

#[no_mangle]
#[link_section = ".text.boot"]
pub extern "C" fn cpu_map_self(cpu_id: usize) -> usize {
    let mut cpu_lock = CPU_LIST.lock();
    let mut cpu = &mut (*cpu_lock)[cpu_id];
    // let mut cpu = unsafe { &mut CPU_LIST[cpu_id] };
    (*cpu).id = cpu_id;

    let lvl1_addr = pt_map_banked_cpu(cpu);

    lvl1_addr
}
