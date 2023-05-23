// Move to ARM register from system coprocessor register.
// MRS Xd, sysreg "Xd = sysreg"
macro_rules! mrs {
    ($reg: expr) => {
        {
            let r: u64;
            unsafe {
                core::arch::asm!(concat!("mrs {0}, ", stringify!($reg)), out(reg) r, options(nomem, nostack));
            }
            r
        }
    };
    ($val: expr, $reg: expr, $asm_width:tt) => {
        unsafe {
            core::arch::asm!(concat!("mrs {0:", $asm_width, "}, ", stringify!($reg)), out(reg) $val, options(nomem, nostack));
        }
    };
    ($val: expr, $reg: expr) => {
        unsafe {
            core::arch::asm!(concat!("mrs {0}, ", stringify!($reg)), out(reg) $val, options(nomem, nostack));
        }
    };
}

// Move to system coprocessor register from ARM register.
// MSR sysreg, Xn "sysreg = Xn"
macro_rules! msr {
    ($reg: expr, $val: expr, $asm_width:tt) => {
        unsafe {
            core::arch::asm!(concat!("msr ", stringify!($reg), ", {0:", $asm_width, "}"), in(reg) $val, options(nomem, nostack));
        }
    };
    ($reg: expr, $val: expr) => {
        unsafe {
            core::arch::asm!(concat!("msr ", stringify!($reg), ", {0}"), in(reg) $val, options(nomem, nostack));
        }
    };
}

macro_rules! sysreg_encode_addr {
    ($op0:expr, $op1:expr, $crn:expr, $crm:expr, $op2:expr) => {
        // (Op0[21..20] + Op2[19..17] + Op1[16..14] + CRn[13..10]) + CRm[4..1]
        ((($op0 & 0b11) << 20)
            | (($op2 & 0b111) << 17)
            | (($op1 & 0b111) << 14)
            | (($crn & 0xf) << 10)
            | (($crm & 0xf) << 1))
    };
}

macro_rules! arm_at {
    ($at_op:expr, $addr:expr) => {
        unsafe {
            core::arch::asm!(concat!("AT ", $at_op, ", {0}"), in(reg) $addr, options(nomem, nostack));
            core::arch::asm!("isb");
        }
    };
}
