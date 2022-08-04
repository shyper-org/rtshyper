use alloc::sync::Arc;
use alloc::vec::Vec;
use core::arch::asm;
use core::mem::size_of;

use spin::Mutex;

use crate::arch::{Aarch64ContextFrame, ContextFrameTrait, cpu_interrupt_unmask, GicContext, GICD, VmContext};
use crate::arch::tlb_invalidate_guest_all;
use crate::board::{platform_cpuid_to_cpuif, PLATFORM_GICV_BASE, PLATFORM_VCPU_NUM_MAX};
use crate::kernel::{current_cpu, interrupt_vm_inject, timer_enable, vm_if_set_state};
use crate::kernel::{active_vcpu_id, active_vm_id, CPU_STACK_SIZE};
use crate::lib::{cache_invalidate_d, memcpy_safe};

use super::{CpuState, Vm, VmType};

#[derive(Clone, Copy, Debug)]
pub enum VcpuState {
    VcpuInv = 0,
    VcpuPend = 1,
    VcpuAct = 2,
}

#[derive(Clone)]
pub struct Vcpu {
    pub inner: Arc<Mutex<VcpuInner>>,
}

impl Vcpu {
    pub fn default() -> Vcpu {
        Vcpu {
            inner: Arc::new(Mutex::new(VcpuInner::default())),
        }
    }

    pub fn init(&self, vm: Vm, vcpu_id: usize) {
        let mut inner = self.inner.lock();
        inner.vm = Some(vm.clone());
        inner.id = vcpu_id;
        inner.phys_id = 0;
        drop(inner);
        crate::arch::vcpu_arch_init(vm.clone(), self.clone());
    }

    pub fn shutdown(&self) {
        println!(
            "Core {} (vm {} vcpu {}) shutdown ok",
            current_cpu().id,
            active_vm_id(),
            active_vcpu_id()
        );
        crate::board::platform_cpu_shutdown();
    }

    pub fn migrate_vm_ctx_save(&self, cache_pa: usize) {
        let inner = self.inner.lock();
        memcpy_safe(
            cache_pa as *const u8,
            &(inner.vm_ctx) as *const _ as *const u8,
            size_of::<VmContext>(),
        );
    }

    pub fn migrate_vcpu_ctx_save(&self, cache_pa: usize) {
        let inner = self.inner.lock();
        memcpy_safe(
            cache_pa as *const u8,
            &(inner.vcpu_ctx) as *const _ as *const u8,
            size_of::<Aarch64ContextFrame>(),
        );
    }

    pub fn migrate_gic_ctx_save(&self, cache_pa: usize) {
        let inner = self.inner.lock();
        memcpy_safe(
            cache_pa as *const u8,
            &(inner.gic_ctx) as *const _ as *const u8,
            size_of::<GicContext>(),
        );
    }

    pub fn migrate_vm_ctx_restore(&self, cache_pa: usize) {
        let inner = self.inner.lock();
        memcpy_safe(
            &(inner.vm_ctx) as *const _ as *const u8,
            cache_pa as *const u8,
            size_of::<VmContext>(),
        );
    }

    pub fn migrate_vcpu_ctx_restore(&self, cache_pa: usize) {
        let inner = self.inner.lock();
        memcpy_safe(
            &(inner.vcpu_ctx) as *const _ as *const u8,
            cache_pa as *const u8,
            size_of::<Aarch64ContextFrame>(),
        );
    }

    pub fn migrate_gic_ctx_restore(&self, cache_pa: usize) {
        let inner = self.inner.lock();
        memcpy_safe(
            &(inner.gic_ctx) as *const _ as *const u8,
            cache_pa as *const u8,
            size_of::<GicContext>(),
        );
    }

    pub fn context_vm_store(&self) {
        self.save_cpu_ctx();

        let mut inner = self.inner.lock();
        inner.vm_ctx.ext_regs_store();
        inner.vm_ctx.fpsimd_save_context();
        inner.vm_ctx.gic_save_state();
    }

