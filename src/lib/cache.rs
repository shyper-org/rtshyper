global_asm!(include_str!("../arch/aarch64/cache.S"));

extern "C" {
    pub fn cache_invalidate_d(start: usize, len: usize);
    pub fn cache_clean_invalidate_d(start: usize, len: usize);
}
