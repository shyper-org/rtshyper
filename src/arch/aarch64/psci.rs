pub const PSCI_CPU_ON_AARCH64: usize = 0xc4000003;

pub fn power_arch_cpu_on(mpidr: usize, entry: usize, ctx: usize) -> usize {
    use super::smc::smc_call;
    smc_call(PSCI_CPU_ON_AARCH64, mpidr, entry, ctx)
}
