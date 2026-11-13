/* Bounded CGB boot comparison tool. Requires SameBoy Core built by the local
 * preview tester and deliberately does not modify FoxMulator's PPU/core. */
#include <stdint.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "gb.h"
#include "memory.h"

static const char *boot_path;

static uint32_t pixels[160 * 144];

static uint32_t rgb_encode(GB_gameboy_t *unused, uint8_t r, uint8_t g, uint8_t b)
{
    (void) unused;
    return 0xFF000000u | ((uint32_t) r << 16) | ((uint32_t) g << 8) | b;
}

static void load_boot_rom(GB_gameboy_t *gb, GB_boot_rom_t type)
{
    (void) type;
    if (GB_load_boot_rom(gb, boot_path)) {
        fprintf(stderr, "cannot load boot ROM: %s\n", boot_path);
        exit(2);
    }
}

static void dump_palette(const char *name, const uint8_t *palette, size_t size)
{
    printf("%s=", name);
    for (size_t i = 0; i < size; i++) printf("%02X%s", palette[i], i + 1 == size ? "" : ":");
    putchar('\n');
}

static void dump_state(const char *label, GB_gameboy_t *gb)
{
    size_t size = 0;
    uint16_t bank = 0;
    const uint8_t *bg = GB_get_direct_access(gb, GB_DIRECT_ACCESS_BGP, &size, &bank);
    const uint8_t *obj = GB_get_direct_access(gb, GB_DIRECT_ACCESS_OBP, &size, &bank);
    GB_registers_t *r = GB_get_registers(gb);

    printf("[%s]\n", label);
    printf("AF=%04X BC=%04X DE=%04X HL=%04X SP=%04X PC=%04X\n",
           r->af, r->bc, r->de, r->hl, r->sp, r->pc);
    printf("FF4C=%02X FF4F=%02X FF50=%02X FF68=%02X FF69=%02X "
           "FF6A=%02X FF6B=%02X FF40=%02X FF41=%02X FF42=%02X FF43=%02X "
           "FF47=%02X FF48=%02X FF49=%02X FF70=%02X\n",
           GB_read_memory(gb, 0xFF4C), GB_read_memory(gb, 0xFF4F),
           GB_read_memory(gb, 0xFF50), GB_read_memory(gb, 0xFF68),
           GB_read_memory(gb, 0xFF69), GB_read_memory(gb, 0xFF6A),
           GB_read_memory(gb, 0xFF6B), GB_read_memory(gb, 0xFF40),
           GB_read_memory(gb, 0xFF41), GB_read_memory(gb, 0xFF42),
           GB_read_memory(gb, 0xFF43), GB_read_memory(gb, 0xFF47),
           GB_read_memory(gb, 0xFF48), GB_read_memory(gb, 0xFF49),
           GB_read_memory(gb, 0xFF70));
    printf("AUDIO=NR10:%02X NR11:%02X NR12:%02X NR13:%02X NR14:%02X "
           "NR19:%02X NR1A:%02X NR50:%02X NR51:%02X NR52:%02X\n",
           GB_read_memory(gb, 0xFF10), GB_read_memory(gb, 0xFF11),
           GB_read_memory(gb, 0xFF12), GB_read_memory(gb, 0xFF13),
           GB_read_memory(gb, 0xFF14), GB_read_memory(gb, 0xFF19),
           GB_read_memory(gb, 0xFF1A), GB_read_memory(gb, 0xFF24),
           GB_read_memory(gb, 0xFF25), GB_read_memory(gb, 0xFF26));
    printf("cgb_mode=%d boot_finished=%d vram_bank=%d ram_bank=%u "
           "rom_bank=%u cart_ram_bank=%u pending_cycles=%u\n",
           gb->cgb_mode, gb->boot_rom_finished, gb->cgb_vram_bank,
           gb->cgb_ram_bank, gb->mbc_rom_bank, gb->mbc_ram_bank,
           gb->pending_cycles);
    dump_palette("BG_PALETTE", bg, 0x40);
    dump_palette("OBJ_PALETTE", obj, 0x40);
}

static void write_ppm(const char *path)
{
    FILE *file = fopen(path, "wb");
    if (!file) { perror(path); exit(2); }
    fprintf(file, "P6\n160 144\n255\n");
    for (size_t i = 0; i < 160 * 144; i++) {
        uint32_t pixel = pixels[i];
        fputc((pixel >> 16) & 0xFF, file);
        fputc((pixel >> 8) & 0xFF, file);
        fputc(pixel & 0xFF, file);
    }
    fclose(file);
}

int main(int argc, char **argv)
{
    if (argc < 4 || argc > 5) {
        fprintf(stderr, "usage: boot_state_dump <boot.bin> <cart.gb/c> <prefix> [post_ticks]\n");
        return 2;
    }
    boot_path = argv[1];
    unsigned long long post_ticks = argc == 5 ? strtoull(argv[4], NULL, 0) : 140448;
    GB_gameboy_t gb_storage;
    GB_gameboy_t *gb = &gb_storage;
    GB_init(gb, GB_MODEL_CGB_E);
    GB_set_boot_rom_load_callback(gb, load_boot_rom);
    GB_set_pixels_output(gb, pixels);
    GB_set_rgb_encode_callback(gb, rgb_encode);
    if (GB_load_rom(gb, argv[2])) {
        fprintf(stderr, "cannot load cartridge: %s\n", argv[2]);
        return 2;
    }

    unsigned long long boot_ticks = 0;
    unsigned calls = 0;
    bool saw_boot = false;
    GB_gameboy_t *before = malloc(sizeof *before);
    if (!before) return 2;
    while ((GB_read_memory(gb, 0xFF50) & 1) == 0) {
        if (calls++ >= 2000000 || boot_ticks >= 50000000) {
            fprintf(stderr, "TIMEOUT before FF50 calls=%u ticks=%llu pc=%04X\n",
                    calls, boot_ticks, GB_get_registers(gb)->pc);
            GB_free(gb);
            return 1;
        }
        memcpy(before, gb, sizeof *before);
        unsigned elapsed = GB_run(gb);
        boot_ticks += elapsed;
        if ((GB_read_memory(gb, 0xFF50) & 1) != 0) {
            saw_boot = true;
            printf("FF50_TRANSITION calls=%u boot_ticks=%llu run_ticks=%u\n",
                   calls, boot_ticks, elapsed);
            dump_state("BEFORE_FF50", before);
            dump_state("AFTER_FF50", gb);
            break;
        }
    }
    if (!saw_boot) { free(before); return 1; }

    unsigned long long after_ticks = 0;
    while (after_ticks < post_ticks) {
        after_ticks += GB_run(gb);
        if (after_ticks > post_ticks + 500000) break;
    }
    printf("POST_TICKS requested=%llu actual=%llu pc=%04X\n", post_ticks,
           after_ticks, GB_get_registers(gb)->pc);
    dump_state("AFTER_POST_TICKS", gb);
    /* GB_run advances the requested CPU time; one identical extra frame makes
     * the framebuffer comparable for both boot ROMs. */
    GB_run_frame(gb);
    printf("VISUAL_FRAME=1\n");
    char path[512];
    snprintf(path, sizeof path, "%s-%llu.ppm", argv[3], post_ticks);
    write_ppm(path);
    free(before);
    GB_free(gb);
    return 0;
}
