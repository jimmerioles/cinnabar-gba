.syntax unified
.arm

@ Fill a rectangle using r8 as the color. r0/r1/r2/r4/r5/r6/r7 are scratch.
.macro RECT x, y, w, h
    ldr r0, =0x06000000 + ((\y * 240) + \x) * 2
    mov r1, #\w
    mov r2, #\h
    mov r4, #(240 - \w) * 2
    mov r5, r1
    mov r6, r2
1:
    mov r7, r5
2:
    strh r8, [r0], #2
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

    @ Dark blue background.
    mov r0, #0x06000000
    ldr r1, =0x0C00
    mov r2, #0x9600
bg:
    strh r1, [r0], #2
    subs r2, r2, #1
    bne bg

    mov r10, #0x04000000
    add r11, r10, #0x100

loop:
    @ Wait for the VBlank flag to clear, then for the next VBlank.
wait_clear:
    ldrh r1, [r10, #4]
    tst r1, #1
    bne wait_clear
wait_set:
    ldrh r1, [r10, #4]
    tst r1, #1
    beq wait_set

    ldrh r9, [r11, #0x30]      @ KEYINPUT, active low

    @ All indicators off.
    ldr r8, =0x1CE7
    RECT 52, 44, 16, 24        @ up
    RECT 52, 92, 16, 24        @ down
    RECT 24, 68, 24, 16        @ left
    RECT 72, 68, 24, 16        @ right
    RECT 176, 44, 24, 24       @ A
    RECT 144, 84, 20, 20       @ B
    RECT 88, 132, 24, 10       @ start
    RECT 120, 132, 24, 10      @ select

    @ D-pad, green when pressed.
    ldr r8, =0x03E0
    tst r9, #0x40
    beq up_on
    tst r9, #0x80
    beq down_on
    tst r9, #0x20
    beq left_on
    tst r9, #0x10
    beq right_on
    b buttons

up_on:
    RECT 52, 44, 16, 24
    b buttons
down_on:
    RECT 52, 92, 16, 24
    b buttons
left_on:
    RECT 24, 68, 24, 16
    b buttons
right_on:
    RECT 72, 68, 24, 16

buttons:
    ldr r8, =0x001F
    tst r9, #0x01
    beq a_on
    tst r9, #0x02
    beq b_on
    b start_select

a_on:
    RECT 176, 44, 24, 24
    b start_select
b_on:
    RECT 144, 84, 20, 20

start_select:
    ldr r8, =0x7FFF
    tst r9, #0x08
    beq start_on
    tst r9, #0x04
    beq select_on
    b loop

start_on:
    RECT 88, 132, 24, 10
    b loop
select_on:
    RECT 120, 132, 24, 10
    b loop
