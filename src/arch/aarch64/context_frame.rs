use core::fmt::Formatter;
use cortex_a::regs::*;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct Aarch64ContextFrame {
    gpr: [u64; 31],
    spsr: u64,
    elr: u64,
    sp: u64,
}

impl core::fmt::Display for Aarch64ContextFrame {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), core::fmt::Error> {
        for i in 0..31 {
            write!(f, "x{:02}: {:016x}   ", i, self.gpr[i])?;
            if (i + 1) % 2 == 0 {
                write!(f, "\n")?;
            }
        }
        writeln!(f, "spsr:{:016x}", self.spsr)?;
        write!(f, "elr: {:016x}", self.elr)?;
        writeln!(f, "   sp:  {:016x}", self.sp)?;
        Ok(())
    }
}

impl crate::arch::ContextFrameTrait for Aarch64ContextFrame {
    fn new(pc: usize, sp: usize, arg: usize) -> Self {
        let mut r = Aarch64ContextFrame {
            gpr: [0; 31],
            spsr: (SPSR_EL1::M::EL1h
                + SPSR_EL1::I::Masked
                + SPSR_EL1::F::Masked
                + SPSR_EL1::A::Masked
                + SPSR_EL1::D::Masked)
                .value as u64,
            elr: pc as u64,
            sp: sp as u64,
        };
        r.set_argument(arg);
        r
    }

    fn exception_pc(&self) -> usize {
        self.elr as usize
    }

    fn set_exception_pc(&mut self, pc: usize) {
        self.elr = pc as u64;
    }

    fn stack_pointer(&self) -> usize {
        self.sp as usize
    }

    fn set_stack_pointer(&mut self, sp: usize) {
        self.sp = sp as u64;
    }

    fn set_argument(&mut self, arg: usize) {
        self.gpr[0] = arg as u64;
    }

    fn set_gpr(&mut self, index: usize, val: usize) {
        self.gpr[index] = val as u64;
    }

    fn gpr(&self, index: usize) -> usize {
        self.gpr[index] as usize
    }
}

impl Aarch64ContextFrame {
    pub fn default() -> Aarch64ContextFrame {
        Aarch64ContextFrame {
            gpr: [0; 31],
            spsr: (SPSR_EL1::M::EL1h
                + SPSR_EL1::I::Masked
                + SPSR_EL1::F::Masked
                + SPSR_EL1::A::Masked
                + SPSR_EL1::D::Masked)
                .value as u64,
            elr: 0,
            sp: 0,
        }
    }

    pub fn elr(&self) -> usize {
        self.elr as usize
    }
}

#[repr(C)]
#[repr(align(16))]
#[derive(Copy, Clone, Debug)]
pub struct VmCtxFpsimd {
    fpsimd: [u64; 64],
    fpsr: u32,
    fpcr: u32,
}

impl VmCtxFpsimd {
    pub fn default() -> VmCtxFpsimd {
        VmCtxFpsimd {
            fpsimd: [0; 64],
            fpsr: 0,
            fpcr: 0,
        }
    }
}

#[repr(C)]
#[repr(align(16))]
#[derive(Copy, Clone, Debug)]
pub struct VmContext {
    // generic timer
    cntvoff_el2: u64,
    cntp_cval_el0: u64,
    cntv_cval_el0: u64,
    cntkctl_el1: u32,
    cntp_ctl_el0: u32,

    // vpidr and vmpidr
    vpidr_el2: u32,
    vmpidr_el2: u64,

    // 64bit EL1/EL0 register
    sp_el0: u64,
    sp_el1: u64,
    elr_el1: u64,
    spsr_el1: u32,
    sctlr_el1: u32,
    actlr_el1: u64,
    cpacr_el1: u32,
    ttbr0_el1: u64,
    ttbr1_el1: u64,
    tcr_el1: u64,
    esr_el1: u32,
    far_el1: u64,
    par_el1: u64,
    mair_el1: u64,
    amair_el1: u64,
    vbar_el1: u64,
    contextidr_el1: u32,
    tpidr_el0: u64,
    tpidr_el1: u64,
    tpidrro_el0: u64,

    // hypervisor context
    hcr_el2: u64,
    cptr_el2: u64,
    hstr_el2: u64,

    // exception
    far_el2: u64,
    hpfar_el2: u64,
    fpsimd: VmCtxFpsimd,
}

