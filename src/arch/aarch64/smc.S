// smc(usize, usize, usize, usize) -> (usize, usize, usize, usize)
.global smc
smc:
    smc #0x0
    stp	x0, x1, [x8]
    stp	x2, x3, [x8, #16]
    ret