    pub fn context_gic_store(&self) {
        let mut inner = self.inner.lock();
        let vm = inner.vm.clone().unwrap();
        inner.gic_ctx;
        for int_id in 0..16 {
            println!(
                "Core[{}] int {} ISENABLER {:x}, ISACTIVER {:x}, IPRIORITY {:x}, ITARGETSR {:x}, ICFGR {:x}",
                current_cpu().id,
                int_id,
                GICD.is_enabler(int_id / 32),
                GICD.is_activer(int_id / 32),
                GICD.ipriorityr(int_id / 4),
                GICD.itargetsr(int_id / 4),
                GICD.icfgr(int_id / 2)
            );
        }
        let int_id = 27;
        println!(
            "int {} ISENABLER {:x}, ISACTIVER {:x}, IPRIORITY {:x}, ITARGETSR {:x}, ICFGR {:x}",
            int_id,
            GICD.is_enabler(int_id / 32),
            GICD.is_activer(int_id / 32),
            GICD.ipriorityr(int_id / 4),
            GICD.itargetsr(int_id / 4),
            GICD.icfgr(int_id / 2)
        );
        for irq in vm.config().passthrough_device_irqs() {
            inner.gic_ctx.add_irq(irq as u64);
        }
        let gicv_ctlr = unsafe { &*((PLATFORM_GICV_BASE + 0x8_0000_0000) as *const u32) };
        inner.gic_ctx.set_gicv_ctlr(*gicv_ctlr);
        let gicv_pmr = unsafe { &*((PLATFORM_GICV_BASE + 0x8_0000_0000 + 0x4) as *const u32) };
        inner.gic_ctx.set_gicv_pmr(*gicv_pmr);
    }

    pub fn context_gic_restore(&self) {
        let inner = self.inner.lock();

        for irq_state in inner.gic_ctx.irq_state.iter() {
            if irq_state.id != 0 {
                println!("Core {} set irq {} GICD", current_cpu().id, irq_state.id);
                GICD.set_enable(irq_state.id as usize, irq_state.enable != 0);
                GICD.set_prio(irq_state.id as usize, irq_state.priority);
                GICD.set_trgt(irq_state.id as usize, 1 << platform_cpuid_to_cpuif(current_cpu().id));
                // let int_id = irq_state.id as usize;
                // println!(
                //     "Core[{}] context_gic_restore after: int {} ISENABLER {:x}, ISACTIVER {:x}, IPRIORITY {:x}, ITARGETSR {:x}, ICFGR {:x}",
                //     current_cpu().id,
                //     int_id,
                //     GICD.is_enabler(int_id / 32),
                //     GICD.is_activer(int_id / 32),
                //     GICD.ipriorityr(int_id / 4),
                //     GICD.itargetsr(int_id / 4),
                //     GICD.icfgr(int_id / 2)
                // );
            }
        }

        let gicv_pmr = unsafe { &mut *((PLATFORM_GICV_BASE + 0x8_0000_0000 + 0x4) as *mut u32) };
        *gicv_pmr = inner.gic_ctx.gicv_pmr();
        println!("Core[{}] save gic context", current_cpu().id);
        let gicv_ctlr = unsafe { &mut *((PLATFORM_GICV_BASE + 0x8_0000_0000) as *mut u32) };
        *gicv_ctlr = inner.gic_ctx.gicv_ctlr();
    }

    pub fn context_vm_restore(&self) {
        // println!("context_vm_restore");
        self.restore_cpu_ctx();

        let inner = self.inner.lock();
        inner.vm_ctx.ext_regs_restore();

        // restore vm's VFP and SIMD
        inner.vm_ctx.fpsimd_restore_context();
        inner.vm_ctx.gic_restore_state();
        drop(inner);

        // restore vm's Stage2 MMU context
        let vttbr = (self.vm_id() << 48) | self.vm_pt_dir();
        println!("vttbr {:x}", vttbr);
        unsafe {
            asm!("msr VTTBR_EL2, {0}", "isb", in(reg) vttbr);
        }
    }

