pub struct GicDesc {
    pub gicd_addr: u64,
    pub gicc_addr: u64,
    pub gich_addr: u64,
    pub gicv_addr: u64,
    pub maintenance_int_id: u64,
}
