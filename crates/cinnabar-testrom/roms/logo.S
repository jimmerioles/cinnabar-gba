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

    @ A short PSG loop so the browser arcade has something to play as soon
    @ as the visitor enables sound. VBlank drives the note sequencer.
    ldr r0, =0x08000000 + irq_handler
    ldr r1, =0x03007FFC
    str r0, [r1]

    mov r0, #0x04000000
    mov r1, #1
    mov r2, r0
    add r2, r2, #0x200
    strh r1, [r2]             @ IE: VBlank
    strh r1, [r2, #8]         @ IME

    mov r1, #0x80
    strh r1, [r0, #0x84]      @ SOUNDCNT_X: master enable
    ldr r1, =0x1177
    strh r1, [r0, #0x80]      @ SOUNDCNT_L: CH1 both sides, volume 7
    ldr r1, =0x0002
    strh r1, [r0, #0x82]      @ SOUNDCNT_H: PSG volume 100%
    ldr r1, =0xF080
    strh r1, [r0, #0x62]      @ CH1 duty 50%, full volume
    ldr r1, =0x8706
    strh r1, [r0, #0x64]      @ CH1 frequency 1798, trigger

hang:
    b hang

irq_handler:
    stmfd sp!, {r0-r7, lr}
    mov r0, #0x04000000
    add r0, r0, #0x200
    ldrh r1, [r0, #2]
    tst r1, #1
    beq irq_done
    mov r1, #1
    strh r1, [r0, #2]         @ acknowledge VBlank

    mov r2, #0x03000000
    add r2, r2, #8
    ldr r3, [r2]
    add r3, r3, #1
    str r3, [r2]
    and r3, r3, #15
    cmp r3, #0
    bne irq_done

    add r2, r2, #4
    ldr r3, [r2]
    add r3, r3, #1
    and r3, r3, #7
    str r3, [r2]
    ldr r4, =0x08000000 + note_table
    add r4, r4, r3, lsl #1
    ldrh r1, [r4]
    ldr r5, =0x8000
    orr r1, r1, r5
    mov r0, #0x04000000
    strh r1, [r0, #0x64]

irq_done:
    ldmfd sp!, {r0-r7, pc}

.align 2
note_table:
    .short 1798, 1825, 1849, 1881, 1899, 1923, 1899, 1881
