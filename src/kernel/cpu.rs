use alloc::vec::Vec;

use spin::Mutex;

use crate::arch::{PAGE_SIZE, pt_map_banked_cpu, PTE_PER_PAGE};
use crate::arch::ContextFrame;
use crate::arch::ContextFrameTrait;
// use core::ops::{Deref, DerefMut};
use crate::arch::cpu_interrupt_unmask;
use crate::board::PLATFORM_CPU_NUM_MAX;
use crate::kernel::{SchedType, Vcpu, VcpuArray, VcpuState, Vm, Scheduler};
use crate::kernel::IpiMessage;
use crate::lib::trace;

pub const CPU_MASTER: usize = 0;
pub const CPU_STACK_SIZE: usize = PAGE_SIZE * 128;
pub const CONTEXT_GPR_NUM: usize = 31;

#[repr(C)]
#[repr(align(4096))]
#[derive(Copy, Clone, Debug, Eq)]
pub struct CpuPt {
    pub lvl1: [usize; PTE_PER_PAGE],
    pub lvl2: [usize; PTE_PER_PAGE],
    pub lvl3: [usize; PTE_PER_PAGE],
}

impl PartialEq for CpuPt {
    fn eq(&self, other: &Self) -> bool {
        self.lvl1 == other.lvl1 && self.lvl2 == other.lvl2 && self.lvl3 == other.lvl3
    }
}

#[derive(Copy, Clone, Debug, Eq)]
pub enum CpuState {
    CpuInv = 0,
    CpuIdle = 1,
    CpuRun = 2,
}

impl PartialEq for CpuState {
    fn eq(&self, other: &Self) -> bool {
        *self as usize == *other as usize
    }
}

pub struct CpuIf {
    pub msg_queue: Vec<IpiMessage>,
}

impl CpuIf {
    pub fn default() -> CpuIf {
        CpuIf { msg_queue: Vec::new() }
    }

    pub fn push(&mut self, ipi_msg: IpiMessage) {
        self.msg_queue.push(ipi_msg);
    }

    pub fn pop(&mut self) -> Option<IpiMessage> {
        self.msg_queue.pop()
    }
}

pub static CPU_IF_LIST: Mutex<Vec<CpuIf>> = Mutex::new(Vec::new());

fn cpu_if_init() {
    let mut cpu_if_list = CPU_IF_LIST.lock();
    for _ in 0..PLATFORM_CPU_NUM_MAX {
        cpu_if_list.push(CpuIf::default());
    }
}

#[repr(C)]
#[repr(align(4096))]
// #[derive(Clone)]
pub struct Cpu {
    pub id: usize,
    pub cpu_state: CpuState,
    pub active_vcpu: Option<Vcpu>,
    pub ctx: Option<usize>,

    pub sched: SchedType,
    pub vcpu_array: VcpuArray,
    pub current_irq: usize,
    pub cpu_pt: CpuPt,
    pub stack: [u8; CPU_STACK_SIZE],
}

impl Cpu {
    const fn default() -> Cpu {
        Cpu {
            id: 0,
            cpu_state: CpuState::CpuInv,
            active_vcpu: None,
            ctx: None,
            sched: SchedType::None,
            vcpu_array: VcpuArray::new(),
            current_irq: 0,
            cpu_pt: CpuPt {
                lvl1: [0; PTE_PER_PAGE],
                lvl2: [0; PTE_PER_PAGE],
                lvl3: [0; PTE_PER_PAGE],
            },
            stack: [0; CPU_STACK_SIZE],
        }
    }

    pub fn set_ctx(&mut self, ctx: *mut ContextFrame) {
        self.ctx = Some(ctx as usize);
    }

    pub fn clear_ctx(&mut self) {
        self.ctx = None;
    }

    pub fn set_gpr(&self, idx: usize, val: usize) {
        if idx >= CONTEXT_GPR_NUM {
            return;
        }
        match self.ctx {
            Some(ctx_addr) => {
                if trace() && ctx_addr < 0x1000 {
                    panic!("illegal ctx addr {:x}", ctx_addr);
                }
                let ctx = ctx_addr as *mut ContextFrame;
                unsafe {
                    (*ctx).set_gpr(idx, val);
                }
            }
            None => {}
        }
    }

    pub fn get_gpr(&self, idx: usize) -> usize {
        if idx >= CONTEXT_GPR_NUM {
            return 0;
        }
        match self.ctx {
            Some(ctx_addr) => {
                if trace() && ctx_addr < 0x1000 {
                    panic!("illegal ctx addr {:x}", ctx_addr);
                }
                let ctx = ctx_addr as *mut ContextFrame;
                unsafe { (*ctx).gpr(idx) }
            }
            None => 0,
        }
    }

    pub fn get_elr(&self) -> usize {
        match self.ctx {
            Some(ctx_addr) => {
                if trace() && ctx_addr < 0x1000 {
                    panic!("illegal ctx addr {:x}", ctx_addr);
                }
                let ctx = ctx_addr as *mut ContextFrame;
                unsafe { (*ctx).exception_pc() }
            }
            None => 0,
        }
    }

    pub fn get_spsr(&self) -> usize {
        match self.ctx {
            Some(ctx_addr) => {
                if trace() && ctx_addr < 0x1000 {
                    panic!("illegal ctx addr {:x}", ctx_addr);
                }
                let ctx = ctx_addr as *mut ContextFrame;
                unsafe { (*ctx).spsr as usize }
            }
            None => 0,
        }
    }

