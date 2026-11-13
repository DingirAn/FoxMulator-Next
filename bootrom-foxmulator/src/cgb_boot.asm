; FoxMulator minimal CGB bootstrap BIOS sketch.
; This variant intentionally has no DMG compatibility path.
include "hardware.inc"
include "logo.inc"

DEF rBANK EQU $FF50

SECTION "BootCode", ROM0[$0000]
Start:
    di
    ld sp, $FFFE

    ; SameBoy-compatible hardware foundations, without DMG logo handling.
    call ClearVRAM
    ld a, 1
    ldh [rVBK], a
    call ClearVRAM
    xor a
    ldh [rVBK], a

    ld h, HIGH(_OAMRAM)
    ld c, sizeof_OAM_ATTRS * OAM_COUNT
.clearOAM
    ldi [hl], a
    dec c
    jr nz, .clearOAM

IF !DEF(CGB0)
    ld c, 16
    ld hl, _AUD3WAVERAM
.clearWave
    ldi [hl], a
    cpl
    dec c
    jr nz, .clearWave
ENDC

    ; Same audio power-up values used by the reference CGB boot.
    ld a, AUDENA_ON
    ldh [rNR52], a
    ldh [rNR11], a
    ld a, $F3
    ldh [rNR12], a
    ldh [rNR51], a
    ld a, $77
    ldh [rNR50], a
    ld a, %11_11_11_00
    ldh [rBGP], a

    call LoadLogoTiles
    ; The cleared tilemap starts blank. The logo is revealed tile by tile
    ; after LCD startup, so arbitrary logos get a visible CGB intro animation.
    ld hl, PaletteData
    call WritePalettes

    ld a, LCDCF_ON | LCDCF_BLK01 | LCDCF_BGON
    ldh [rLCDC], a
    call AnimateLogo
    ; SameBoy's two-note CGB boot chime. The existing NR11/NR12 setup supplies
    ; the duty and envelope; PlaySound changes frequency and starts CH1.
    ld a, $83
    call PlaySound
    call WaitFrame
    ld a, $C1
    call PlaySound
    call AnimateColor
    ; SameBoy leaves the cartridge with a white BG palette after its fade.
    ; Keep the custom animation visible, then restore that post-boot state.
    call WaitVBlank
    ld hl, PaletteWhite
    call WritePalettes
    call WaitFrame
    xor a
    ldh [rSVBK], a
    ; Match the principal CGB post-boot CPU state used by SameBoy. The value
    ; in A is deliberately $11: any non-zero FF50 write unmaps the boot ROM.
    ld b, a
    ld c, a
    ld de, $FF56
    ld hl, $000D
    ld a, $11
    jp BootGame

ClearVRAM:
    ld hl, _VRAM
    xor a
.loop
    ldi [hl], a
    bit 5, h
    jr z, .loop
    ret

LoadLogoTiles:
    ld hl, LogoTiles
    ; Tile 0 stays blank (VRAM was cleared); logo tiles start at tile 1.
    ld de, _VRAM + 16 - 1
    ld bc, LOGO_TILE_BYTES
.loop
    inc de
    ld a, [hli]
    ld [de], a
    dec bc
    ld a, b
    or c
    jr nz, .loop
    ret

WritePalettes:
    ld a, %1000_0000
    ldh [rBGPI], a
    ld b, 64
.loop
    ld a, [hli]
    ldh [rBGPD], a
    dec b
    jr nz, .loop
    ret

WritePalette0:
    ld a, %1000_0000
    ldh [rBGPI], a
    ld b, 8
.loop
    ld a, [hli]
    ldh [rBGPD], a
    dec b
    jr nz, .loop
    ret

; Reveal one generated tile per frame. VRAM writes happen during VBlank, so
; this remains valid on real CGB hardware instead of relying on mode-3 writes.
AnimateLogo:
    ld hl, _SCRN0 + 6 * SCRN_VX_B + 2
    ld de, LogoTilemap
    ld b, 3
.row
    ld c, 16
.cell
    call WaitVBlank
    ld a, [de]
    inc de
    ldi [hl], a
    call WaitFrame
    dec c
    jr nz, .cell
    ld a, l
    add SCRN_VX_B - 16
    ld l, a
    jr nc, .sameRowPage
    inc h
.sameRowPage
    dec b
    jr nz, .row
    ret

; SameBoy's original intro walks through blue tile variants. Here the logo is
; arbitrary, so the equivalent visual phase is a CGB palette transition. It
    ; takes 60 steps and two frames per step: the final steps hold the finished
    ; logo on screen for roughly two seconds before the post-boot cleanup.
    AnimateColor:
    ld hl, ColorAnimation
    ld c, 60
.step
    call WaitVBlank
    call WritePalette0
    call WaitFrame
    call WaitFrame
    dec c
    jr nz, .step
    ret

WaitVBlank:
.wait
    ldh a, [rLY]
    cp 144
    jr c, .wait
    ret

PlaySound:
    ldh [rNR13], a
    ld a, $87
    ldh [rNR14], a
    ret

WaitFrame:
.vblank
    ldh a, [rLY]
    cp 144
    jr c, .vblank
.visible
    ldh a, [rLY]
    cp 144
    jr nc, .visible
    ret

SECTION "BootGame", ROM0[$00FE]
BootGame:
    ldh [rBANK], a

SECTION "BootData", ROM0[$0200]
LogoTiles:
    incbin "logo.2bpp"
LogoTilemap:
    incbin "tilemap.bin"

PaletteData:
    ; White, light gray, dark gray, black; repeated for all eight palettes.
    REPT 8
        dw $7FFF, $5294, $2108, $0000
    ENDR

ColorAnimation:
    ; Four RGB15 colors per step: white, light tone, dark tone, black.
    dw $7FFF, $5294, $2108, $0000
    dw $7FFF, $5294, $2508, $0000
    dw $7FFF, $5693, $2508, $0400
    dw $7FFF, $5693, $2907, $0400
    dw $7FFF, $5A92, $2D07, $0800
    dw $7FFF, $5A92, $2D07, $0820
    dw $7FFF, $5E92, $3107, $0C20
    dw $7FFF, $5E91, $3127, $0C20
    dw $7FFF, $6291, $3526, $1020
    dw $7FFF, $6290, $3926, $1020
    dw $7FFF, $6290, $3926, $1420
    dw $7FFF, $6690, $3D26, $1420
    dw $7FFF, $668F, $4126, $1820
    dw $7FFF, $6A8F, $4125, $1840
    dw $7FFF, $6A8E, $4525, $1C40
    dw $7FFF, $6E8E, $4925, $1C40
    dw $7FFF, $6E8E, $4925, $2040
    dw $7FFF, $6E8D, $4D25, $2040
    dw $7FFF, $728D, $5124, $2440
    dw $7FFF, $728C, $5144, $2440
    dw $7FFF, $768C, $5544, $2840
    dw $7FFF, $768C, $5544, $2860
    dw $7FFF, $7A8B, $5944, $2C60
    dw $7FFF, $7A8B, $5D43, $2C60
    dw $7FFF, $7E8A, $5D43, $3060
    dw $7FFF, $7E8A, $6143, $3060
    REPT 34
        dw $7FFF, $7E8A, $6143, $3060
    ENDR

PaletteWhite:
    REPT 8
        dw $7FFF, $7FFF, $7FFF, $7FFF
    ENDR


BootEnd:
IF BootEnd > $0900
    FAIL "minimal CGB BIOS overflowed: {BootEnd}"
ENDC
    ds $100 + $800 - @
