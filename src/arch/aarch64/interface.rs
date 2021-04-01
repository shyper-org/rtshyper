pub const PAGE_SIZE: usize = 4096;
pub const PAGE_SHIFT: usize = 12;
pub const ENTRY_PER_PAGE: usize = PAGE_SIZE / 8;

pub type ContextFrame = super::context_frame::Aarch64ContextFrame;
