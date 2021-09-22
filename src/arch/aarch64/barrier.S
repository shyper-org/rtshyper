.text
.global spin_lock
spin_lock:
	sub	sp, sp, #0x20
	str	x0, [sp, #8]
    mov	w0, #0x1                   	// #1
    str	w0, [sp, #28]
    ldr	x2, [sp, #8]
    ldr	w1, [sp, #28]
    ldr	x0, [sp, #8]
1:
    ldaxr	w3, [x2]
    cbnz	w3,	1b
    stxr	w3, w1, [x2]
    cbnz	w3,	1b
    mov	w0, w3
    str	w0, [sp, #24]
    add	sp, sp, #0x20
    ret

.text
.global spin_lock
spin_unlock:
    sub	sp, sp, #0x10
    str	x0, [sp, #8]
    ldr	x0, [sp, #8]
    stlr	wzr, [x0]
    add	sp, sp, #0x10
    ret
