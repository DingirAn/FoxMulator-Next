#include <gtk/gtk.h>
#include <stdbool.h>
#include <stdio.h>
#include <string.h>
#include "gb.h"
#include "memory.h"

static GB_gameboy_t gb_storage;
static GB_gameboy_t *gb = &gb_storage;
static GtkWidget *area;
static GtkWidget *status;
static bool paused;
static bool boot_finished;
static unsigned screenshot_number;
static const char *boot_path;
static uint32_t pixels[160 * 144];
static bool cgb_preview;

static void clear_cartridge_header_area(void)
{
    if (!cgb_preview) return;
    for (unsigned y = 0; y < 48; y++) {
        for (unsigned x = 0; x < 160; x++) pixels[y * 160 + x] = 0xFFFFFFFFu;
    }
}

static void save_capture(void)
{
    char path[160];
    snprintf(path, sizeof path, "build/boot-preview/capture-%05u.png", screenshot_number++);
    cairo_surface_t *surface = cairo_image_surface_create_for_data(
        (unsigned char *) pixels, CAIRO_FORMAT_ARGB32, 160, 144, 160 * 4);
    cairo_surface_mark_dirty(surface);
    if (cairo_surface_write_to_png(surface, path) != CAIRO_STATUS_SUCCESS) {
        fprintf(stderr, "No se pudo guardar la captura: %s\n", path);
    }
    cairo_surface_destroy(surface);
}

static uint32_t rgb_encode(GB_gameboy_t *unused, uint8_t r, uint8_t g, uint8_t b)
{
    (void) unused;
    /* Cairo ARGB32 on little-endian hosts stores [B,G,R,A]. */
    return 0xFF000000u | ((uint32_t) r << 16) | ((uint32_t) g << 8) | b;
}

static void load_boot_rom(GB_gameboy_t *unused, GB_boot_rom_t type)
{
    (void) type;
    if (GB_load_boot_rom(unused, boot_path)) {
        fprintf(stderr, "No se pudo cargar la Boot ROM: %s\n", boot_path);
    }
}

static void vblank(GB_gameboy_t *unused, GB_vblank_type_t type)
{
    (void) unused;
    (void) type;
}

static gboolean tick(gpointer unused)
{
    (void) unused;
    if (!paused && !boot_finished) {
        GB_run_frame(gb);
        clear_cartridge_header_area();
        save_capture();
        gtk_widget_queue_draw(area);
    }
    if ((GB_read_memory(gb, 0xFF50) & 1) != 0) {
        boot_finished = true;
        paused = true;
    }
    char text[160];
    snprintf(text, sizeof text, "%s | FF50: %s | Espacio reanuda  S captura  Q salir",
             boot_finished ? "BOOT COMPLETO (pausado)" : (paused ? "PAUSADO" : "ARRANQUE"),
             boot_finished ? "desmapeada" : "mapeada");
    gtk_label_set_text(GTK_LABEL(status), text);
    return G_SOURCE_CONTINUE;
}

static gboolean draw(GtkWidget *widget, cairo_t *cr, gpointer unused)
{
    (void) widget; (void) unused;
    cairo_set_source_rgb(cr, 0.05, 0.05, 0.05);
    cairo_paint(cr);
    cairo_surface_t *surface = cairo_image_surface_create_for_data(
        (unsigned char *) pixels, CAIRO_FORMAT_ARGB32, 160, 144, 160 * 4);
    cairo_surface_mark_dirty(surface);
    cairo_scale(cr, 3.0, 3.0);
    cairo_set_source_surface(cr, surface, 0, 0);
    cairo_paint(cr);
    cairo_surface_destroy(surface);
    return FALSE;
}

static gboolean key(GtkWidget *widget, GdkEventKey *event, gpointer unused)
{
    (void) widget; (void) unused;
    if (event->keyval == GDK_KEY_space) paused = !paused;
    else if (event->keyval == GDK_KEY_q || event->keyval == GDK_KEY_Escape) gtk_main_quit();
    else if (event->keyval == GDK_KEY_s && paused) save_capture();
    return TRUE;
}

int main(int argc, char **argv)
{
    if (argc != 4) { fprintf(stderr, "usage: boot_preview [cgb|dmg] boot.bin cart.gb\n"); return 2; }
    bool cgb = strcmp(argv[1], "cgb") == 0;
    if (!cgb && strcmp(argv[1], "dmg") != 0) return 2;
    cgb_preview = cgb;
    GB_init(gb, cgb ? GB_MODEL_CGB_E : GB_MODEL_DMG_B);
    boot_path = argv[2];
    GB_set_boot_rom_load_callback(gb, load_boot_rom);
    GB_set_vblank_callback(gb, vblank);
    GB_set_pixels_output(gb, pixels);
    GB_set_rgb_encode_callback(gb, rgb_encode);
    if (GB_load_rom(gb, argv[3])) {
        fprintf(stderr, "No se pudo cargar la BIOS o ROM de prueba\n"); return 1;
    }
    gtk_init(&argc, &argv);
    GtkWidget *window = gtk_window_new(GTK_WINDOW_TOPLEVEL);
    gtk_window_set_title(GTK_WINDOW(window), cgb ? "FoxMulator Boot ROM — CGB" : "FoxMulator Boot ROM — DMG");
    gtk_window_set_default_size(GTK_WINDOW(window), 520, 500);
    GtkWidget *box = gtk_box_new(GTK_ORIENTATION_VERTICAL, 4);
    area = gtk_drawing_area_new();
    gtk_widget_set_size_request(area, 480, 432);
    status = gtk_label_new("ARRANQUE | FF50: mapeada");
    gtk_box_pack_start(GTK_BOX(box), area, TRUE, TRUE, 0);
    gtk_box_pack_start(GTK_BOX(box), status, FALSE, FALSE, 0);
    gtk_container_add(GTK_CONTAINER(window), box);
    g_signal_connect(window, "destroy", G_CALLBACK(gtk_main_quit), NULL);
    g_signal_connect(area, "draw", G_CALLBACK(draw), NULL);
    g_signal_connect(window, "key-press-event", G_CALLBACK(key), NULL);
    gtk_widget_set_can_focus(window, TRUE);
    gtk_widget_show_all(window);
    g_timeout_add(16, tick, NULL);
    gtk_main();
    GB_free(gb);
    return 0;
}
