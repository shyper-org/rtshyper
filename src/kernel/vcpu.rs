use alloc::sync::Arc;
use alloc::vec::Vec;
use core::mem::size_of;
use spin::Mutex;

use crate::arch::{
    ContextFrame, ContextFrameTrait, cpu_interrupt_unmask, GIC_INTS_MAX, GIC_SGI_REGS_NUM, GICC, GicContext, GICD,
    GICH, VmContext, timer_arch_get_counter, VM_IPA_SIZE, DEVICE_BASE,
};
use crate::board::{PlatOperation, Platform};
use crate::kernel::{current_cpu, interrupt_vm_inject, vm_if_set_state};
use crate::kernel::{active_vcpu_id, active_vm_id};
use crate::util::memcpy_safe;

use super::{CpuState, Vm, VmType, WeakVm};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VcpuState {
    Inv = 0,
    Pend = 1,
    Active = 2,
}

#[derive(Clone)]
pub struct Vcpu {
    pub inner: Arc<Mutex<VcpuInner>>,
}

impl Vcpu {
    pub fn new(vm: &Vm, vcpu_id: usize) -> Self {
        let this = Self {
            inner: Arc::new(Mutex::new(VcpuInner::new(vm.get_weak(), vcpu_id))),
        };
        crate::arch::vcpu_arch_init(vm, &this);
        this.reset_context();
        this
    }

    pub fn shutdown(&self) {
        println!(
            "Core {} (vm {} vcpu {}) shutdown ok",
            current_cpu().id,
            active_vm_id(),
            active_vcpu_id()
        );
        Platform::cpu_shutdown();
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
            size_of::<ContextFrame>(),
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
            size_of::<ContextFrame>(),
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
        self.vm().unwrap().update_vtimer();
        self.save_cpu_ctx();

        let mut inner = self.inner.lock();
        inner.vm_ctx.ext_regs_store();
        inner.vm_ctx.fpsimd_save_context();
        inner.vm_ctx.gic_save_state();
    }

    pub fn context_gic_irqs_store(&self) {
        let mut inner = self.inner.lock();
        let vm = inner.vm.get_vm().unwrap();
        for irq in vm.config().passthrough_device_irqs() {
            inner.gic_ctx.add_irq(irq as u64);
        }
        inner.gic_ctx.add_irq(25);
        let gicv_ctlr = unsafe { &*((Platform::GICV_BASE + DEVICE_BASE) as *const u32) };
        inner.gic_ctx.set_gicv_ctlr(*gicv_ctlr);
        let gicv_pmr = unsafe { &*((Platform::GICV_BASE + DEVICE_BASE + 0x4) as *const u32) };
        inner.gic_ctx.set_gicv_pmr(*gicv_pmr);
    }

    pub fn context_gic_irqs_restore(&self) {
        let inner = self.inner.lock();

        for irq_state in inner.gic_ctx.irq_state.iter() {
            if irq_state.id != 0 {
                GICD.set_enable(irq_state.id as usize, irq_state.enable != 0);
                GICD.set_prio(irq_state.id as usize, irq_state.priority);
                GICD.set_trgt(irq_state.id as usize, 1 << Platform::cpuid_to_cpuif(current_cpu().id));
            }
        }

        let gicv_pmr = unsafe { &mut *((Platform::GICV_BASE + DEVICE_BASE + 0x4) as *mut u32) };
        *gicv_pmr = inner.gic_ctx.gicv_pmr();
        // println!("Core[{}] save gic context", current_cpu().id);
        let gicv_ctlr = unsafe { &mut *((Platform::GICV_BASE + DEVICE_BASE) as *mut u32) };
        *gicv_ctlr = inner.gic_ctx.gicv_ctlr();
        // show_vcpu_reg_context();
    }

