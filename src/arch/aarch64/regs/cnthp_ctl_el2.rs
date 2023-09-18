use tock_registers::{
    interfaces::{Readable, Writeable},
    register_bitfields,
};

register_bitfields! {u64,
    pub CNTHP_CTL_EL2 [
        /// The status of the timer. This bit indicates whether the timer condition is met:
        ///
        /// 0 Timer condition is not met.
        /// 1 Timer condition is met.
        ///
        /// When the value of the ENABLE bit is 1, ISTATUS indicates whether the timer condition is
        /// met. ISTATUS takes no account of the value of the IMASK bit. If the value of ISTATUS is
        /// 1 and the value of IMASK is 0 then the timer interrupt is asserted.
        ///
        /// When the value of the ENABLE bit is 0, the ISTATUS field is UNKNOWN.
        ///
        /// This bit is read-only.
        ISTATUS OFFSET(2) NUMBITS(1) [],

        /// Timer interrupt mask bit. Permitted values are:
        ///
        /// 0 Timer interrupt is not masked by the IMASK bit.
        /// 1 Timer interrupt is masked by the IMASK bit.
        IMASK   OFFSET(1) NUMBITS(1) [],

        /// Enables the timer. Permitted values are:
        ///
        /// 0 Timer disabled.
        /// 1 Timer enabled.
        ENABLE  OFFSET(0) NUMBITS(1) []
    ]
}

pub struct Reg;

impl Readable for Reg {
    type T = u64;
    type R = CNTHP_CTL_EL2::Register;

    #[inline]
    fn get(&self) -> Self::T {
        let reg;
        mrs!(reg, CNTHP_CTL_EL2, "x");
        reg
    }
}

impl Writeable for Reg {
    type T = u64;
    type R = CNTHP_CTL_EL2::Register;

    #[inline]
    fn set(&self, value: Self::T) {
        msr!(CNTHP_CTL_EL2, value, "x");
    }
}

pub const CNTHP_CTL_EL2: Reg = Reg {};
