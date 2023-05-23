use tock_registers::interfaces::*;

use crate::arch::{ArchTrait, Address};

pub const PAGE_SHIFT: usize = 12;
pub const PAGE_SIZE: usize = 1 << PAGE_SHIFT;
pub const ENTRY_PER_PAGE: usize = PAGE_SIZE / 8;

pub type ContextFrame = super::context_frame::Aarch64ContextFrame;

pub const WORD_SIZE: usize = core::mem::size_of::<usize>();
const_assert_eq!(WORD_SIZE, 8);
pub const PTE_PER_PAGE: usize = PAGE_SIZE / WORD_SIZE;

// The size offset of the memory region addressed by TTBR0_EL2
// see TCR_EL2::T0SZ
pub const HYP_VA_SIZE: u64 = 39;
// The size offset of the memory region addressed by VTTBR_EL2
// see VTCR_EL2::T0SZ
pub const VM_IPA_SIZE: u64 = 36;

pub type Arch = Aarch64Arch;

pub struct Aarch64Arch;

impl ArchTrait for Aarch64Arch {
    fn exception_init() {
        todo!()
    }

    #[inline]
    fn wait_for_interrupt() {
        cortex_a::asm::wfi();
    }

    #[inline]
    fn nop() {
        cortex_a::asm::nop();
    }

    fn fault_address() -> usize {
        todo!()
    }

    fn install_vm_page_table(base: usize, vmid: usize) {
        // restore vm's Stage2 MMU context
        let vttbr = (vmid << 48) | base;
        // println!("vttbr {:#x}", vttbr);
        msr!(VTTBR_EL2, vttbr);
        unsafe {
            core::arch::asm!("isb");
        }
    }

    #[inline]
    fn install_self_page_table(base: usize) {
        cortex_a::registers::TTBR0_EL2.set_baddr(base as u64);
        unsafe {
            core::arch::asm!("isb");
        }
    }

    fn disable_prefetch() {
        let mut cpu_extended_control: u64;
        mrs!(cpu_extended_control, S3_1_c15_c2_1);
        debug!("disable_prefetch: ori {:#x}", cpu_extended_control);
        cpu_extended_control &= !((0b11) << 32);
        cpu_extended_control &= !((0b11) << 35);
        debug!("disable_prefetch: new {:#x}", cpu_extended_control);
        msr!(S3_1_c15_c2_1, cpu_extended_control);
        let tmp: u64;
        mrs!(tmp, S3_1_c15_c2_1);
        debug!("disable_prefetch: test {:#x}", tmp);
    }

    fn mem_translate(va: usize) -> Option<usize> {
        use cortex_a::registers::PAR_EL1;
        const PAR_PA_MASK: u64 = ((1 << (48 - 12)) - 1) << 12; // 0xFFFF_FFFF_F000

        let par = PAR_EL1.get();
        arm_at!("s1e2r", va);
        let tmp = PAR_EL1.get();
        PAR_EL1.set(par);
        if (tmp & PAR_EL1::F::TranslationAborted.value) != 0 {
            None
        } else {
            let pa = (tmp & PAR_PA_MASK) as usize | (va & (PAGE_SIZE - 1));
            Some(pa)
        }
    }

    #[inline]
    fn current_stack_pointer() -> usize {
        cortex_a::registers::SP.get() as usize
    }
}

const PA2HVA: usize = 0b11 << 34; // 34 is pa limit 16GB
const_assert!(PA2HVA < 1 << VM_IPA_SIZE); // if not, the va will ocuppy the ipa2hva space, which is very dangerous

impl Address for usize {
    #[inline]
    fn pa2hva(self) -> usize {
        debug_assert_eq!(self & PA2HVA, 0, "illegal pa {self:#x}");
        self | PA2HVA
    }
}
