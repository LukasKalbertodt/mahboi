; Writes the value \2 to $FF00 + \1
SetIoReg: MACRO
    LD A, \2
    LD [$FF00 + \1], A
    ENDM

SECTION "BIOS", ROM0[0]
    ; Initialize IO registers
    SetIoReg $05, $00 ; TIMA
    SetIoReg $06, $00   ; TMA
    SetIoReg $07, $00   ; TAC
    SetIoReg $10, $80   ; NR10
    SetIoReg $11, $BF   ; NR11
    SetIoReg $12, $F3   ; NR12
    SetIoReg $14, $BF   ; NR14
    SetIoReg $16, $3F   ; NR21
    SetIoReg $17, $00   ; NR22
    SetIoReg $19, $BF   ; NR24
    SetIoReg $1A, $7F   ; NR30
    SetIoReg $1B, $FF   ; NR31
    SetIoReg $1C, $9F   ; NR32
    SetIoReg $1E, $BF   ; NR33
    SetIoReg $20, $FF   ; NR41
    SetIoReg $21, $00   ; NR42
    SetIoReg $22, $00   ; NR43
    SetIoReg $23, $BF   ; NR44
    SetIoReg $24, $77   ; NR50
    SetIoReg $25, $F3   ; NR51
    SetIoReg $26, $F1   ; NR52
    SetIoReg $40, $91   ; LCDC
    SetIoReg $42, $00   ; SCY
    SetIoReg $43, $00   ; SCX
    SetIoReg $45, $00   ; LYC
    SetIoReg $47, $FC   ; BGP
    SetIoReg $48, $FF   ; OBP0
    SetIoReg $49, $FF   ; OBP1
    SetIoReg $4A, $00   ; WY
    SetIoReg $4B, $00   ; WX
    SetIoReg $FF, $00   ; IE

    ; Initialize registers
    LD BC, $0013
    LD DE, $00D8
    LD HL, $014D
    LD SP, $FFFE


; Turn off boot rom (this code have to be the last four bytes of the BIOS)
SECTION "BIOS END", ROM0[$FC]
    SetIoReg $50, $01
