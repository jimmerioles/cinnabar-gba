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

    ldr r0, =0x08000000 + irq_handler
    ldr r1, =0x03007FFC
    str r0, [r1]

    mov r4, #0x04000000
    ldrh r0, [r4, #4]
    orr r0, r0, #8
    strh r0, [r4, #4]
    add r5, r4, #0x200
    mov r0, #1
    strh r0, [r5]
    strh r0, [r5, #8]

loop:
    swi 0x05
    b loop

irq_handler:
    stmfd sp!, {r0-r3, lr}
    mov r0, #0x04000000
    add r0, r0, #0x200
    ldrh r1, [r0, #2]
    tst r1, #1
    beq done
    mov r2, #0x03000000
    ldr r3, [r2]
    add r3, r3, #1
    str r3, [r2]
    mov r2, #1
    strh r2, [r0, #2]
    mov r2, #0x06000000
    ldr r3, =0x7FFF
    strh r3, [r2]
done:
    ldmfd sp!, {r0-r3, pc}
