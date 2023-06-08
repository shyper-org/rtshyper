.equ CPU_STACK_OFFSET, 0x5000
.equ CPU_STACK_SIZE, (4096 * 128)

.section .text.boot

.global _start
_start:
    mov x20, x0 // save fdt pointer to x20
#ifdef PLATFORM_TX2
    mrs x0, mpidr_el1
    and x1, x0, #0x100
    cbz x1, 1f
    and x0, x0, #3
    b   2f
1:  wfe
    b   1b

#endif
#if defined(PLATFORM_QEMU) || defined(PLATFORM_PI4)
    mrs x0, mpidr_el1
    and x0, x0, #7
#endif

2:  /*
     * only cluster 1 cpu 0,1,2,3 reach here
     * x0 holds core_id (indexed from zero)
     */

    // disable cache and MMU
    mrs x1, sctlr_el2
    bic x1, x1, #0xf
    msr sctlr_el2, x1

    mov x19, x0 // save core_id

    // cache_invalidate(0): clear dl1$
    mov x0, #0
    bl  cache_invalidate

    // if (core_id == 0) cache_invalidate(2): clear l2$
    cbnz x19, 3f
    mov x0, #2
    bl  cache_invalidate

3:
    mov x0, x19 // restore core_id
    ic  iallu // clear icache

    // setup stack sp per core
    adrp x1, boot_stack
    mov x2, (4096 * 2)
    mul x3, x0, x2
    add x1, x1, x2
    add x1, x1, x3

    mov sp, x1

    // if core_id is not zero, skip bss clearing and pt_populate
    cbnz x0, 5f

    ldr x0, =_bss_begin
    ldr x1, =_bss_end
    sub x2, x1, x0
    mov x1, xzr
    bl  memset

    adrp x0, lvl1_page_table
    adrp x1, lvl2_page_table
    bl  pt_populate
5:
    // Trap nothing from EL1 to El2
    mov x3, xzr
    msr cptr_el2, x3

    adrp x0, lvl1_page_table
    bl  mmu_init

    mov x0, x19
    bl  cpu_map_self

    //tlbi	alle2
    //dsb	nsh
    //isb
    msr ttbr0_el2, x0

    ldr x0, =(0x80080019)
    msr hcr_el2, x0

    mov x1, 1
    msr spsel, x1
    ldr x1, =CPU
    add x1, x1, #(CPU_STACK_OFFSET + CPU_STACK_SIZE)
    sub	x1, x1, #0x110
    mov sp, x1

    ldr x1, =vectors
    msr vbar_el2, x1

    mov x0, x19
    ldr x3, =init


    ldr x2, =(0x30c51835)
    msr sctlr_el2, x2

	tlbi	alle2
	dsb	nsh
	isb

    mov x1, x20
    br  x3

/*
 * snipet from "Application Note Bare-metal Boot Code for ARMv8-A
 * Processors - Version 1.0"
 *
 * x0 - cache level to be invalidated (0 - dl1$, 1 - il1$, 2 - l2$)
 */
cache_invalidate:
	msr csselr_el1, x0
	mrs x4, ccsidr_el1 // read cache size id.
	and x1, x4, #0x7
	add x1, x1, #0x4 // x1 = cache line size.
	ldr x3, =0x7fff
	and x2, x3, x4, lsr #13 // x2 = cache set number – 1.
	ldr x3, =0x3ff
	and x3, x3, x4, lsr #3 // x3 = cache associativity number – 1.
	clz w4, w3 // x4 = way position in the cisw instruction.
	mov x5, #0 // x5 = way counter way_loop.
way_loop:
	mov x6, #0 // x6 = set counter set_loop.
set_loop:
	lsl x7, x5, x4
	orr x7, x0, x7 // set way.
	lsl x8, x6, x1
	orr x7, x7, x8 // set set.
	dc cisw, x7 // clean and invalidate cache line.
	add x6, x6, #1 // increment set counter.
	cmp x6, x2 // last set reached yet?
	ble set_loop // if not, iterate set_loop,
	add x5, x5, #1 // else, next way.
	cmp x5, x3 // last way reached yet?
	ble way_loop // if not, iterate way_loop
	ret

.align 12
.section .data.boot
boot_stack:
    .space 4096 * 16
boot_stack_top:

.section .bss

.align 12
.global lvl1_page_table
lvl1_page_table:
    .space 4096

.align 12
.global lvl2_page_table
lvl2_page_table:
    .space 4096
