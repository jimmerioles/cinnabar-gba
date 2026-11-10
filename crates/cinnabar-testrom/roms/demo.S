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
    mov r0, #0x04000000
    mov r1, #0x0400
    add r1, r1, #0x03
    strh r1, [r0]
    mov r2, #0x06000000
    mov r3, #0
    mov r5, #160
line:
    mov r4, #240
pixel:
    strh r3, [r2], #2
    add r3, r3, #1
    subs r4, r4, #1
    bne pixel
    subs r5, r5, #1
    bne line
hang:
    b hang
