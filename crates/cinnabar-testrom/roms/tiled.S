.syntax unified
.arm

.section .text
.global _start
_start:
    b entry
    .org 0xA0
    .ascii "CINNABAR"
    .space 4
    .ascii "CINB"
    .ascii "01"
    .byte 0x96
    .byte 0x00
    .byte 0x00
    .space 7
    .byte 0x00
    .byte 0x00
    .space 2
    .org 0xC0
entry:
    @ Mode 0, BG0 on, OBJ on, 1D sprite mapping.
    mov r0, #0x04000000
    mov r1, #0x1100
    add r1, r1, #0x40
    strh r1, [r0]
    ldr r1, =0x1C00          @ BG0: char base 0, screen base 0xE000, 4bpp
    strh r1, [r0, #8]

    @ Palette: backdrop, red, white, green, blue.
    mov r2, #0x05000000
    ldr r1, =0x0C00
    strh r1, [r2], #2
    ldr r1, =0x001F
    strh r1, [r2], #2
    ldr r1, =0x7FFF
    strh r1, [r2], #2
    ldr r1, =0x03E0
    strh r1, [r2], #2
    ldr r1, =0x7C00
    strh r1, [r2], #2

    @ Four solid 8x8 tiles in the character block.
    mov r2, #0x06000000
    ldr r1, =0x1111
    mov r3, #16
t0:
    strh r1, [r2], #2
    subs r3, r3, #1
    bne t0
    ldr r1, =0x2222
    mov r3, #16
t1:
    strh r1, [r2], #2
    subs r3, r3, #1
    bne t1
    ldr r1, =0x3333
    mov r3, #16
t2:
    strh r1, [r2], #2
    subs r3, r3, #1
    bne t2
    ldr r1, =0x4444
    mov r3, #16
t3:
    strh r1, [r2], #2
    subs r3, r3, #1
    bne t3

    @ Sprite tile 0 lives at 0x06010000 and is solid red.
    mov r2, #0x06000000
    add r2, r2, #0x10000
    ldr r1, =0x1111
    mov r3, #16
ts:
    strh r1, [r2], #2
    subs r3, r3, #1
    bne ts

    @ 32x20 tilemap: 4x4 blocks of red/white/green/blue tiles.
    mov r0, #0x06000000
    add r0, r0, #0xE000
    mov r5, #20
    mov r6, #0
mrow:
    mov r7, #32
    mov r8, #0
mcol:
    mov r4, r8, lsr #2
    add r4, r4, r6, lsr #2
    and r4, r4, #3
    strh r4, [r0], #2
    add r8, r8, #1
    subs r7, r7, #1
    bne mcol
    add r6, r6, #1
    subs r5, r5, #1
    bne mrow

    @ One 8x8 sprite at (104, 64), tile 0, palette 0. It sits over a white
    @ tile so the 50% alpha blend is visible.
    mov r2, #0x07000000
    mov r3, #64
    strh r3, [r2], #2
    mov r3, #104
    strh r3, [r2], #2
    mov r3, #0
    strh r3, [r2], #2
    strh r3, [r2], #2

    @ Alpha-blend the sprite over BG0.
    mov r0, #0x04000000
    ldr r1, =0x2151
    strh r1, [r0, #0x50]
    ldr r1, =0x0808
    strh r1, [r0, #0x52]

hang:
    b hang