    pub fn context_vm_restore(&self) {
        // println!("context_vm_restore");
        let _vtimer_offset = self.vm().unwrap().update_vtimer_offset();
        self.restore_cpu_ctx();

        let inner = self.inner.lock();
        // restore vm's VFP and SIMD
        inner.vm_ctx.fpsimd_restore_context();
        inner.vm_ctx.gic_restore_state();
        inner.vm_ctx.ext_regs_restore();
        drop(inner);

        self.inject_int_inlist();
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
                    size_of::<ContextFrame>(),
                );
            }
        }
    }

    fn restore_cpu_ctx(&self) {
        let inner = self.inner.lock();
        match current_cpu().ctx {
            None => {
                println!("restore_cpu_ctx: cpu{} ctx is NULL", current_cpu().id);
            }
            Some(ctx) => {
                memcpy_safe(
                    ctx as *const u8,
                    &(inner.vcpu_ctx) as *const _ as *const u8,
                    size_of::<ContextFrame>(),
                );
            }
        }
    }

    pub fn set_phys_id(&self, phys_id: usize) {
        let mut inner = self.inner.lock();
        println!("set vcpu {} phys id {}", inner.id, phys_id);
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
        inner.state
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
        inner.vm.get_vm()
    }

    pub fn phys_id(&self) -> usize {
        let inner = self.inner.lock();
        inner.phys_id
    }

    pub fn vm_id(&self) -> usize {
        self.vm().unwrap().id()
    }

    pub fn vm_pt_dir(&self) -> usize {
        self.vm().unwrap().pt_dir()
    }

    pub fn reset_context(&self) {
        let mut inner = self.inner.lock();
        inner.reset_context();
    }

    pub fn reset_vmpidr(&self) {
        let mut inner = self.inner.lock();
        inner.reset_vmpidr();
    }

    pub fn reset_vtimer_offset(&self) {
        let mut inner = self.inner.lock();
        inner.reset_vtimer_offset();
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
        if !inner.int_list.contains(&int) {
            inner.int_list.push(int);
        }
    }

    fn inject_int_inlist(&self) {
        match self.vm() {
            None => {}
            Some(vm) => {
                let mut inner = self.inner.lock();
                let int_list = inner.int_list.clone();
                inner.int_list.clear();
                drop(inner);
                for int in int_list {
                    // println!("schedule: inject int {} for vm {}", int, vm.id());
                    interrupt_vm_inject(&vm, self, int, 0);
                }
            }
        }
    }
}

pub struct VcpuInner {
    pub id: usize,
    pub phys_id: usize,
    pub state: VcpuState,
    pub vm: WeakVm,
    pub int_list: Vec<usize>,
    pub vcpu_ctx: ContextFrame,
    pub vm_ctx: VmContext,
    pub gic_ctx: GicContext,
}

impl VcpuInner {
    pub fn new(vm: WeakVm, id: usize) -> Self {
        Self {
            id,
            phys_id: 0,
            state: VcpuState::Inv,
            vm,
            int_list: vec![],
            vcpu_ctx: ContextFrame::default(),
            vm_ctx: VmContext::default(),
            gic_ctx: GicContext::default(),
        }
    }

    fn vcpu_ctx_addr(&self) -> usize {
        &(self.vcpu_ctx) as *const _ as usize
    }

    fn vm_id(&self) -> usize {
        self.vm.get_vm().unwrap().id()
    }