impl VmContext {
    pub fn default() -> VmContext {
        VmContext {
            // generic timer
            cntvoff_el2: 0,
            cntp_cval_el0: 0,
            cntv_cval_el0: 0,
            cntkctl_el1: 0,
            cntp_ctl_el0: 0,

            // vpidr and vmpidr
            vpidr_el2: 0,
            vmpidr_el2: 0,

            // 64bit EL1/EL0 register
            sp_el0: 0,
            sp_el1: 0,
            elr_el1: 0,
            spsr_el1: 0,
            sctlr_el1: 0,
            actlr_el1: 0,
            cpacr_el1: 0,
            ttbr0_el1: 0,
            ttbr1_el1: 0,
            tcr_el1: 0,
            esr_el1: 0,
            far_el1: 0,
            par_el1: 0,
            mair_el1: 0,
            amair_el1: 0,
            vbar_el1: 0,
            contextidr_el1: 0,
            tpidr_el0: 0,
            tpidr_el1: 0,
            tpidrro_el0: 0,

            // hypervisor context
            hcr_el2: 0,
            cptr_el2: 0,
            hstr_el2: 0,

            // exception
            far_el2: 0,
            hpfar_el2: 0,
            fpsimd: VmCtxFpsimd::default(),
        }
    }

    pub fn ext_regs_store(&mut self) {
        unsafe {
            llvm_asm!("mrs $0, CNTVOFF_EL2" : "=r"(self.cntvoff_el2) ::: "volatile");
            llvm_asm!("mrs $0, CNTP_CVAL_EL0" : "=r"(self.cntp_cval_el0) ::: "volatile");
            llvm_asm!("mrs $0, CNTV_CVAL_EL0" : "=r"(self.cntv_cval_el0) ::: "volatile");
            llvm_asm!("mrs $0, CNTKCTL_EL1" : "=r"(self.cntkctl_el1) ::: "volatile");
            llvm_asm!("mrs $0, CNTP_CTL_EL0" : "=r"(self.cntp_ctl_el0) ::: "volatile");
            llvm_asm!("mrs $0, SP_EL0" : "=r"(self.sp_el0) ::: "volatile");
            llvm_asm!("mrs $0, SP_EL1" : "=r"(self.sp_el1) ::: "volatile");
            llvm_asm!("mrs $0, ELR_EL1" : "=r"(self.elr_el1) ::: "volatile");
            llvm_asm!("mrs $0, SPSR_EL1" : "=r"(self.spsr_el1) ::: "volatile");
            llvm_asm!("mrs $0, SCTLR_EL1" : "=r"(self.sctlr_el1) ::: "volatile");
            llvm_asm!("mrs $0, CPACR_EL1" : "=r"(self.cpacr_el1) ::: "volatile");
            llvm_asm!("mrs $0, TTBR0_EL1" : "=r"(self.ttbr0_el1) ::: "volatile");
            llvm_asm!("mrs $0, TTBR1_EL1" : "=r"(self.ttbr1_el1) ::: "volatile");
            llvm_asm!("mrs $0, TCR_EL1" : "=r"(self.tcr_el1) ::: "volatile");
            llvm_asm!("mrs $0, ESR_EL1" : "=r"(self.esr_el1) ::: "volatile");
            llvm_asm!("mrs $0, FAR_EL1" : "=r"(self.far_el1) ::: "volatile");
            llvm_asm!("mrs $0, PAR_EL1" : "=r"(self.par_el1) ::: "volatile");
            llvm_asm!("mrs $0, MAIR_EL1" : "=r"(self.mair_el1) ::: "volatile");
            llvm_asm!("mrs $0, AMAIR_EL1" : "=r"(self.amair_el1) ::: "volatile");
            llvm_asm!("mrs $0, VBAR_EL1" : "=r"(self.vbar_el1) ::: "volatile");
            llvm_asm!("mrs $0, CONTEXTIDR_EL1" : "=r"(self.contextidr_el1) ::: "volatile");
            llvm_asm!("mrs $0, TPIDR_EL0" : "=r"(self.tpidr_el0) ::: "volatile");
            llvm_asm!("mrs $0, TPIDR_EL1" : "=r"(self.tpidr_el1) ::: "volatile");
            llvm_asm!("mrs $0, TPIDRRO_EL0" : "=r"(self.tpidrro_el0) ::: "volatile");
            llvm_asm!("mrs $0, HCR_EL2" : "=r"(self.hcr_el2) ::: "volatile");
            llvm_asm!("mrs $0, CPTR_EL2" : "=r"(self.cptr_el2) ::: "volatile");
            llvm_asm!("mrs $0, HSTR_EL2" : "=r"(self.hstr_el2) ::: "volatile");
            llvm_asm!("mrs $0, FAR_EL2" : "=r"(self.far_el2) ::: "volatile");
            llvm_asm!("mrs $0, HPFAR_EL2" : "=r"(self.hpfar_el2) ::: "volatile");
            llvm_asm!("mrs $0, VPIDR_EL2" : "=r"(self.vpidr_el2) ::: "volatile");
            llvm_asm!("mrs $0, VMPIDR_EL2" : "=r"(self.vmpidr_el2) ::: "volatile");
            llvm_asm!("mrs $0, ACTLR_EL1" : "=r"(self.actlr_el1) ::: "volatile");
        }
    }
}
