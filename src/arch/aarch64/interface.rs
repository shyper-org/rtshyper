pub const PAGE_SIZE: usize = 4096;
pub const PAGE_SHIFT: usize = 12;
pub const ENTRY_PER_PAGE: usize = PAGE_SIZE / 8;

pub type ContextFrame = super::context_frame::Aarch64ContextFrame;

pub const WORD_SIZE: usize = 8;
pub const PTE_PER_PAGE: usize = PAGE_SIZE / WORD_SIZE;
