; This ROM is a test rom to check how large the input lag is. Every V-Blank
; this program writes a palette value. If no button is presse, it's a standard
; palette. If any button is pressed, it's "inverted" (meaning the pattern 0
; is white).

SECTION "START_ROM", ROM0[$100]
    NOP
    JP $150

SECTION "MAIN", ROM0[$150]
    ; We need to select all input buttons
    XOR A
    LD [$FF00 + $00], A


wait_for_vblank:
    LD A, [$FF00 + $41]
    AND $03
    CP $01
    JR NZ, wait_for_vblank

    ; check if any button is pressed
    LD A, [$FF00 + 00]
    AND $0F
    CP $0F
    JR Z, normal_palette
inverted_palette:
    LD A, $1B    ; 0b00011011
    JR write_palette
normal_palette:
    LD A, $E4    ; 0b11100100
write_palette:
    LD [$FF00 + $47], A

wait_for_next_frame:
    LD A, [$FF00 + $41]
    AND $03
    CP $01
    JR Z, wait_for_next_frame

    JR wait_for_vblank
