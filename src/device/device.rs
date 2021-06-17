pub const ARM_CORTEX_A57: u8 = 0;
pub const ARM_NVIDIA_DENVER: u8 = 1;

pub struct BlkStat {
    pub read_req: usize,
    pub write_req: usize,
    pub read_byte: usize,
    pub write_byte: usize,
}

impl BlkStat {
    pub fn default() -> BlkStat {
        BlkStat {
            read_req: 0,
            write_req: 0,
            read_byte: 0,
            write_byte: 0,
        }
    }
}
