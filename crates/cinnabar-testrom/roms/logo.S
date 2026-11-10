.syntax unified
.arm

.macro RECT x, y, w, h, color
    ldr r0, =0x06000000 + ((\y * 240) + \x) * 2
    mov r1, #\w
    mov r2, #\h
    ldr r3, =\color
    mov r4, #(240 - \w) * 2
    mov r5, r1
    mov r6, r2
1:
    mov r7, r5
2:
    strh r3, [r0], #2
    subs r7, r7, #1
    bne 2b
    add r0, r0, r4
    subs r6, r6, #1
    bne 1b
.endm

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
    mov r0, #0x04000000
    mov r1, #0x0400
    add r1, r1, #0x03
    strh r1, [r0]

    @ Dark blue backdrop.
    mov r0, #0x06000000
    mov r1, #0x0C00
    mov r2, #0x9600
fill:
    strh r1, [r0], #2
    subs r2, r2, #1
    bne fill

    @ A blocky Cinnabar "C".
    RECT 76, 32, 88, 16, 0x001F
    RECT 76, 112, 88, 16, 0x001F
    RECT 76, 32, 16, 96, 0x001F
    RECT 112, 64, 28, 24, 0x7FFF

hang:
    b hang