    fn arch_ctx_reset(&mut self) {
        // let migrate = self.vm.as_ref().unwrap().migration_state();
        // if !migrate {
        self.vm_ctx.cntvoff_el2 = 0;
        self.vm_ctx.sctlr_el1 = 0x30C50830;
        self.vm_ctx.cntkctl_el1 = 0;
        self.vm_ctx.pmcr_el0 = 0;
        self.vm_ctx.vtcr_el2 = 0x80013540 + ((64 - VM_IPA_SIZE) & ((1 << 6) - 1));
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

    fn reset_vtimer_offset(&mut self) {
        let curpct = timer_arch_get_counter() as u64;
        self.vm_ctx.cntvoff_el2 = curpct - self.vm_ctx.cntvct_el0;
    }

    fn reset_context(&mut self) {
        // let migrate = self.vm.as_ref().unwrap().migration_state();
        self.arch_ctx_reset();
        // if !migrate {
        self.gic_ctx_reset();
        // }
        use crate::kernel::vm_if_get_type;
        if vm_if_get_type(self.vm_id()) == VmType::VmTBma {
            println!("vm {} bma ctx restore", self.vm_id());
            self.reset_vm_ctx();
            self.context_ext_regs_store();
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

pub fn vcpu_idle(_vcpu: Vcpu) -> ! {
    cpu_interrupt_unmask();
    loop {
        use crate::arch::ArchTrait;
        crate::arch::Arch::wait_for_interrupt();
    }
}

// WARNING: No Auto `drop` in this function
pub fn vcpu_run(announce: bool) {
    {
        let vcpu = current_cpu().active_vcpu.clone().unwrap();
        let vm = vcpu.vm().unwrap();

        vm_if_set_state(active_vm_id(), super::VmState::Active);

        if announce {
            crate::device::virtio_net_announce(vm);
        }
        // if the cpu is already running (a vcpu in scheduling queue),
        // just return, no `context_vm_entry` required
        if current_cpu().cpu_state == CpuState::Run {
            return;
        }
        current_cpu().cpu_state = CpuState::Run;
        // tlb_invalidate_guest_all();
        // for i in 0..vm.mem_region_num() {
        //     unsafe {
        //         cache_invalidate_d(vm.pa_start(i), vm.pa_length(i));
        //     }
        // }
    }
    extern "C" {
        fn context_vm_entry(ctx: usize) -> !;
    }
    unsafe {
        context_vm_entry(current_cpu().ctx.unwrap());
    }
}

pub fn show_vcpu_reg_context() {
    print!("#### GICD ISENABLER ####");
    for i in 0..GIC_INTS_MAX / 32 {
        if i % 8 == 0 {
            println!();
        }
        print!("{:x} ", GICD.is_enabler(i));
    }
    println!();
    print!("#### GICD ISACTIVER ####");
    for i in 0..GIC_INTS_MAX / 32 {
        if i % 8 == 0 {
            println!();
        }
        print!("{:x} ", GICD.is_activer(i));
    }
    println!();
    print!("#### GICD ISPENDER ####");
    for i in 0..GIC_INTS_MAX / 32 {
        if i % 8 == 0 {
            println!();
        }
        print!("{:x} ", GICD.is_pender(i));
    }
    println!();
    print!("#### GICD IGROUP ####");
    for i in 0..GIC_INTS_MAX / 32 {
        if i % 8 == 0 {
            println!();
        }
        print!("{:x} ", GICD.igroup(i));
    }
    println!();
    print!("#### GICD ICFGR ####");
    for i in 0..GIC_INTS_MAX * 2 / 32 {
        if i % 8 == 0 {
            println!();
        }
        print!("{:x} ", GICD.icfgr(i));
    }
    println!();
    print!("#### GICD CPENDSGIR ####");
    for i in 0..GIC_SGI_REGS_NUM {
        if i % 8 == 0 {
            println!();
        }
        print!("{:x} ", GICD.cpendsgir(i));
    }
    println!();
    println!("GICH_APR {:x}", GICH.misr());

    println!("GICD_CTLR {:x}", GICD.ctlr());
    print!("#### GICD ITARGETSR ####");
    for i in 0..GIC_INTS_MAX * 8 / 32 {
        if i % 8 == 0 {
            println!();
        }
        print!("{:x} ", GICD.itargetsr(i));
    }
    println!();

    print!("#### GICD IPRIORITYR ####");
    for i in 0..GIC_INTS_MAX * 8 / 32 {
        if i % 16 == 0 {
            println!();
        }
        print!("{:x} ", GICD.ipriorityr(i));
    }
    println!();

    println!("GICC_RPR {:x}", GICC.rpr());
    println!("GICC_HPPIR {:x}", GICC.hppir());
    println!("GICC_BPR {:x}", GICC.bpr());
    println!("GICC_ABPR {:x}", GICC.abpr());
    println!("#### GICC APR ####");
    for i in 0..4 {
        print!("{:x} ", GICC.apr(i));
    }
    println!();
    println!("#### GICC NSAPR ####");
    for i in 0..4 {
        print!("{:x} ", GICC.nsapr(i));
    }

    println!("GICH_MISR {:x}", GICH.misr());
    println!("GICV_CTLR {:x}", unsafe {
        *((Platform::GICV_BASE + DEVICE_BASE) as *const u32)
    });
    println!("GICV_PMR {:x}", unsafe {
        *((Platform::GICV_BASE + DEVICE_BASE + 0x4) as *const u32)
    });
    println!("GICV_BPR {:x}", unsafe {
        *((Platform::GICV_BASE + DEVICE_BASE + 0x8) as *const u32)
    });
    println!("GICV_ABPR {:x}", unsafe {
        *((Platform::GICV_BASE + DEVICE_BASE + 0x1c) as *const u32)
    });
    println!("GICV_STATUSR {:x}", unsafe {
        *((Platform::GICV_BASE + DEVICE_BASE + 0x2c) as *const u32)
    });
    println!(
        "GICV_APR[0] {:x}, GICV_APR[1] {:x}, GICV_APR[2] {:x}, GICV_APR[3] {:x}",
        unsafe { *((Platform::GICV_BASE + DEVICE_BASE + 0xd0) as *const u32) },
        unsafe { *((Platform::GICV_BASE + DEVICE_BASE + 0xd4) as *const u32) },
        unsafe { *((Platform::GICV_BASE + DEVICE_BASE + 0xd8) as *const u32) },
        unsafe { *((Platform::GICV_BASE + DEVICE_BASE + 0xdc) as *const u32) },
    );
}
