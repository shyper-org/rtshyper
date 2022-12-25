use crate::arch::ArchTrait;

pub const PAGE_SIZE: usize = 4096;
pub const PAGE_SHIFT: usize = 12;
pub const ENTRY_PER_PAGE: usize = PAGE_SIZE / 8;

pub type ContextFrame = super::context_frame::Aarch64ContextFrame;

pub const WORD_SIZE: usize = 8;
pub const PTE_PER_PAGE: usize = PAGE_SIZE / WORD_SIZE;

pub type Arch = Aarch64Arch;

pub struct Aarch64Arch;

impl ArchTrait for Aarch64Arch {
    fn exception_init() {
        todo!()
    }

    fn invalidate_tlb() {
        todo!()
    }

    fn wait_for_interrupt() {
        cortex_a::asm::wfi();
    }

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
}
