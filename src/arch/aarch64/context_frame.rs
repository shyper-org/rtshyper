use core::fmt::Formatter;

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

impl crate::arch::traits::ContextFrameTrait for Aarch64ContextFrame {
    fn new(pc: usize, sp: usize, arg: usize, privileged: bool) -> Self {
        use cortex_a::regs::*;
        let mut r = Aarch64ContextFrame {
            gpr: [0; 31],
            spsr: (if privileged {
                SPSR_EL1::M::EL1t
            } else {
                SPSR_EL1::M::EL0t
            } + SPSR_EL1::I::Unmasked
                + SPSR_EL1::F::Masked)
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

    fn gpr(&self, index: usize) -> usize {
        self.gpr[index] as usize
    }
}
