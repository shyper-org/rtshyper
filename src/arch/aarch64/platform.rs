pub struct GicDesc {
    pub gicd_addr: usize,
    pub gicc_addr: usize,
    pub gich_addr: usize,
    pub gicv_addr: usize,
    pub maintenance_int_id: usize,
}
