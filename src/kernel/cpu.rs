use crate::arch::{pt_map_banked_cpu, PAGE_SIZE, PTE_PER_PAGE};
use crate::board::PLATFORM_CPU_NUM_MAX;
use crate::kernel::{Vcpu, VcpuPool, VcpuState, Vm};
use alloc::boxed::Box;
use alloc::vec::Vec;
// use core::ops::{Deref, DerefMut};
use crate::arch::cpu_interrupt_unmask;
use crate::arch::ContextFrame;
use crate::arch::ContextFrameTrait;
use crate::kernel::IpiMessage;
use spin::Mutex;

pub const CPU_MASTER: usize = 0;
pub const CPU_STACK_SIZE: usize = PAGE_SIZE * 128;
pub const CONTEXT_GPR_NUM: usize = 31;

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

pub struct CpuIf {
    pub msg_queue: Vec<IpiMessage>,
}

impl CpuIf {
    pub fn default() -> CpuIf {
        CpuIf {
            msg_queue: Vec::new(),
        }
    }

    #[allow(dead_code)]
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

// struct CpuSelf {}

// impl Deref<Cpu> for CpuSelf {}

#[repr(C)]
#[repr(align(4096))]
// #[derive(Clone)]
pub struct Cpu {
    pub id: usize,
    pub assigned: bool,
    pub cpu_state: CpuState,
    pub active_vcpu: Option<Vcpu>,
    pub ctx: Option<usize>,
    pub vcpu_pool: Option<Box<VcpuPool>>,

    pub current_irq: usize,
    pub cpu_pt: CpuPt,
    pub stack: [u8; CPU_STACK_SIZE],
}

impl Cpu {
    const fn default() -> Cpu {
        Cpu {
            id: 0,
            assigned: false,
            cpu_state: CpuState::CpuInv,
            active_vcpu: None,
            ctx: None,
            vcpu_pool: None,
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
                let ctx = ctx_addr as *mut ContextFrame;
                unsafe { (*ctx).gpr(idx) }
            }
            None => 0,
        }
    }

    pub fn get_elr(&self) -> usize {
        match self.ctx {
            Some(ctx_addr) => {
                let ctx = ctx_addr as *mut ContextFrame;
                unsafe { (*ctx).exception_pc() }
            }
            None => 0,
        }
    }

    pub fn set_elr(&self, val: usize) {
        match self.ctx {
            Some(ctx_addr) => {
                let ctx = ctx_addr as *mut ContextFrame;
                unsafe { (*ctx).set_exception_pc(val) }
            }
            None => {}
        }
    }
}

#[no_mangle]
#[link_section = ".cpu_private"]
pub static mut CPU: Cpu = Cpu {
    id: 0,
    assigned: false,
    cpu_state: CpuState::CpuInv,
    active_vcpu: None,
    ctx: None,
    vcpu_pool: None,
    current_irq: 0,
    cpu_pt: CpuPt {
        lvl1: [0; PTE_PER_PAGE],
        lvl2: [0; PTE_PER_PAGE],
        lvl3: [0; PTE_PER_PAGE],
    },
    stack: [0; CPU_STACK_SIZE],
};

// set/get CPU
pub fn cpu_id() -> usize {
    unsafe { CPU.id }
}

pub fn cpu_assigned() -> bool {
    unsafe { CPU.assigned }
}

pub fn active_vcpu() -> Result<Vcpu, ()> {
    unsafe {
        if CPU.active_vcpu.is_none() {
            return Err(());
        }
        Ok(CPU.active_vcpu.as_ref().unwrap().clone())
    }
}

pub fn active_vcpu_id() -> usize {
    let active_vcpu = active_vcpu().unwrap();
    active_vcpu.id()
}

pub fn active_vm_id() -> usize {
    let vm = active_vm().unwrap();
    vm.vm_id()
}

pub fn active_vm() -> Result<Vm, ()> {
    if active_vcpu().is_err() {
        return Err(());
    }
    let active_vcpu = active_vcpu().unwrap();

    match active_vcpu.vm() {
        Ok(vm) => Ok(vm),
        Err(_) => Err(()),
    }
}

pub fn active_vm_ncpu() -> usize {
    match active_vm() {
        Ok(vm) => vm.ncpu(),
        Err(_) => 0,
    }
}

pub fn cpu_vcpu_pool_size() -> usize {
    unsafe {
        let vcpu_pool = CPU.vcpu_pool.as_ref().unwrap();
        vcpu_pool.content.len()
    }
}

pub fn cpu_vcpu_pool() -> &'static Box<VcpuPool> {
    unsafe {
        let vcpu_pool = CPU.vcpu_pool.as_ref().unwrap();
        vcpu_pool
    }
}

pub fn cpu_current_irq() -> usize {
    unsafe { CPU.current_irq }
}

pub fn context_get_gpr(idx: usize) -> usize {
    unsafe { CPU.get_gpr(idx) }
}

pub fn get_cpu_ctx_elr() -> usize {
    unsafe { CPU.get_elr() }
}

pub fn set_cpu_assign(assigned: bool) {
    unsafe {
        CPU.assigned = assigned;
    }
}

pub fn set_cpu_vcpu_pool(pool: Box<VcpuPool>) {
    unsafe {
        CPU.vcpu_pool = Some(pool);
    }
}

pub fn set_cpu_state(state: CpuState) {
    unsafe {
        CPU.cpu_state = state;
    }
}

pub fn set_active_vcpu(idx: usize) {
    unsafe {
        let vcpu_pool = CPU.vcpu_pool.as_mut().unwrap();
        let vcpu = vcpu_pool.content[idx].vcpu.clone();
        vcpu_pool.active_idx = idx;

        vcpu.set_state(VcpuState::VcpuAct);
        CPU.active_vcpu = Some(vcpu);
    }
}

pub fn set_cpu_current_irq(irq: usize) {
    unsafe {
        CPU.current_irq = irq;
    }
}

pub fn set_cpu_ctx(ctx: *mut ContextFrame) {
    unsafe {
        CPU.set_ctx(ctx);
    }
}

pub fn cpu_ctx() -> Option<usize> {
    unsafe { CPU.ctx }
}

pub fn clear_cpu_ctx() {
    unsafe {
        CPU.clear_ctx();
    }
}

pub fn context_set_gpr(idx: usize, val: usize) {
    unsafe {
        CPU.set_gpr(idx, val);
    }
}

pub fn set_cpu_ctx_elr(val: usize) {
    unsafe {
        CPU.set_elr(val);
    }
}

pub fn cpu_stack() -> usize {
    unsafe { &(CPU.stack) as *const _ as usize }
}
// end set/get CPU

pub fn cpu_init() {
    let cpu_id = cpu_id();
    // println!("cpu id {}", cpu_id);
    if cpu_id == 0 {
        use crate::board::{platform_power_on_secondary_cores, power_arch_init};
        platform_power_on_secondary_cores();
        power_arch_init();
        cpu_if_init();
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

pub fn cpu_idle() {
    set_cpu_state(CpuState::CpuIdle);
    cpu_interrupt_unmask();
    loop {
        unsafe {
            llvm_asm!("wfi");
        }
    }
}

static CPU_LIST: Mutex<[Cpu; PLATFORM_CPU_NUM_MAX]> = Mutex::new([
    Cpu::default(),
    Cpu::default(),
    Cpu::default(),
    Cpu::default(),
    Cpu::default(),
    Cpu::default(),
    Cpu::default(),
    Cpu::default(),
]);

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
