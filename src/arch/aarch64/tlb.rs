use core::arch::asm;

use crate::arch::TlbInvalidate;
use crate::arch::PAGE_SHIFT;

use super::Aarch64Arch;

impl TlbInvalidate for Aarch64Arch {
    fn invalid_hypervisor_va(va: usize) {
        unsafe {
            asm!(
                "dsb ish",
                "tlbi vae2is, {0}",
                "dsb ish",
                "isb",
                in(reg) va >> PAGE_SHIFT,
                options(nostack)
            );
        }
    }

    #[inline]
    fn invalid_hypervisor_all() {
        unsafe {
            asm!("dsb ish", "tlbi alle2is", "dsb ish", "isb", options(nostack));
        }
    }

    fn invalid_guest_ipa(ipa: usize) {
        unsafe {
            asm!(
                "dsb ish",
                "tlbi ipas2e1is, {0}",
                "dsb ish",
                "isb",
                in(reg) ipa >> PAGE_SHIFT,
                options(nostack)
            );
        }
    }

    #[inline]
    fn invalid_guest_all() {
        unsafe {
            asm!("dsb ish", "tlbi vmalls12e1is", "dsb ish", "isb", options(nostack));
        }
    }
}
