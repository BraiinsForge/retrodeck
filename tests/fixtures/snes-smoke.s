; Original 32 KiB LoROM smoke test. No game assets or external ROM required.
.setcpu "65816"

.segment "CODE"
.a8
.i8
reset:
    sei
    cld
    lda #$8f
    sta $2100               ; Force blank at full brightness during setup.
    stz $4200               ; Disable interrupts during setup.
    stz $2121
    lda #$1f
    sta $2122
    stz $2122               ; Set the backdrop to red (BGR555 $001f).
    lda #$0f
    sta $2100               ; Enable output.
    lda #$80
    sta $4200               ; Enable NMI at vblank.
loop:
    wai
    bra loop

nmi:
    pha
    stz $2121
    stz $2122
    lda #$7c
    sta $2122               ; Change the backdrop to blue after reset.
    pla
    rti

.segment "HEADER"
    .byte "RETRODECK SMOKE TEST "  ; 21 bytes
    .byte $20, $00, $05, $00     ; Slow LoROM, no SRAM, 32 KiB
    .byte $01, $00, $00          ; NTSC, developer, revision
    .word $ffff, $0000           ; Complementary header fields

.segment "VECTORS"
    .word 0, 0, 0, 0, 0
    .word nmi, reset, reset      ; Emulation-mode NMI, RESET, IRQ