    pub fn gic_restore_context(&self) {
        let inner = self.inner.lock();
        inner.vm_ctx.gic_restore_state();
    }

    pub fn gic_save_context(&self) {
        let mut inner = self.inner.lock();
        inner.vm_ctx.gic_save_state();
    }

    pub fn save_cpu_ctx(&self) {
        let inner = self.inner.lock();
        match current_cpu().ctx {
            None => {
                println!("save_cpu_ctx: cpu{} ctx is NULL", current_cpu().id);
            }
            Some(ctx) => {
                memcpy_safe(
                    &(inner.vcpu_ctx) as *const _ as *const u8,
                    ctx as *const u8,
                    size_of::<Aarch64ContextFrame>(),
                );
            }
        }
    }

    pub fn restore_cpu_ctx(&self) {
        let inner = self.inner.lock();
        match current_cpu().ctx {
            None => {
                println!("restore_cpu_ctx: cpu{} ctx is NULL", current_cpu().id);
            }
            Some(ctx) => {
                memcpy_safe(
                    ctx as *const u8,
                    &(inner.vcpu_ctx) as *const _ as *const u8,
                    size_of::<Aarch64ContextFrame>(),
                );
            }
        }
    }

    pub fn set_phys_id(&self, phys_id: usize) {
        let mut inner = self.inner.lock();
        inner.phys_id = phys_id;
    }

    pub fn set_gich_ctlr(&self, ctlr: u32) {
        let mut inner = self.inner.lock();
        inner.vm_ctx.gic_state.ctlr = ctlr;
    }

    pub fn set_hcr(&self, hcr: u64) {
        let mut inner = self.inner.lock();
        inner.vm_ctx.hcr_el2 = hcr;
    }

    pub fn state(&self) -> VcpuState {
        let inner = self.inner.lock();
        inner.state.clone()
    }

    pub fn set_state(&self, state: VcpuState) {
        let mut inner = self.inner.lock();
        inner.state = state;
    }

    pub fn id(&self) -> usize {
        let inner = self.inner.lock();
        inner.id
    }

    pub fn vm(&self) -> Option<Vm> {
        let inner = self.inner.lock();
        // inner.vm.clone()
        match &inner.vm {
            None => None,
            Some(vm) => Some(vm.clone()),
        }
        // if inner.vm.is_none() {
        //     None
        // } else {
        //     inner.vm.clone()
        //     // Some(inner.vm.as_ref().unwrap().clone())
        // }
    }

    pub fn phys_id(&self) -> usize {
        let inner = self.inner.lock();
        inner.phys_id
    }

    pub fn vm_id(&self) -> usize {
        let inner = self.inner.lock();
        let vm = inner.vm.clone().unwrap();
        drop(inner);
        vm.id()
    }

    pub fn vm_pt_dir(&self) -> usize {
        let inner = self.inner.lock();
        let vm = inner.vm.clone().unwrap();
        drop(inner);
        vm.pt_dir()
    }

    pub fn arch_reset(&self) {
        let mut inner = self.inner.lock();
        inner.arch_ctx_reset();
    }

    pub fn reset_context(&self) {
        let mut inner = self.inner.lock();
        inner.reset_context();
    }

    pub fn reset_vmpidr(&self) {
        let mut inner = self.inner.lock();
        inner.reset_vmpidr();
    }

    pub fn context_ext_regs_store(&self) {
        let mut inner = self.inner.lock();
        inner.context_ext_regs_store();
    }

    pub fn vcpu_ctx_addr(&self) -> usize {
        let inner = self.inner.lock();
        inner.vcpu_ctx_addr()
    }

    pub fn set_elr(&self, elr: usize) {
        let mut inner = self.inner.lock();
        inner.set_elr(elr);
    }

