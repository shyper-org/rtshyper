/* Copyright (c) 2012-2013, Linaro Limited
   All rights reserved.
   Redistribution and use in source and binary forms, with or without
   modification, are permitted provided that the following conditions are met:
       * Redistributions of source code must retain the above copyright
         notice, this list of conditions and the following disclaimer.
       * Redistributions in binary form must reproduce the above copyright
         notice, this list of conditions and the following disclaimer in the
         documentation and/or other materials provided with the distribution.
       * Neither the name of the Linaro nor the
         names of its contributors may be used to endorse or promote products
         derived from this software without specific prior written permission.
   THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
   "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
   LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
   A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
   HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
   SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
   LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
   DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
   THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
   (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
   OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE. */
/* Assumptions:
 *
 * ARMv8-a, AArch64
 * Unaligned accesses
 *
 */


/* By default we assume that the DC instruction can be used to zero
   data blocks more efficiently.  In some circumstances this might be
   unsafe, for example in an asymmetric multiprocessor environment with
   different DC clear lengths (neither the upper nor lower lengths are
   safe to use).  The feature can be disabled by defining DONT_USE_DC.
   If code may be run in a virtualized environment, then define
   MAYBE_VIRT.  This will cause the code to cache the system register
   values rather than re-reading them each call.  */

.macro def_fn f p2align=0
.text
.p2align \p2align
.global \f
//	.type \f, %function
\f:
	.endm
def_fn memset p2align=6
	mov	x8, x0		/* Preserve return value.  */
	ands	w7, w1, #255

	orr	w7, w7, w7, lsl #8
	orr	w7, w7, w7, lsl #16
	orr	x7, x7, x7, lsl #32
.Ltail_maybe_long:
	cmp	x2, #64
	b.ge	.Lnot_short
.Ltail_maybe_tiny:
	cmp	x2, #15
	b.le	.Ltail15tiny
.Ltail63:
	ands	x3, x2, #0x30
	b.eq	.Ltail15
	add	x8, x8, x3
	cmp	w3, #0x20
	b.eq	1f
	b.lt	2f
	stp	x7, x7, [x8, #-48]
1:
	stp	x7, x7, [x8, #-32]
2:
	stp	x7, x7, [x8, #-16]
.Ltail15:
	and	x2, x2, #15
	add	x8, x8, x2
	stp	x7, x7, [x8, #-16]	/* Repeat some/all of last store. */
	ret
.Ltail15tiny:
	/* Set up to 15 bytes.  Does not assume earlier memory
	   being set.  */
	tbz	x2, #3, 1f
	str	x7, [x8], #8
1:
	tbz	x2, #2, 1f
	str	w7, [x8], #4
1:
	tbz	x2, #1, 1f
	strh	w7, [x8], #2
1:
	tbz	x2, #0, 1f
	strb	w7, [x8]
1:
	ret
	/* Critical loop.  Start at a new cache line boundary.  Assuming
	 * 64 bytes per line, this ensures the entire loop is in one line.  */
	.p2align 6
.Lnot_short:
	neg	x4, x8
	ands	x4, x4, #15
	b.eq	2f
	/* Bring x8 to 128-bit (16-byte) alignment.  We know that there's
	 * more than that to set, so we simply store 16 bytes and advance by
	 * the amount required to reach alignment.  */
	sub	x2, x2, x4
	stp	x7, x7, [x8]
	add	x8, x8, x4
	/* There may be less than 63 bytes to go now.  */
	cmp	x2, #63
	b.le	.Ltail63
2:
	sub	x8, x8, #16		/* Pre-bias.  */
	sub	x2, x2, #64
1:
	stp	x7, x7, [x8, #16]
	stp	x7, x7, [x8, #32]
	stp	x7, x7, [x8, #48]
	stp	x7, x7, [x8, #64]!
	subs	x2, x2, #64
	b.ge	1b
	tst	x2, #0x3f
	add	x8, x8, #16
	b.ne	.Ltail63
	ret