    pub fn set_elr(&self, val: usize) {
        match self.ctx {
            Some(ctx_addr) => {
                if trace() && ctx_addr < 0x1000 {
                    panic!("illegal ctx addr {:x}", ctx_addr);
                }
                let ctx = ctx_addr as *mut ContextFrame;
                unsafe { (*ctx).set_exception_pc(val) }
            }
            None => {}
        }
    }

    pub fn set_active_vcpu(&mut self, active_vcpu: Option<Vcpu>) {
        self.active_vcpu = active_vcpu.clone();
        match active_vcpu {
            None => {}
            Some(vcpu) => {
                vcpu.set_state(VcpuState::VcpuAct);
                vcpu.context_vm_restore();
                // restore vm's Stage2 MMU context
                let vttbr = (vcpu.vm_id() << 48) | vcpu.vm_pt_dir();
                // println!("vttbr {:#x}", vttbr);
                // TODO: replace the arch related expr
                unsafe {
                    core::arch::asm!("msr VTTBR_EL2, {0}", "isb", in(reg) vttbr);
                }
            }
        }
    }

    pub fn schedule_to(&mut self, next_vcpu: Vcpu) {
        if let Some(prev_vcpu) = &self.active_vcpu {
            if prev_vcpu.vm_id() != next_vcpu.vm_id() {
                // println!(
                //     "next vm{} vcpu {}, prev vm{} vcpu {}",
                //     next_vcpu.vm_id(),
                //     next_vcpu.id(),
                //     prev_vcpu.vm_id(),
                //     prev_vcpu.id()
                // );
                prev_vcpu.set_state(VcpuState::VcpuPend);
                prev_vcpu.context_vm_store();
            }
        }
        // NOTE: Must set active first and then restore context!!!
        //      because context restore while inject pending interrupt for VM
        //      and will judge if current active vcpu
        self.set_active_vcpu(Some(next_vcpu.clone()));
    }

    pub fn scheduler(&mut self) -> &mut impl Scheduler {
        match &mut self.sched {
            SchedType::None => panic!("scheduler is None"),
            SchedType::SchedRR(rr) => rr,
        }
    }

    pub fn assigned(&self) -> bool {
        self.vcpu_array.vcpu_num() != 0
    }
}

#[no_mangle]
#[link_section = ".cpu_private"]
pub static mut CPU: Cpu = Cpu {
    id: 0,
    cpu_state: CpuState::CpuInv,
    active_vcpu: None,
    ctx: None,
    vcpu_array: VcpuArray::new(),
    sched: SchedType::None,
    current_irq: 0,
    cpu_pt: CpuPt {
        lvl1: [0; PTE_PER_PAGE],
        lvl2: [0; PTE_PER_PAGE],
        lvl3: [0; PTE_PER_PAGE],
    },
    stack: [0; CPU_STACK_SIZE],
};

pub fn current_cpu() -> &'static mut Cpu {
    unsafe { &mut CPU }
}

pub fn active_vcpu_id() -> usize {
    let active_vcpu = current_cpu().active_vcpu.clone().unwrap();
    active_vcpu.id()
}

pub fn active_vm_id() -> usize {
    let vm = active_vm().unwrap();
    vm.id()
}

pub fn active_vm() -> Option<Vm> {
    match current_cpu().active_vcpu.clone() {
        None => {
            return None;
        }
        Some(active_vcpu) => {
            return active_vcpu.vm();
        }
    }
}

pub fn active_vm_ncpu() -> usize {
    match active_vm() {
        Some(vm) => vm.ncpu(),
        None => 0,
    }
}

pub fn cpu_init() {
    let cpu_id = current_cpu().id;
    if cpu_id == 0 {
        use crate::arch::power_arch_init;
        use crate::board::platform_power_on_secondary_cores;
        platform_power_on_secondary_cores();
        power_arch_init();
        cpu_if_init();
    }

    let state = CpuState::CpuIdle;
    current_cpu().cpu_state = state;
    let sp = current_cpu().stack.as_ptr() as usize + CPU_STACK_SIZE;
    let size = core::mem::size_of::<ContextFrame>();
    current_cpu().set_ctx((sp - size) as *mut _);
    println!("Core {} init ok", cpu_id);

    crate::lib::barrier();
    // println!("after barrier cpu init");
    use crate::board::PLAT_DESC;
    if cpu_id == 0 {
        println!("Bring up {} cores", PLAT_DESC.cpu_desc.num);
        println!("Cpu init ok");
    }
}

pub fn cpu_idle() -> ! {
    let state = CpuState::CpuIdle;
    current_cpu().cpu_state = state;
    cpu_interrupt_unmask();
    loop {
        // TODO: replace it with an Arch function `arch_idle`
        cortex_a::asm::wfi();
    }
}

pub static mut CPU_LIST: [Cpu; PLATFORM_CPU_NUM_MAX] = [
    Cpu::default(),
    Cpu::default(),
    Cpu::default(),
    Cpu::default(),
    Cpu::default(),
    Cpu::default(),
    Cpu::default(),
    Cpu::default(),
];

#[no_mangle]
// #[link_section = ".text.boot"]
pub extern "C" fn cpu_map_self(cpu_id: usize) -> usize {
    let mut cpu = unsafe { &mut CPU_LIST[cpu_id] };
    (*cpu).id = cpu_id;

    let lvl1_addr = pt_map_banked_cpu(cpu);

    lvl1_addr
}
