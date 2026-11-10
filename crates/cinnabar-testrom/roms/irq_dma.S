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
    @ Mode 3.
    mov r0, #0x04000000
    mov r1, #0x0400
    add r1, r1, #0x03
    strh r1, [r0]

    @ DMA3 immediate 32-bit fill with a fixed dark-blue source word.
    @ The flat binary's labels are section offsets; add the ROM base.
    ldr r0, =0x08000000 + color_word
    ldr r1, =0x06000000
    ldr r2, =19200
    ldr r3, =0x8500
    mov r4, #0x04000000
    str r0, [r4, #0xD4]
    str r1, [r4, #0xD8]
    strh r2, [r4, #0xDC]
    strh r3, [r4, #0xDE]

    @ Install the IRQ handler.
    ldr r0, =0x08000000 + irq_handler
    ldr r1, =0x03007FFC
    str r0, [r1]

    @ VBlank IRQ, timer 0 IRQ, then start timer 0.
    mov r4, #0x04000000
    ldrh r0, [r4, #4]
    orr r0, r0, #8
    strh r0, [r4, #4]
    add r5, r4, #0x200
    add r6, r4, #0x100
    mov r0, #9
    strh r0, [r5]
    mov r0, #0
    strh r0, [r6]
    mov r0, #0xC0
    strh r0, [r6, #2]
    mov r0, #1
    strh r0, [r5, #8]

hang:
    b hang

irq_handler:
    stmfd sp!, {r0-r4, lr}
    mov r0, #0x04000000
    add r0, r0, #0x200
    ldrh r1, [r0, #2]

    tst r1, #1
    beq no_vblank
    mov r2, #0x03000000
    ldr r3, [r2]
    add r3, r3, #1
    str r3, [r2]
    mov r2, #1
    strh r2, [r0, #2]

no_vblank:
    tst r1, #8
    beq no_timer
    mov r2, #0x03000000
    add r2, r2, #4
    ldr r3, [r2]
    add r3, r3, #1
    str r3, [r2]
    mov r2, #0x06000000
    ldr r3, =0x03E0
    strh r3, [r2]
    mov r2, #8
    strh r2, [r0, #2]

no_timer:
    ldmfd sp!, {r0-r4, pc}

.align 4
color_word:
    .word 0x0C000C00