    pub fn elr(&self) -> usize {
        let inner = self.inner.lock();
        inner.vcpu_ctx.exception_pc()
    }

    pub fn set_gpr(&self, idx: usize, val: usize) {
        let mut inner = self.inner.lock();
        inner.set_gpr(idx, val);
    }

    pub fn show_ctx(&self) {
        let inner = self.inner.lock();
        inner.show_ctx();
    }

    pub fn push_int(&self, int: usize) {
        let mut inner = self.inner.lock();
        for i in &inner.int_list {
            if *i == int {
                return;
            }
        }
        inner.int_list.push(int);
    }

    // TODO: ugly lock for self.inner
    pub fn inject_int_inlist(&self) {
        let inner = self.inner.lock();
        match inner.vm.clone() {
            None => {}
            Some(vm) => {
                let int_list = inner.int_list.clone();
                drop(inner);
                for int in int_list {
                    // println!("schedule: inject int {} for vm {}", int, vm.id());
                    interrupt_vm_inject(vm.clone(), self.clone(), int, 0);
                }
                let mut inner = self.inner.lock();
                inner.int_list.clear();
            }
        }
    }
}

pub struct VcpuInner {
    pub id: usize,
    pub phys_id: usize,
    pub state: VcpuState,
    pub vm: Option<Vm>,
    pub int_list: Vec<usize>,
    pub vcpu_ctx: Aarch64ContextFrame,
    pub vm_ctx: VmContext,
    pub gic_ctx: GicContext,
}

impl VcpuInner {
    pub fn default() -> VcpuInner {
        VcpuInner {
            id: 0,
            phys_id: 0,
            state: VcpuState::VcpuInv,
            vm: None,
            int_list: vec![],
            vcpu_ctx: Aarch64ContextFrame::default(),
            vm_ctx: VmContext::default(),
            gic_ctx: GicContext::default(),
        }
    }

    fn vcpu_ctx_addr(&self) -> usize {
        &(self.vcpu_ctx) as *const _ as usize
    }

    fn vm_id(&self) -> usize {
        let vm = self.vm.as_ref().unwrap();
        vm.id()
    }

    fn arch_ctx_reset(&mut self) {
        // let migrate = self.vm.as_ref().unwrap().migration_state();
        // if !migrate {
        self.vm_ctx.cntvoff_el2 = 0;
        self.vm_ctx.sctlr_el1 = 0x30C50830;
        self.vm_ctx.cntkctl_el1 = 0;
        self.vm_ctx.pmcr_el0 = 0;
        self.vm_ctx.vtcr_el2 = 0x8001355c;
        // }
        let mut vmpidr = 0;
        vmpidr |= 1 << 31;

        #[cfg(feature = "tx2")]
        if self.vm_id() == 0 {
            // A57 is cluster #1 for L4T
            vmpidr |= 0x100;
        }

        vmpidr |= self.id;
        self.vm_ctx.vmpidr_el2 = vmpidr as u64;
    }

    fn reset_vmpidr(&mut self) {
        let mut vmpidr = 0;
        vmpidr |= 1 << 31;

        #[cfg(feature = "tx2")]
        if self.vm_id() == 0 {
            // A57 is cluster #1 for L4T
            vmpidr |= 0x100;
        }

        vmpidr |= self.id;
        self.vm_ctx.vmpidr_el2 = vmpidr as u64;
    }

    fn reset_context(&mut self) {
        // let migrate = self.vm.as_ref().unwrap().migration_state();
        self.arch_ctx_reset();
        // if !migrate {
        self.gic_ctx_reset();
        // }
        use crate::kernel::vm_if_get_type;
        match vm_if_get_type(self.vm_id()) {
            VmType::VmTBma => {
                println!("vm {} bma ctx restore", self.vm_id());
                self.reset_vm_ctx();
                self.context_ext_regs_store();
            }
            _ => {}
        }
    }

    fn gic_ctx_reset(&mut self) {
        use crate::arch::gich_lrs_num;
        for i in 0..gich_lrs_num() {
            self.vm_ctx.gic_state.lr[i] = 0;
        }
        self.vm_ctx.gic_state.hcr |= 1 << 2;
    }

    fn context_ext_regs_store(&mut self) {
        self.vm_ctx.ext_regs_store();
    }

    fn reset_vm_ctx(&mut self) {
        self.vm_ctx.reset();
    }

    fn set_elr(&mut self, elr: usize) {
        self.vcpu_ctx.set_exception_pc(elr);
    }

    fn set_gpr(&mut self, idx: usize, val: usize) {
        self.vcpu_ctx.set_gpr(idx, val);
    }

    fn show_ctx(&self) {
        println!(
            "cntvoff_el2 {:x}, sctlr_el1 {:x}, cntkctl_el1 {:x}, pmcr_el0 {:x}, vtcr_el2 {:x} x0 {:x}",
            self.vm_ctx.cntvoff_el2,
            self.vm_ctx.sctlr_el1,
            self.vm_ctx.cntkctl_el1,
            self.vm_ctx.pmcr_el0,
            self.vm_ctx.vtcr_el2,
            self.vcpu_ctx.gpr(0)
        );
    }
}

pub static VCPU_LIST: Mutex<Vec<Vcpu>> = Mutex::new(Vec::new());

pub fn vcpu_alloc() -> Option<Vcpu> {
    let mut vcpu_list = VCPU_LIST.lock();
    if vcpu_list.len() >= PLATFORM_VCPU_NUM_MAX {
        return None;
    }

    let val = Vcpu::default();
    vcpu_list.push(val.clone());
    Some(val.clone())
}

pub fn vcpu_remove(vcpu: Vcpu) {
    let mut vcpu_list = VCPU_LIST.lock();
    for (idx, core) in vcpu_list.iter().enumerate() {
        if core.id() == vcpu.id() && core.vm_id() == vcpu.vm_id() {
            vcpu_list.remove(idx);
            return;
        }
    }
    panic!("illegal vm{} vcpu{}, not exist in vcpu_list", vcpu.vm_id(), vcpu.id());
}

pub fn vcpu_idle(_vcpu: Vcpu) {
    cpu_interrupt_unmask();
    loop {
        unsafe {
            asm!("wfi");
        }
    }
}

pub fn vcpu_run() {
    println!(
        "Core {} (vm {}, vcpu {}) start running",
        current_cpu().id,
        active_vm_id(),
        active_vcpu_id()
    );

    if current_cpu().vcpu_pool().running() > 1 {
        timer_enable(true);
    }

    // let vm = crate::kernel::active_vm().unwrap();
    // vm.show_pagetable(0x17000000);
    let vcpu = current_cpu().active_vcpu.clone().unwrap();
    vcpu.set_state(VcpuState::VcpuAct);
    let vm = vcpu.vm().unwrap().clone();
    let sp = &(current_cpu().stack) as *const _ as usize + CPU_STACK_SIZE;
    let size = size_of::<Aarch64ContextFrame>();
    current_cpu().set_ctx((sp - size) as *mut _);

    vcpu.context_vm_restore();
    tlb_invalidate_guest_all();
    vcpu.show_ctx();

    current_cpu().cpu_state = CpuState::CpuRun;
    vm_if_set_state(active_vm_id(), super::VmState::VmActive);

    for i in 0..vm.mem_region_num() {
        unsafe {
            cache_invalidate_d(vm.pa_start(i), vm.pa_length(i));
        }
    }

    println!(
        "vcpu run elr {:x} x0 {:016x} sp 0x{:x}",
        current_cpu().active_vcpu.clone().unwrap().elr(),
        current_cpu().get_gpr(0),
        sp
    );
    // TODO: vcpu_run
    extern "C" {
        fn context_vm_entry(ctx: usize) -> !;
    }
    unsafe {
        context_vm_entry(sp - size);
    }
}
