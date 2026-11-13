//! FoxMulator desktop front end.
//!
//! With the `gui` feature this launches a Slint window, loads a ROM (from the
//! first command-line argument or an internal NOP loop), and steps the emulator
//! once per timer tick. Without the feature it runs a headless smoke test.

use clap::Parser;
use foxmulator_core::joypad::Button;
#[cfg(feature = "gui")]
use foxmulator_core::serial::SB_ADDR;
#[cfg(feature = "gui")]
use foxmulator_core::system::LinkCable;
use foxmulator_core::system::System;
#[cfg(feature = "gui")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "gui")]
use std::collections::{HashMap, HashSet};

#[cfg(feature = "gui")]
use slint::winit_030::{winit, WinitWindowAccessor};
#[cfg(feature = "gui")]
use slint::{Model, ModelRc, VecModel};
#[cfg(feature = "gui")]
use std::cell::RefCell;
#[cfg(all(feature = "gui", feature = "audio"))]
use std::cell::UnsafeCell;
#[cfg(all(feature = "gui", feature = "audio"))]
use std::io::Write;
#[cfg(feature = "gui")]
use std::rc::Rc;
#[cfg(all(feature = "gui", feature = "audio"))]
use std::sync::atomic::AtomicUsize;
#[cfg(feature = "gui")]
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
#[cfg(feature = "gui")]
use std::sync::{Arc, Mutex, MutexGuard};
#[cfg(feature = "gui")]
mod i18n;
mod rom_source;
mod save_slots;

use rom_source::RomSource;
#[cfg(feature = "gui")]
use rom_source::{ScanOutcome, ScanProgress};

#[cfg(feature = "gui")]
thread_local! {
    static ACTIVE_LANGUAGE: RefCell<Option<Rc<RefCell<i18n::LanguageManager>>>> = const { RefCell::new(None) };
}

#[cfg(feature = "gui")]
slint::include_modules!();

/// Parse a button name coming from the UI or keyboard.
#[allow(dead_code)]
fn button_from_name(name: &str) -> Option<Button> {
    match name {
        "a" => Some(Button::A),
        "b" => Some(Button::B),
        "select" => Some(Button::Select),
        "start" => Some(Button::Start),
        "left" => Some(Button::Left),
        "right" => Some(Button::Right),
        "up" => Some(Button::Up),
        "down" => Some(Button::Down),
        _ => None,
    }
}

#[cfg(feature = "gui")]
#[derive(Clone, Copy)]
enum InputSource {
    Keyboard,
    Virtual,
    Gamepad,
}

#[cfg(feature = "gui")]
impl InputSource {
    const fn index(self) -> usize {
        match self {
            Self::Keyboard => 0,
            Self::Virtual => 1,
            Self::Gamepad => 2,
        }
    }
}

#[cfg(feature = "gui")]
const fn button_index(button: Button) -> usize {
    match button {
        Button::Right => 0,
        Button::Left => 1,
        Button::Up => 2,
        Button::Down => 3,
        Button::A => 4,
        Button::B => 5,
        Button::Select => 6,
        Button::Start => 7,
    }
}

#[cfg(feature = "gui")]
const BUTTONS: [Button; 8] = [
    Button::Right,
    Button::Left,
    Button::Up,
    Button::Down,
    Button::A,
    Button::B,
    Button::Select,
    Button::Start,
];

/// Keeps each physical source independent. A release only reaches the core
/// when no keyboard, virtual or gamepad source still holds that button.
#[cfg(feature = "gui")]
#[derive(Default)]
struct InputState {
    held: [[[bool; 8]; 3]; 2],
}

#[cfg(feature = "gui")]
impl InputState {
    fn set(
        &mut self,
        console: usize,
        source: InputSource,
        button: Button,
        pressed: bool,
    ) -> Option<bool> {
        let console = console.checked_sub(1)?;
        let button_index = button_index(button);
        let before = self.held[console].iter().any(|source| source[button_index]);
        self.held[console][source.index()][button_index] = pressed;
        let after = self.held[console].iter().any(|source| source[button_index]);
        (before != after).then_some(after)
    }

    fn release_source(&mut self, console: usize, source: InputSource) -> Vec<Button> {
        let Some(console) = console.checked_sub(1) else {
            return Vec::new();
        };
        let mut released = Vec::new();
        for button in BUTTONS {
            let index = button_index(button);
            let before = self.held[console].iter().any(|held| held[index]);
            self.held[console][source.index()][index] = false;
            let after = self.held[console].iter().any(|held| held[index]);
            if before && !after {
                released.push(button);
            }
        }
        released
    }

    fn release_console(&mut self, console: usize) -> Vec<Button> {
        let Some(console) = console.checked_sub(1) else {
            return Vec::new();
        };
        let mut released = Vec::new();
        for button in BUTTONS {
            let index = button_index(button);
            if self.held[console].iter().any(|source| source[index]) {
                released.push(button);
            }
            for source in &mut self.held[console] {
                source[index] = false;
            }
        }
        released
    }
}

#[cfg(feature = "gui")]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct InputProfile {
    left: String,
    right: String,
    up: String,
    down: String,
    a: String,
    b: String,
    select: String,
    start: String,
    #[serde(default = "default_quick_save")]
    quick_save: String,
    #[serde(default = "default_quick_load")]
    quick_load: String,
    gamepad_slot: u8,
}

#[cfg(feature = "gui")]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct InputConfig {
    console1: InputProfile,
    console2: InputProfile,
    #[serde(default = "default_true")]
    show_virtual_controls: bool,
    #[serde(default)]
    lcd_grid: bool,
    #[serde(default)]
    second_console: bool,
    #[serde(default = "default_screen_scale")]
    screen_scale: u8,
    #[serde(default = "default_true")]
    auto_scale: bool,
    #[serde(default = "default_true")]
    translucent_shell: bool,
    #[serde(default = "default_translucent_tint")]
    translucent_tint: u8,
    #[serde(default = "default_shell_color")]
    shell_color: String,
    #[serde(default = "default_accent_color")]
    accent_color: String,
    #[serde(default = "default_volume")]
    master_volume: u8,
    #[serde(default = "default_volume")]
    console1_volume: u8,
    #[serde(default = "default_volume")]
    console2_volume: u8,
    #[serde(default)]
    console1_muted: bool,
    #[serde(default)]
    console2_muted: bool,
    #[serde(default = "default_language")]
    language: String,
    #[serde(default = "default_renderer")]
    renderer: String,
    #[serde(default = "default_boost_speed")]
    boost_speed: u32,
}

#[cfg(feature = "gui")]
fn default_true() -> bool {
    true
}

#[cfg(feature = "gui")]
fn default_screen_scale() -> u8 {
    2
}

#[cfg(feature = "gui")]
fn default_translucent_tint() -> u8 {
    60
}

#[cfg(feature = "gui")]
fn default_shell_color() -> String {
    "#9161b6".into()
}

#[cfg(feature = "gui")]
fn default_accent_color() -> String {
    "#d8b4ff".into()
}

#[cfg(feature = "gui")]
fn default_volume() -> u8 {
    100
}

#[cfg(feature = "gui")]
fn default_language() -> String {
    i18n::SPANISH_ID.into()
}

#[cfg(feature = "gui")]
fn default_renderer() -> String {
    "opengl".into()
}

#[cfg(feature = "gui")]
fn default_boost_speed() -> u32 {
    2
}

#[cfg(feature = "gui")]
const BOOST_SPEEDS: &[u32] = &[1, 2, 3, 4];

#[cfg(feature = "gui")]
fn valid_boost_speed(speed: i32) -> Option<u32> {
    BOOST_SPEEDS
        .iter()
        .copied()
        .find(|candidate| *candidate == speed as u32)
}

#[cfg(feature = "gui")]
fn rate_per_second(delta: u64, elapsed: std::time::Duration) -> f64 {
    delta as f64 / elapsed.as_secs_f64().max(0.001)
}

#[cfg(feature = "gui")]
fn cooperative_quantum() -> std::time::Duration {
    let micros = std::env::var("FOXMOULATOR_QUANTUM_US")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(100)
        .clamp(0, 5_000);
    std::time::Duration::from_micros(micros)
}

#[cfg(feature = "gui")]
fn default_quick_save() -> String {
    "F5".into()
}

#[cfg(feature = "gui")]
fn default_quick_load() -> String {
    "F6".into()
}

#[cfg(feature = "gui")]
fn default_console1_profile() -> InputProfile {
    InputProfile {
        left: "ArrowLeft".into(),
        right: "ArrowRight".into(),
        up: "ArrowUp".into(),
        down: "ArrowDown".into(),
        a: "x".into(),
        b: "z".into(),
        select: "Backspace".into(),
        start: "Enter".into(),
        quick_save: default_quick_save(),
        quick_load: default_quick_load(),
        gamepad_slot: 1,
    }
}

#[cfg(feature = "gui")]
impl Default for InputConfig {
    fn default() -> Self {
        Self {
            console1: default_console1_profile(),
            console2: InputProfile {
                left: "j".into(),
                right: "l".into(),
                up: "i".into(),
                down: "k".into(),
                a: "u".into(),
                b: "o".into(),
                select: "p".into(),
                start: ";".into(),
                quick_save: "F9".into(),
                quick_load: "F10".into(),
                gamepad_slot: 2,
            },
            show_virtual_controls: true,
            lcd_grid: false,
            second_console: false,
            screen_scale: 2,
            auto_scale: true,
            translucent_shell: true,
            translucent_tint: 60,
            shell_color: default_shell_color(),
            accent_color: default_accent_color(),
            master_volume: default_volume(),
            console1_volume: default_volume(),
            console2_volume: default_volume(),
            console1_muted: false,
            console2_muted: false,
            language: default_language(),
            renderer: default_renderer(),
            boost_speed: default_boost_speed(),
        }
    }
}

#[cfg(feature = "gui")]
fn input_config_path() -> std::path::PathBuf {
    directories::ProjectDirs::from("org", "FoxMulator", "FoxMulator")
        .map(|dirs| dirs.config_dir().join("controls.json"))
        .unwrap_or_else(|| std::path::PathBuf::from("foxmulator-controls.json"))
}

#[cfg(feature = "gui")]
fn normalize_key_identifier(raw: &str) -> Option<String> {
    let special = match raw {
        "\u{0008}" | "Backspace" => Some("Backspace"),
        "\u{000a}" | "Enter" => Some("Enter"),
        "\u{001b}" | "Escape" => Some("Escape"),
        "\u{F700}" | "ArrowUp" => Some("ArrowUp"),
        "\u{F701}" | "ArrowDown" => Some("ArrowDown"),
        "\u{F702}" | "ArrowLeft" => Some("ArrowLeft"),
        "\u{F703}" | "ArrowRight" => Some("ArrowRight"),
        "\u{F708}" | "F5" => Some("F5"),
        "\u{F709}" | "F6" => Some("F6"),
        "\u{F70C}" | "F9" => Some("F9"),
        "\u{F70D}" | "F10" => Some("F10"),
        _ => None,
    };
    if let Some(name) = special {
        return Some(name.to_owned());
    }
    let mut characters = raw.chars();
    let character = characters.next()?;
    if characters.next().is_some() || character.is_control() {
        return None;
    }
    Some(character.to_lowercase().collect())
}

#[cfg(feature = "gui")]
fn sanitize_binding(binding: &mut String, fallback: &str) -> bool {
    let Some(normalized) = normalize_key_identifier(binding) else {
        *binding = fallback.to_owned();
        return true;
    };
    if normalized == "Escape" {
        *binding = fallback.to_owned();
        return true;
    }
    if *binding != normalized {
        *binding = normalized;
        return true;
    }
    false
}

#[cfg(feature = "gui")]
fn sanitize_profile(profile: &mut InputProfile, defaults: &InputProfile) -> bool {
    let mut changed = false;
    changed |= sanitize_binding(&mut profile.left, &defaults.left);
    changed |= sanitize_binding(&mut profile.right, &defaults.right);
    changed |= sanitize_binding(&mut profile.up, &defaults.up);
    changed |= sanitize_binding(&mut profile.down, &defaults.down);
    changed |= sanitize_binding(&mut profile.a, &defaults.a);
    changed |= sanitize_binding(&mut profile.b, &defaults.b);
    changed |= sanitize_binding(&mut profile.select, &defaults.select);
    changed |= sanitize_binding(&mut profile.start, &defaults.start);
    changed |= sanitize_binding(&mut profile.quick_save, &defaults.quick_save);
    changed |= sanitize_binding(&mut profile.quick_load, &defaults.quick_load);
    changed
}

#[cfg(feature = "gui")]
fn sanitize_input_config(config: &mut InputConfig) -> bool {
    let defaults = InputConfig::default();
    let mut changed = sanitize_profile(&mut config.console1, &defaults.console1)
        | sanitize_profile(&mut config.console2, &defaults.console2);
    if !BOOST_SPEEDS.contains(&config.boost_speed) {
        config.boost_speed = 4;
        changed = true;
    }
    changed
}

#[cfg(feature = "gui")]
fn load_input_config() -> InputConfig {
    let mut config = std::fs::read(input_config_path())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    if sanitize_input_config(&mut config) {
        save_input_config(&config);
    }
    config
}

#[cfg(feature = "gui")]
fn save_input_config(config: &InputConfig) {
    let path = input_config_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(bytes) = serde_json::to_vec_pretty(config) {
        let _ = std::fs::write(path, bytes);
    }
}

#[cfg(feature = "gui")]
fn language_snapshot(
    manager: &i18n::LanguageManager,
) -> (HashMap<String, String>, Vec<slint::SharedString>, i32) {
    let strings = i18n::KEYS
        .iter()
        .map(|key| {
            let value = key
                .strip_prefix("slot_")
                .and_then(|number| number.parse::<u8>().ok())
                .map_or_else(
                    || manager.text(key),
                    |number| manager.format(key, &[("number", &number.to_string())]),
                );
            ((*key).to_owned(), value)
        })
        .collect::<HashMap<_, _>>();
    let languages = manager
        .languages()
        .iter()
        .map(|language| language.name.clone().into())
        .collect::<Vec<slint::SharedString>>();
    (strings, languages, manager.selected_index())
}

#[cfg(feature = "gui")]
fn apply_language(
    window: &AppWindow,
    snapshot: (HashMap<String, String>, Vec<slint::SharedString>, i32),
) {
    let (strings, languages, selected_index) = snapshot;
    let text = |key: &str| {
        strings
            .get(key)
            .cloned()
            .unwrap_or_else(|| format!("⟦{key}⟧"))
    };
    macro_rules! set_translation {
        ($setter:ident, $key:literal) => {
            window.$setter(text($key).into());
        };
    }
    set_translation!(set_app_no_cartridge, "app_no_cartridge");
    window.set_footer_tagline(text("footer_tagline").into());
    window.set_control_select(text("control_select").into());
    window.set_control_start(text("control_start").into());
    set_translation!(set_menu_settings, "menu_settings");
    set_translation!(set_menu_language, "menu_language");
    set_translation!(set_section_game_cartridges, "section_game_cartridges");
    set_translation!(set_section_save_states, "section_save_states");
    set_translation!(set_section_screen, "section_screen");
    set_translation!(set_section_appearance, "section_appearance");
    set_translation!(set_section_sound, "section_sound");
    set_translation!(set_section_game_link, "section_game_link");
    set_translation!(set_section_controls, "section_controls");
    set_translation!(set_section_advanced, "section_advanced");
    set_translation!(set_quick_save, "quick_save");
    set_translation!(set_quick_load, "quick_load");
    set_translation!(set_state_save, "save_state");
    set_translation!(set_state_load, "load_state");
    set_translation!(set_state_delete, "delete_state");
    set_translation!(set_state_load_file, "load_state_file");
    set_translation!(set_slot_0, "slot_0");
    set_translation!(set_slot_1, "slot_1");
    set_translation!(set_slot_2, "slot_2");
    set_translation!(set_slot_3, "slot_3");
    set_translation!(set_slot_4, "slot_4");
    set_translation!(set_slot_5, "slot_5");
    set_translation!(set_slot_6, "slot_6");
    set_translation!(set_slot_7, "slot_7");
    set_translation!(set_slot_8, "slot_8");
    set_translation!(set_slot_9, "slot_9");
    set_translation!(set_slot_10, "slot_10");
    set_translation!(set_slot_empty, "slot_empty");
    set_translation!(set_slot_occupied, "slot_occupied");
    set_translation!(set_console_one, "console_one");
    set_translation!(set_console_two, "console_two");
    set_translation!(set_console_load, "console_load");
    set_translation!(set_console_reset, "console_reset");
    set_translation!(set_console_eject, "console_eject");
    set_translation!(set_console_pause, "console_pause");
    set_translation!(set_screen_show_controls, "screen_show_controls");
    set_translation!(set_screen_show_second_console, "screen_show_second_console");
    set_translation!(set_screen_show_grid, "screen_show_grid");
    set_translation!(set_screen_auto_scale, "screen_auto_scale");
    set_translation!(set_screen_scale_label, "screen_scale");
    set_translation!(set_render_engine, "render_engine");
    set_translation!(set_render_opengl, "render_opengl");
    set_translation!(set_render_software, "render_software");
    set_translation!(set_render_restart_required, "render_restart_required");
    set_translation!(set_emulation_speed, "emulation_speed");
    set_translation!(set_emulation_speed_native, "emulation_speed_native");
    set_translation!(
        set_emulation_speed_default_boost,
        "emulation_speed_default_boost"
    );
    set_translation!(
        set_emulation_speed_boost_tooltip,
        "emulation_speed_boost_tooltip"
    );
    set_translation!(set_appearance_change_shell, "appearance_change_shell");
    set_translation!(set_appearance_library, "appearance_library");
    set_translation!(set_appearance_translucent, "appearance_translucent");
    window.set_display_tint_intensity(
        text("display_tint_intensity")
            .replace("{value}", &window.get_translucent_tint().to_string())
            .into(),
    );
    set_translation!(set_appearance_themes, "appearance_themes");
    set_translation!(set_appearance_custom_color, "appearance_custom_color");
    set_translation!(set_appearance_shell, "appearance_shell");
    set_translation!(set_appearance_accents, "appearance_accents");
    set_translation!(set_sound_general, "sound_general");
    set_translation!(set_sound_resume, "sound_resume");
    set_translation!(set_sound_mute, "sound_mute");
    set_translation!(set_sound_description, "sound_description");
    set_translation!(set_link_connected, "link_connected");
    set_translation!(set_link_disconnected, "link_disconnected");
    set_translation!(set_link_manual_clock, "link_manual_clock");
    set_translation!(set_controls_keys, "controls_keys");
    set_translation!(set_controls_quick_save, "controls_quick_save");
    set_translation!(set_controls_quick_load, "controls_quick_load");
    set_translation!(set_controls_instructions, "controls_instructions");
    set_translation!(set_controls_restore, "controls_restore");
    set_translation!(set_advanced_diagnostics, "advanced_diagnostics");
    set_translation!(set_library_title, "library_title");
    set_translation!(set_library_search, "library_search");
    set_translation!(set_library_scan, "library_scan");
    set_translation!(set_library_other_folder, "library_other_folder");
    set_translation!(set_library_games_label, "library_games");
    set_translation!(set_library_of, "library_of");
    set_translation!(set_library_empty, "library_empty");
    set_translation!(set_library_no_results, "library_no_results");
    set_translation!(set_library_internal, "library_internal");
    set_translation!(set_library_edit, "library_edit");
    set_translation!(set_library_name, "library_name");
    set_translation!(set_library_search_name_cover, "library_search_name_cover");
    set_translation!(set_library_change_cover, "library_change_cover");
    set_translation!(set_library_automatic, "library_automatic");
    set_translation!(set_library_results, "library_results");
    set_translation!(set_library_save, "library_save");
    set_translation!(set_library_cancel, "library_cancel");
    set_translation!(set_library_scan_searching, "library_scan_searching");
    set_translation!(set_library_scan_reading_file, "library_scan_reading_file");
    set_translation!(
        set_library_scan_inspecting_zip,
        "library_scan_inspecting_zip"
    );
    set_translation!(set_library_scan_file_counter, "library_scan_file_counter");
    set_translation!(set_library_scan_completed, "library_scan_completed");
    set_translation!(set_library_scan_cancel, "library_scan_cancel");
    set_translation!(set_library_scan_cancelled, "library_scan_cancelled");
    set_translation!(set_library_scan_error, "library_scan_error");
    set_translation!(set_state_manual_slots, "manual_slots");
    set_translation!(set_theme_atomic, "theme_atomic");
    set_translation!(set_theme_grape, "theme_grape");
    set_translation!(set_theme_ice, "theme_ice");
    set_translation!(set_theme_ocean, "theme_ocean");
    set_translation!(set_theme_cherry, "theme_cherry");
    set_translation!(set_theme_turquoise, "theme_turquoise");
    set_translation!(set_theme_solar, "theme_solar");
    set_translation!(set_theme_smoke, "theme_smoke");
    set_translation!(set_theme_crystal, "theme_crystal");
    set_translation!(set_theme_graphite, "theme_graphite");
    set_translation!(set_theme_lime, "theme_lime");
    window.set_language_options(ModelRc::new(VecModel::from(languages)));
    window.set_language_index(selected_index);
}

#[cfg(feature = "gui")]
fn active_text(key: &str) -> String {
    ACTIVE_LANGUAGE.with(|active| {
        active
            .borrow()
            .as_ref()
            .map_or_else(|| key.to_owned(), |manager| manager.borrow().text(key))
    })
}

#[cfg(feature = "gui")]
fn active_format(key: &str, params: &[(&str, &str)]) -> String {
    ACTIVE_LANGUAGE.with(|active| {
        active.borrow().as_ref().map_or_else(
            || key.to_owned(),
            |manager| manager.borrow().format(key, params),
        )
    })
}

#[cfg(feature = "gui")]
fn active_slot_label(slot: usize) -> String {
    active_text(&format!("slot_{slot}"))
}

#[cfg(feature = "gui")]
fn format_library_internal(internal: &str) -> String {
    active_format("library_internal", &[("value", internal)])
}

#[cfg(feature = "gui")]
fn refresh_display_tint(window: &AppWindow) {
    window.set_display_tint_intensity(
        active_format(
            "display_tint_intensity",
            &[("value", &window.get_translucent_tint().to_string())],
        )
        .into(),
    );
}

#[cfg(feature = "gui")]
fn bindings_text(profile: &InputProfile) -> String {
    active_format(
        "controls_binding_summary",
        &[
            ("left", &profile.left),
            ("right", &profile.right),
            ("up", &profile.up),
            ("down", &profile.down),
            ("a", &profile.a),
            ("b", &profile.b),
            ("select", &profile.select),
            ("start", &profile.start),
            ("quick_save", &profile.quick_save),
            ("quick_load", &profile.quick_load),
            ("gamepad", &profile.gamepad_slot.to_string()),
        ],
    )
}

#[cfg(feature = "gui")]
const FRAMEBUFFER_WIDTH: u32 = 160;

#[cfg(feature = "gui")]
const FRAMEBUFFER_HEIGHT: u32 = 144;

#[cfg(feature = "gui")]
const MINIMUM_SCALE: u32 = 2;

#[cfg(feature = "gui")]
const CONSOLE_COUNT_SINGLE: u8 = 1;

#[cfg(feature = "gui")]
const CONSOLE_COUNT_DUAL: u8 = 2;

#[cfg(feature = "gui")]
const LCD_HORIZONTAL_INSET: u32 = 24;

#[cfg(feature = "gui")]
const CONTROL_ROW_WIDTH: u32 = 324;

#[cfg(feature = "gui")]
const SHELL_HORIZONTAL_PADDING: u32 = 32;

#[cfg(feature = "gui")]
const WINDOW_HORIZONTAL_PADDING: u32 = 32;

#[cfg(feature = "gui")]
const CONSOLE_GAP: u32 = 16;

#[cfg(feature = "gui")]
const WINDOW_VERTICAL_OVERHEAD: u32 = 64;

#[cfg(feature = "gui")]
const MONITOR_SAFETY_PERCENT: u32 = 96;

#[cfg(feature = "gui")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GeometryReason {
    RequestedScaleReduced,
    MinimumGeometryDoesNotFit,
}

#[cfg(feature = "gui")]
#[derive(Clone, Copy, Debug)]
struct GeometryInput {
    requested_scale: u8,
    consoles: u8,
    controls_visible: bool,
    monitor_physical_width: u32,
    monitor_physical_height: u32,
    system_scale_factor: f64,
}

#[cfg(feature = "gui")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Geometry {
    requested_scale: u8,
    effective_scale: u8,
    console_width: u32,
    console_height: u32,
    minimum_console_width: u32,
    minimum_console_height: u32,
    window_width: u32,
    window_height: u32,
    minimum_window_width: u32,
    minimum_window_height: u32,
    available_width: u32,
    available_height: u32,
    reason: Option<GeometryReason>,
}

#[cfg(feature = "gui")]
fn shell_width(scale: u32) -> u32 {
    (FRAMEBUFFER_WIDTH * scale + LCD_HORIZONTAL_INSET)
        .max(CONTROL_ROW_WIDTH + SHELL_HORIZONTAL_PADDING)
}

#[cfg(feature = "gui")]
fn shell_height(scale: u32, controls_visible: bool) -> u32 {
    FRAMEBUFFER_HEIGHT * scale + if controls_visible { 334 } else { 163 }
}

#[cfg(feature = "gui")]
fn window_size(scale: u32, consoles: u8, controls_visible: bool) -> (u32, u32) {
    let count = u32::from(consoles.clamp(CONSOLE_COUNT_SINGLE, CONSOLE_COUNT_DUAL));
    let width = shell_width(scale) * count
        + CONSOLE_GAP * count.saturating_sub(1)
        + WINDOW_HORIZONTAL_PADDING;
    (
        width,
        shell_height(scale, controls_visible) + WINDOW_VERTICAL_OVERHEAD,
    )
}

#[cfg(feature = "gui")]
fn logical_monitor_area(
    physical_width: u32,
    physical_height: u32,
    scale_factor: f64,
) -> (u32, u32) {
    let factor = scale_factor.max(1.0);
    (
        ((f64::from(physical_width) * f64::from(MONITOR_SAFETY_PERCENT) / 100.0) / factor).floor()
            as u32,
        ((f64::from(physical_height) * f64::from(MONITOR_SAFETY_PERCENT) / 100.0) / factor).floor()
            as u32,
    )
}

#[cfg(feature = "gui")]
fn calculate_geometry(input: GeometryInput) -> Geometry {
    let requested = u32::from(input.requested_scale.clamp(1, 5));
    let (available_width, available_height) = logical_monitor_area(
        input.monitor_physical_width,
        input.monitor_physical_height,
        input.system_scale_factor,
    );
    let effective = (1..=requested)
        .rev()
        .find(|scale| {
            let size = window_size(*scale, input.consoles, input.controls_visible);
            size.0 <= available_width && size.1 <= available_height
        })
        .unwrap_or(1);
    let (window_width, window_height) =
        window_size(effective, input.consoles, input.controls_visible);
    let (minimum_window_width, minimum_window_height) =
        window_size(MINIMUM_SCALE, input.consoles, input.controls_visible);
    let reason = if effective < requested {
        Some(
            if window_size(1, input.consoles, input.controls_visible).0 > available_width
                || window_size(1, input.consoles, input.controls_visible).1 > available_height
            {
                GeometryReason::MinimumGeometryDoesNotFit
            } else {
                GeometryReason::RequestedScaleReduced
            },
        )
    } else {
        None
    };
    Geometry {
        requested_scale: requested as u8,
        effective_scale: effective as u8,
        console_width: shell_width(effective),
        console_height: shell_height(effective, input.controls_visible),
        minimum_console_width: shell_width(MINIMUM_SCALE),
        minimum_console_height: shell_height(MINIMUM_SCALE, input.controls_visible),
        window_width,
        window_height,
        minimum_window_width: minimum_window_width.min(available_width.max(1)),
        minimum_window_height: minimum_window_height.min(available_height.max(1)),
        available_width,
        available_height,
        reason,
    }
}

#[cfg(feature = "gui")]
#[derive(Default)]
struct LayoutRequest {
    pending: Option<(u32, u32)>,
    observation: Option<(u32, u32, u8)>,
    last_geometry: Option<(u32, u32, u32, u32, u8)>,
}

#[cfg(feature = "gui")]
fn apply_layout_size(
    window: &AppWindow,
    request: &Rc<RefCell<LayoutRequest>>,
    dual: bool,
    controls_visible: bool,
    resize: bool,
) {
    let (monitor_width, monitor_height, scale_factor) = window
        .window()
        .with_winit_window(|native_window| {
            let monitor = native_window.current_monitor();
            monitor.map_or((1920, 1080, native_window.scale_factor()), |monitor| {
                let size = monitor.size();
                (size.width, size.height, monitor.scale_factor())
            })
        })
        .unwrap_or((1920, 1080, 1.0));
    let geometry = calculate_geometry(GeometryInput {
        requested_scale: window.get_screen_scale().clamp(1, 5) as u8,
        consoles: if dual {
            CONSOLE_COUNT_DUAL
        } else {
            CONSOLE_COUNT_SINGLE
        },
        controls_visible,
        monitor_physical_width: monitor_width,
        monitor_physical_height: monitor_height,
        system_scale_factor: scale_factor,
    });
    window.set_effective_screen_scale(i32::from(geometry.effective_scale));
    window.set_layout_min_width(geometry.minimum_window_width as i32);
    window.set_layout_min_height(geometry.minimum_window_height as i32);
    if resize {
        let signature = (
            geometry.window_width,
            geometry.window_height,
            geometry.minimum_window_width,
            geometry.minimum_window_height,
            geometry.effective_scale,
        );
        let mut request = request.borrow_mut();
        if request.last_geometry != Some(signature) {
            request.pending = Some((geometry.window_width, geometry.window_height));
            request.last_geometry = Some(signature);
        }
    }
}

#[cfg(feature = "gui")]
fn select_renderer(configured: &str) -> Result<(), slint::PlatformError> {
    // An explicit environment choice is a diagnostic/user override and must
    // never be replaced by the persisted preference.
    if std::env::var_os("SLINT_BACKEND").is_some() {
        return slint::BackendSelector::new().select();
    }
    let renderer = match configured {
        "software" => "software",
        _ => "femtovg",
    };
    if slint::BackendSelector::new()
        .renderer_name(renderer.to_owned())
        .select()
        .is_ok()
    {
        return Ok(());
    }
    eprintln!("FoxMulator: renderer {renderer} unavailable; falling back to software");
    slint::BackendSelector::new()
        .renderer_name("software".to_owned())
        .select()
}

#[cfg(feature = "gui")]
fn apply_pending_native_layout(window: &AppWindow, request: &Rc<RefCell<LayoutRequest>>) {
    let trace = std::env::var_os("FOXMOULATOR_WINDOW_TRACE").is_some();
    if request.borrow().pending.is_none() {
        let observation = request.borrow().observation;
        if let Some((expected_width, expected_height, attempt)) = observation {
            window.window().with_winit_window(|native_window| {
                let actual = native_window.inner_size();
                if trace {
                    eprintln!(
                        "FoxMulator window resize observed: expected={}x{} logical, actual={}x{} physical, scale_factor={:.3}, attempt={attempt}",
                        expected_width,
                        expected_height,
                        actual.width,
                        actual.height,
                        native_window.scale_factor(),
                    );
                }
                if attempt >= 10
                    || (actual.width as f64
                        - f64::from(expected_width) * native_window.scale_factor())
                        .abs()
                        <= 1.0
                        && (actual.height as f64
                            - f64::from(expected_height) * native_window.scale_factor())
                            .abs()
                            <= 1.0
                {
                    request.borrow_mut().observation = None;
                } else {
                    request.borrow_mut().observation =
                        Some((expected_width, expected_height, attempt + 1));
                }
            });
        }
        return;
    }
    let Some((requested_width, requested_height)) = request.borrow().pending else {
        return;
    };
    let effective = window.window().with_winit_window(|native_window| {
        let before = native_window.inner_size();
        let was_maximized = native_window.is_maximized();
        let dual = window.get_second_console_enabled();
        let controls_visible = window.get_show_virtual_controls();
        let requested_scale = window.get_screen_scale().clamp(1, 5) as u32;
        let (monitor_width, monitor_height, scale_factor) = native_window
            .current_monitor()
            .map_or((requested_width, requested_height, native_window.scale_factor()), |monitor| {
                let size = monitor.size();
                (size.width, size.height, monitor.scale_factor())
            });
        let geometry = calculate_geometry(GeometryInput {
            requested_scale: requested_scale as u8,
            consoles: if dual { CONSOLE_COUNT_DUAL } else { CONSOLE_COUNT_SINGLE },
            controls_visible,
            monitor_physical_width: monitor_width,
            monitor_physical_height: monitor_height,
            system_scale_factor: scale_factor,
        });
        window.set_effective_screen_scale(i32::from(geometry.effective_scale));
        window.set_layout_min_width(geometry.minimum_window_width as i32);
        window.set_layout_min_height(geometry.minimum_window_height as i32);
        request.borrow_mut().last_geometry = Some((
            geometry.window_width,
            geometry.window_height,
            geometry.minimum_window_width,
            geometry.minimum_window_height,
            geometry.effective_scale,
        ));
        native_window.set_min_inner_size(Some(winit::dpi::LogicalSize::new(
            f64::from(geometry.minimum_window_width),
            f64::from(geometry.minimum_window_height),
        )));
        if trace {
            eprintln!(
                "FoxMulator window resize request: requested_scale={} requested={}x{} logical, effective_scale={} effective={}x{} logical, available={}x{} logical, before={}x{} physical, scale_factor={:.3}, maximized={}, reason={:?}",
                requested_scale,
                requested_width,
                requested_height,
                geometry.effective_scale,
                geometry.window_width,
                geometry.window_height,
                geometry.available_width,
                geometry.available_height,
                before.width,
                before.height,
                scale_factor,
                was_maximized,
                geometry.reason,
            );
        }
        if was_maximized {
            return (0, 0);
        }
        (geometry.window_width, geometry.window_height)
    });
    if let Some((width, height)) = effective.filter(|size| size.0 > 0 && size.1 > 0) {
        // Use Slint's window API as the single authoritative resize request.
        // The Winit backend converts these logical units exactly once for the
        // active monitor scale factor and reports the result asynchronously.
        window
            .window()
            .set_size(slint::LogicalSize::new(width as f32, height as f32));
        request.borrow_mut().pending = None;
        request.borrow_mut().observation = Some((width, height, 0));
    }
}

#[cfg(feature = "gui")]
fn apply_fullscreen(window: &AppWindow) {
    window.window().with_winit_window(|native_window| {
        native_window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(
            native_window.current_monitor(),
        )));
    });
}

#[cfg(feature = "gui")]
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct LibraryState {
    favorites: HashSet<String>,
    recent: Vec<String>,
    covers: HashMap<String, String>,
    #[serde(default)]
    commercial_names: HashMap<String, String>,
    #[serde(default)]
    manual_covers: HashSet<String>,
}

#[cfg(feature = "gui")]
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct UserPreferences {
    library_folder: Option<String>,
    last_rom_directory: Option<String>,
}

#[cfg(feature = "gui")]
fn library_state_path() -> std::path::PathBuf {
    directories::ProjectDirs::from("org", "FoxMulator", "FoxMulator")
        .map(|dirs| dirs.config_dir().join("library.json"))
        .unwrap_or_else(|| std::path::PathBuf::from("foxmulator-library.json"))
}

#[cfg(feature = "gui")]
fn load_library_state() -> LibraryState {
    std::fs::read(library_state_path())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

#[cfg(feature = "gui")]
fn save_library_state(state: &LibraryState) {
    let path = library_state_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(bytes) = serde_json::to_vec_pretty(state) {
        let _ = std::fs::write(path, bytes);
    }
}

#[cfg(feature = "gui")]
fn preferences_path() -> std::path::PathBuf {
    directories::ProjectDirs::from("org", "FoxMulator", "FoxMulator")
        .map(|dirs| dirs.config_dir().join("preferences.json"))
        .unwrap_or_else(|| std::path::PathBuf::from("foxmulator-preferences.json"))
}

#[cfg(feature = "gui")]
fn load_preferences() -> UserPreferences {
    std::fs::read(preferences_path())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

#[cfg(feature = "gui")]
fn save_preferences(preferences: &UserPreferences) {
    let path = preferences_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(bytes) = serde_json::to_vec_pretty(preferences) {
        let _ = std::fs::write(path, bytes);
    }
}

#[cfg(feature = "gui")]
fn recent_labels(state: &LibraryState) -> Vec<slint::SharedString> {
    state
        .recent
        .iter()
        .take(5)
        .filter_map(|id| rom_label_source(&RomSource::from_id(id)))
        .map(Into::into)
        .collect()
}

#[cfg(feature = "gui")]
fn fold_search_text(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|character| match character {
            'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' | 'õ' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            'ç' => 'c',
            other => other,
        })
        .collect()
}

#[cfg(feature = "gui")]
#[derive(Clone, Debug)]
struct LibrarySearchEntry {
    source: RomSource,
    commercial: String,
    internal: String,
    system: String,
    search_text: String,
}

#[cfg(feature = "gui")]
fn build_library_search_index(
    paths: &[RomSource],
    state: &LibraryState,
) -> Vec<LibrarySearchEntry> {
    paths
        .iter()
        .filter_map(|path| {
            let raw_label = rom_label_source(path)?;
            let commercial = commercial_name_source(path, state);
            let internal = library_internal_title_source(path);
            let filename = path.display_name();
            let search_text =
                fold_search_text(&format!("{raw_label} {commercial} {internal} {filename}"));
            let system = library_system_name_source(path);
            Some(LibrarySearchEntry {
                source: path.clone(),
                commercial,
                internal,
                system,
                search_text,
            })
        })
        .collect()
}

#[cfg(feature = "gui")]
fn visible_library_entries<'a>(
    index: &'a [LibrarySearchEntry],
    state: &LibraryState,
    query: &str,
) -> Vec<&'a LibrarySearchEntry> {
    let terms: Vec<String> = fold_search_text(query)
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    let mut result = index
        .iter()
        .filter(|entry| terms.iter().all(|term| entry.search_text.contains(term)))
        .collect::<Vec<_>>();
    result.sort_by_key(|entry| {
        let favorite = state.favorites.contains(&entry.source.id());
        !favorite
    });
    result
}

#[cfg(feature = "gui")]
fn library_key(source: &RomSource) -> String {
    rom_identity_source(source).unwrap_or_else(|| source.id())
}

#[cfg(feature = "gui")]
fn library_internal_title_source(source: &RomSource) -> String {
    source
        .load()
        .ok()
        .map(|bytes| rom_title(&bytes))
        .filter(|title| !title.is_empty())
        .or_else(|| Some(source.display_name()))
        .unwrap_or_else(|| "Unknown ROM".into())
}

#[cfg(feature = "gui")]
fn library_system_name_source(source: &RomSource) -> String {
    source
        .load()
        .map(|bytes| {
            if System::cgb_mode_for_rom(&bytes) {
                "Game Boy Color"
            } else {
                "Game Boy"
            }
            .into()
        })
        .unwrap_or_else(|_| "Unknown system".into())
}

#[cfg(feature = "gui")]
fn commercial_name_source(source: &RomSource, state: &LibraryState) -> String {
    let key = library_key(source);
    if let Some(name) = state.commercial_names.get(&key) {
        return name.clone();
    }
    let bytes = match source.load() {
        Ok(bytes) => bytes,
        Err(_) => return library_internal_title_source(source),
    };
    let color = System::cgb_mode_for_rom(&bytes);
    let database = load_rdb_index(color);
    let catalog = catalog_names_cached(color);
    let internal = library_internal_title_source(source);
    let filename = source.display_name();
    let candidates = vec![
        database
            .get(&rom_crc32(&bytes))
            .cloned()
            .unwrap_or_default(),
        internal,
        filename,
    ];
    catalog_match(&candidates, &catalog).unwrap_or_else(|| {
        candidates
            .into_iter()
            .find(|name| !name.is_empty())
            .unwrap_or_else(|| "Unknown game".into())
    })
}

#[cfg(feature = "gui")]
fn library_model_data(
    entries: &[&LibrarySearchEntry],
    state: &LibraryState,
) -> (
    Vec<slint::SharedString>,
    Vec<slint::SharedString>,
    Vec<slint::SharedString>,
    Vec<slint::SharedString>,
) {
    let ids = entries
        .iter()
        .map(|entry| entry.source.id().into())
        .collect();
    let commercial: Vec<slint::SharedString> = entries
        .iter()
        .map(|entry| entry.commercial.clone().into())
        .collect();
    let internal: Vec<slint::SharedString> = entries
        .iter()
        .map(|entry| entry.internal.clone().into())
        .collect();
    let systems: Vec<slint::SharedString> = entries
        .iter()
        .map(|entry| entry.system.clone().into())
        .collect();
    let labels: Vec<slint::SharedString> = commercial
        .iter()
        .zip(entries)
        .map(|(name, entry)| {
            if state.favorites.contains(&entry.source.id()) {
                format!("★ {name}")
            } else {
                name.to_string()
            }
        })
        .map(Into::into)
        .collect();
    (labels, internal, systems, ids)
}

#[cfg(feature = "gui")]
fn set_library_models(window: &AppWindow, index: &[LibrarySearchEntry], state: &LibraryState) {
    let visible = visible_library_entries(index, state, &window.get_library_query());
    let (labels, raw_internals, systems, ids) = library_model_data(&visible, state);
    let internals = raw_internals
        .into_iter()
        .map(|internal| format_library_internal(internal.as_str()).into())
        .collect::<Vec<slint::SharedString>>();
    let visible_paths: Vec<RomSource> = visible.iter().map(|entry| entry.source.clone()).collect();
    window.set_library_games(slint::ModelRc::new(slint::VecModel::from(labels.clone())));
    window.set_library_internals(slint::ModelRc::new(slint::VecModel::from(internals)));
    window.set_library_systems(slint::ModelRc::new(slint::VecModel::from(systems)));
    window.set_library_ids(slint::ModelRc::new(slint::VecModel::from(ids)));
    window.set_library_covers(slint::ModelRc::new(slint::VecModel::from(
        library_cover_images(&visible_paths, state, &labels),
    )));
    window.set_library_total_count(index.len() as i32);
    window.set_library_filtered_count(visible.len() as i32);
}

#[cfg(feature = "gui")]
fn cover_cache_dir() -> std::path::PathBuf {
    directories::ProjectDirs::from("org", "FoxMulator", "FoxMulator")
        .map(|dirs| dirs.cache_dir().join("covers"))
        .unwrap_or_else(|| std::path::PathBuf::from("foxmulator-covers"))
}

#[cfg(feature = "gui")]
fn cover_cache_path(source: &RomSource) -> std::path::PathBuf {
    let key = source.id();
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    let digest = format!("{:016x}", hasher.finish());
    cover_cache_dir().join(format!("{digest}.png"))
}

#[cfg(feature = "gui")]
fn database_cache_path(color: bool) -> std::path::PathBuf {
    cover_cache_dir().join("databases").join(if color {
        "Nintendo - Game Boy Color.rdb"
    } else {
        "Nintendo - Game Boy.rdb"
    })
}

#[cfg(feature = "gui")]
fn catalog_cache_path(color: bool) -> std::path::PathBuf {
    cover_cache_dir()
        .join("catalog")
        .join(if color { "cgb.txt" } else { "dmg.txt" })
}

#[cfg(feature = "gui")]
fn catalog_names(color: bool) -> Vec<String> {
    let cache = catalog_cache_path(color);
    let text = if let Ok(text) = std::fs::read_to_string(&cache) {
        text
    } else {
        let system = if color {
            "Nintendo%20-%20Game%20Boy%20Color"
        } else {
            "Nintendo%20-%20Game%20Boy"
        };
        let Ok(response) = reqwest::blocking::get(format!(
            "https://thumbnails.libretro.com/{system}/Named_Boxarts/"
        )) else {
            return Vec::new();
        };
        if !response.status().is_success() {
            return Vec::new();
        }
        let Ok(text) = response.text() else {
            return Vec::new();
        };
        if let Some(parent) = cache.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let temporary = cache.with_extension("part");
        if std::fs::write(&temporary, &text).is_ok() {
            let _ = std::fs::rename(temporary, &cache);
        }
        text
    };
    text.split("href=\"")
        .skip(1)
        .filter_map(|part| part.split('"').next())
        .filter(|name| name.ends_with(".png"))
        .map(|name| name.replace("%20", " "))
        .collect()
}

#[cfg(feature = "gui")]
fn catalog_names_cached(color: bool) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(catalog_cache_path(color)) else {
        return Vec::new();
    };
    text.split("href=\"")
        .skip(1)
        .filter_map(|part| part.split('"').next())
        .filter(|name| name.ends_with(".png"))
        .map(|name| name.replace("%20", " "))
        .collect()
}

#[cfg(feature = "gui")]
fn normalized_catalog_name(value: &str) -> String {
    value
        .trim_end_matches(".png")
        .trim_end_matches(".gbc")
        .trim_end_matches(".gb")
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_uppercase)
        .collect()
}

#[cfg(feature = "gui")]
fn short_catalog_name(value: &str) -> String {
    normalized_catalog_name(value.split('(').next().unwrap_or(value))
}

#[cfg(feature = "gui")]
fn catalog_match(candidates: &[String], catalog: &[String]) -> Option<String> {
    let mut matches = candidates
        .iter()
        .flat_map(|candidate| {
            let normalized = normalized_catalog_name(candidate);
            catalog
                .iter()
                .filter(move |entry| normalized_catalog_name(entry) == normalized)
        })
        .collect::<Vec<_>>();
    if matches.is_empty() {
        matches = candidates
            .iter()
            .flat_map(|candidate| {
                let normalized = short_catalog_name(candidate);
                catalog
                    .iter()
                    .filter(move |entry| short_catalog_name(entry) == normalized)
            })
            .collect();
    }
    matches.dedup();
    (matches.len() == 1).then(|| matches[0].trim_end_matches(".png").to_owned())
}

#[cfg(feature = "gui")]
fn load_rdb_index(color: bool) -> HashMap<u32, String> {
    let path = database_cache_path(color);
    std::fs::read(path)
        .map(|bytes| parse_rdb_crc_names(&bytes))
        .unwrap_or_default()
}

#[cfg(feature = "gui")]
fn download_rdb_index(color: bool) -> HashMap<u32, String> {
    let cache = database_cache_path(color);
    let cached = load_rdb_index(color);
    if !cached.is_empty() {
        return cached;
    }
    let name = if color {
        "Nintendo%20-%20Game%20Boy%20Color.rdb"
    } else {
        "Nintendo%20-%20Game%20Boy.rdb"
    };
    let Ok(client) = reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(4))
        .timeout(std::time::Duration::from_secs(12))
        .user_agent("FoxMulator Next")
        .build()
    else {
        return cached;
    };
    let url =
        format!("https://raw.githubusercontent.com/libretro/libretro-database/master/rdb/{name}");
    let Ok(response) = client.get(url).send() else {
        return cached;
    };
    if !response.status().is_success() {
        return cached;
    }
    let Ok(bytes) = response.bytes() else {
        return cached;
    };
    let bytes = bytes.to_vec();
    let index = parse_rdb_crc_names(&bytes);
    if index.is_empty() {
        return cached;
    }
    if let Some(parent) = cache.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let temporary = cache.with_extension("rdb.part");
    if std::fs::write(&temporary, &bytes).is_ok() && std::fs::rename(temporary, cache).is_ok() {
        index
    } else {
        cached
    }
}

#[cfg(feature = "gui")]
fn valid_png(bytes: &[u8]) -> bool {
    const SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 33 || &bytes[..8] != SIGNATURE || &bytes[12..16] != b"IHDR" {
        return false;
    }
    let mut offset = 8;
    while offset + 12 <= bytes.len() {
        let length = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap()) as usize;
        let end = offset.saturating_add(12).saturating_add(length);
        if end > bytes.len() {
            return false;
        }
        if &bytes[offset + 4..offset + 8] == b"IEND" {
            return end == bytes.len();
        }
        offset = end;
    }
    false
}

#[cfg(feature = "gui")]
fn cached_cover(source: &RomSource) -> Option<slint::Image> {
    let cache = cover_cache_path(source);
    let bytes = std::fs::read(&cache).ok()?;
    if !valid_png(&bytes) {
        let _ = std::fs::remove_file(cache);
        return None;
    }
    slint::Image::load_from_path(&cache).ok()
}

#[cfg(feature = "gui")]
fn url_component(value: &str) -> String {
    value.bytes().fold(String::new(), |mut out, byte| {
        if byte.is_ascii_alphanumeric() || b"-_.()".contains(&byte) {
            out.push(byte as char);
        } else if byte == b' ' {
            out.push_str("%20");
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
        out
    })
}

fn rom_crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn rom_identity_source(source: &RomSource) -> Option<String> {
    let bytes = source.load().ok()?;
    if bytes.len() < 0x150 {
        return None;
    }
    let system = if System::cgb_mode_for_rom(&bytes) {
        "cgb"
    } else {
        "dmg"
    };
    Some(format!("{system}:{:08X}", rom_crc32(&bytes)))
}

#[cfg(feature = "gui")]
fn rdb_string(data: &[u8], position: &mut usize) -> Option<String> {
    let kind = *data.get(*position)?;
    *position += 1;
    let length = match kind {
        0xa0..=0xbf => usize::from(kind - 0xa0),
        0xd9 => {
            let length = usize::from(*data.get(*position)?);
            *position += 1;
            length
        }
        0xda => {
            let value = u16::from_be_bytes(data.get(*position..*position + 2)?.try_into().ok()?);
            *position += 2;
            usize::from(value)
        }
        0xdb => {
            let value = u32::from_be_bytes(data.get(*position..*position + 4)?.try_into().ok()?);
            *position += 4;
            usize::try_from(value).ok()?
        }
        _ => return None,
    };
    let bytes = data.get(*position..*position + length)?;
    *position += length;
    String::from_utf8(bytes.to_vec()).ok()
}

#[cfg(feature = "gui")]
fn rdb_skip(data: &[u8], position: &mut usize) -> Option<()> {
    let kind = *data.get(*position)?;
    *position += 1;
    let bytes = match kind {
        0x00..=0x7f | 0xe0..=0xff | 0xc0 | 0xc2 | 0xc3 => (0, 0),
        0xa0..=0xbf => (0, usize::from(kind - 0xa0)),
        0xd9 | 0xc4 => {
            let n = usize::from(*data.get(*position)?);
            *position += 1;
            (0, n)
        }
        0xda | 0xc5 => {
            let n = usize::from(u16::from_be_bytes(
                data.get(*position..*position + 2)?.try_into().ok()?,
            ));
            *position += 2;
            (0, n)
        }
        0xdb | 0xc6 => {
            let n = usize::try_from(u32::from_be_bytes(
                data.get(*position..*position + 4)?.try_into().ok()?,
            ))
            .ok()?;
            *position += 4;
            (0, n)
        }
        0xcc | 0xd0 => (0, 1),
        0xcd | 0xd1 => (0, 2),
        0xce | 0xd2 | 0xca => (0, 4),
        0xcf | 0xd3 | 0xcb => (0, 8),
        0x80..=0x8f => {
            let mut n = usize::from(kind - 0x80) * 2;
            while n > 0 {
                rdb_skip(data, position)?;
                n -= 1;
            }
            return Some(());
        }
        0xde => {
            let n = usize::from(u16::from_be_bytes(
                data.get(*position..*position + 2)?.try_into().ok()?,
            ));
            *position += 2;
            for _ in 0..n * 2 {
                rdb_skip(data, position)?;
            }
            return Some(());
        }
        0xdf => {
            let n = usize::try_from(u32::from_be_bytes(
                data.get(*position..*position + 4)?.try_into().ok()?,
            ))
            .ok()?;
            *position += 4;
            for _ in 0..n * 2 {
                rdb_skip(data, position)?;
            }
            return Some(());
        }
        0x90..=0x9f => {
            for _ in 0..usize::from(kind - 0x90) {
                rdb_skip(data, position)?;
            }
            return Some(());
        }
        0xdc => {
            let n = usize::from(u16::from_be_bytes(
                data.get(*position..*position + 2)?.try_into().ok()?,
            ));
            *position += 2;
            for _ in 0..n {
                rdb_skip(data, position)?;
            }
            return Some(());
        }
        0xdd => {
            let n = usize::try_from(u32::from_be_bytes(
                data.get(*position..*position + 4)?.try_into().ok()?,
            ))
            .ok()?;
            *position += 4;
            for _ in 0..n {
                rdb_skip(data, position)?;
            }
            return Some(());
        }
        _ => return None,
    };
    *position += bytes.1;
    Some(())
}

#[cfg(feature = "gui")]
fn parse_rdb_crc_names(data: &[u8]) -> HashMap<u32, String> {
    let mut result = HashMap::new();
    let mut position = 16usize;
    while position < data.len() {
        if data.get(position) == Some(&0xc0) {
            break;
        }
        let kind = match data.get(position) {
            Some(kind) => *kind,
            None => break,
        };
        let count = match kind {
            0x80..=0x8f => {
                position += 1;
                usize::from(kind - 0x80)
            }
            0xde => {
                position += 1;
                let n = u16::from_be_bytes(match data.get(position..position + 2) {
                    Some(v) => v.try_into().ok().unwrap(),
                    None => break,
                });
                position += 2;
                usize::from(n)
            }
            0xdf => {
                position += 1;
                let n = u32::from_be_bytes(match data.get(position..position + 4) {
                    Some(v) => v.try_into().ok().unwrap(),
                    None => break,
                });
                position += 4;
                usize::try_from(n).unwrap_or(0)
            }
            _ => break,
        };
        let mut name = None;
        let mut crc = None;
        for _ in 0..count {
            let key = match rdb_string(data, &mut position) {
                Some(key) => key,
                None => break,
            };
            if key == "name" || key == "rom_name" {
                name = rdb_string(data, &mut position);
            } else if key == "crc" {
                let kind = match data.get(position) {
                    Some(kind) => *kind,
                    None => break,
                };
                position += 1;
                let length = match kind {
                    0xc4 => usize::from(data.get(position).copied().unwrap_or(0)),
                    0xc5 => {
                        usize::from(u16::from_be_bytes(match data.get(position..position + 2) {
                            Some(v) => v.try_into().ok().unwrap(),
                            None => break,
                        }))
                    }
                    0xc6 => usize::try_from(u32::from_be_bytes(
                        match data.get(position..position + 4) {
                            Some(v) => v.try_into().ok().unwrap(),
                            None => break,
                        },
                    ))
                    .unwrap_or(0),
                    _ => 0,
                };
                if length == 0 {
                    break;
                }
                let header = match kind {
                    0xc4 => 1,
                    0xc5 => 2,
                    0xc6 => 4,
                    _ => 0,
                };
                position += header;
                let bytes = match data.get(position..position + length) {
                    Some(v) => v,
                    None => break,
                };
                if length == 4 {
                    crc = Some(u32::from_be_bytes(bytes.try_into().unwrap()));
                }
                position += length;
            } else if rdb_skip(data, &mut position).is_none() {
                break;
            }
        }
        if let (Some(crc), Some(name)) = (crc, name) {
            result.insert(crc, name);
        }
    }
    result
}

#[cfg(feature = "gui")]
fn download_cover(
    source: &RomSource,
    database: &HashMap<u32, String>,
    catalog: &[String],
) -> Option<std::path::PathBuf> {
    let cache = cover_cache_path(source);
    if std::fs::read(&cache).is_ok_and(|bytes| valid_png(&bytes)) {
        return Some(cache);
    }
    let _ = std::fs::remove_file(&cache);
    let rom = source.load().ok()?;
    if rom.len() < 0x150 {
        return None;
    }
    let title = rom_title(&rom);
    if title.is_empty() {
        return None;
    }
    let system = if System::cgb_mode_for_rom(&rom) {
        "Nintendo%20-%20Game%20Boy%20Color"
    } else {
        "Nintendo%20-%20Game%20Boy"
    };
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(4))
        .timeout(std::time::Duration::from_secs(8))
        .user_agent("FoxMulator Next")
        .build()
        .ok()?;
    let mut candidates = database
        .get(&rom_crc32(&rom))
        .cloned()
        .into_iter()
        .chain(std::iter::once(title))
        .collect::<Vec<_>>();
    if let RomSource::File(path) = source {
        if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
            if !candidates.iter().any(|candidate| candidate == stem) {
                candidates.push(stem.to_owned());
            }
        }
    } else if let Some(stem) = source
        .physical_path()
        .file_stem()
        .and_then(|stem| stem.to_str())
    {
        if !candidates.iter().any(|candidate| candidate == stem) {
            candidates.push(stem.to_owned());
        }
    }
    let catalog_name = catalog_match(&candidates, catalog)?;
    let bytes = std::iter::once(catalog_name).find_map(|candidate| {
        let url = format!(
            "https://thumbnails.libretro.com/{system}/Named_Boxarts/{}.png",
            url_component(&candidate)
        );
        let response = client.get(url).send().ok()?;
        if response.status().is_success() {
            response.bytes().ok()
        } else {
            None
        }
    })?;
    if !valid_png(&bytes) {
        return None;
    }
    std::fs::create_dir_all(cover_cache_dir()).ok()?;
    let temporary = cache.with_extension("png.part");
    std::fs::write(&temporary, &bytes).ok()?;
    if !valid_png(&std::fs::read(&temporary).ok()?) {
        let _ = std::fs::remove_file(&temporary);
        return None;
    }
    std::fs::rename(&temporary, &cache).ok()?;
    Some(cache)
}

#[cfg(feature = "gui")]
fn download_named_cover(source: &RomSource, catalog_name: &str) -> Option<std::path::PathBuf> {
    let rom = source.load().ok()?;
    let color = System::cgb_mode_for_rom(&rom);
    let system = if color {
        "Nintendo%20-%20Game%20Boy%20Color"
    } else {
        "Nintendo%20-%20Game%20Boy"
    };
    let response = reqwest::blocking::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(4))
        .timeout(std::time::Duration::from_secs(8))
        .user_agent("FoxMulator Next")
        .build()
        .ok()?
        .get(format!(
            "https://thumbnails.libretro.com/{system}/Named_Boxarts/{}.png",
            url_component(catalog_name)
        ))
        .send()
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let bytes = response.bytes().ok()?.to_vec();
    if !valid_png(&bytes) {
        return None;
    }
    let cache = cover_cache_path(source);
    std::fs::create_dir_all(cover_cache_dir()).ok()?;
    let temporary = cache.with_extension("png.part");
    std::fs::write(&temporary, &bytes).ok()?;
    std::fs::rename(temporary, cache.clone()).ok()?;
    Some(cache)
}

#[cfg(feature = "gui")]
fn library_cover_images(
    sources: &[RomSource],
    state: &LibraryState,
    labels: &[slint::SharedString],
) -> Vec<slint::Image> {
    labels
        .iter()
        .enumerate()
        .map(|(index, _)| {
            sources
                .get(index)
                .and_then(|source| {
                    let identity = rom_identity_source(source)?;
                    if let Some(manual) = state
                        .covers
                        .get(&identity)
                        .or_else(|| state.covers.get(&source.id()))
                    {
                        let manual_path = std::path::Path::new(manual);
                        return manual_path
                            .is_file()
                            .then(|| slint::Image::load_from_path(manual_path).ok())
                            .flatten();
                    }
                    cached_cover(source)
                })
                .unwrap_or_default()
        })
        .collect()
}

#[cfg(feature = "gui")]
fn start_cover_downloads(sources: Vec<RomSource>, window: slint::Weak<AppWindow>) {
    std::thread::spawn(move || {
        let dmg_database = download_rdb_index(false);
        let cgb_database = download_rdb_index(true);
        let dmg_catalog = catalog_names(false);
        let cgb_catalog = catalog_names(true);
        for source in sources {
            let rom = source.load().ok();
            let database = rom.as_deref().map(|bytes| {
                if System::cgb_mode_for_rom(bytes) {
                    &cgb_database
                } else {
                    &dmg_database
                }
            });
            let catalog = rom.as_deref().map(|bytes| {
                if System::cgb_mode_for_rom(bytes) {
                    &cgb_catalog
                } else {
                    &dmg_catalog
                }
            });
            let _ = database
                .zip(catalog)
                .and_then(|(database, catalog)| download_cover(&source, database, catalog));
            let refresh_source = source.clone();
            let refresh_window = window.clone();
            let _ = slint::invoke_from_event_loop(move || {
                let Some(window) = refresh_window.upgrade() else {
                    return;
                };
                let ids = window.get_library_ids();
                let refresh_id = refresh_source.id();
                if ids.iter().any(|id| id == refresh_id.as_str()) {
                    let mut images = window.get_library_covers().iter().collect::<Vec<_>>();
                    let label_index = ids.iter().position(|id| id == refresh_id.as_str());
                    if let Some(index) = label_index {
                        if let Some(image) = cached_cover(&refresh_source) {
                            if index >= images.len() {
                                images.resize(index + 1, slint::Image::default());
                            }
                            images[index] = image;
                            window.set_library_covers(slint::ModelRc::new(slint::VecModel::from(
                                images,
                            )));
                        }
                    }
                }
            });
        }
    });
}

#[cfg(feature = "gui")]
fn profile_for_key(config: &InputConfig, key: &str) -> Option<(usize, Button)> {
    let key = normalize_key_identifier(key)?;
    for (console, profile) in [(1, &config.console1), (2, &config.console2)] {
        let button = if key == profile.left {
            Button::Left
        } else if key == profile.right {
            Button::Right
        } else if key == profile.up {
            Button::Up
        } else if key == profile.down {
            Button::Down
        } else if key == profile.a {
            Button::A
        } else if key == profile.b {
            Button::B
        } else if key == profile.select {
            Button::Select
        } else if key == profile.start {
            Button::Start
        } else {
            continue;
        };
        return Some((console, button));
    }
    None
}

#[cfg(feature = "gui")]
fn quick_action_for_key(config: &InputConfig, key: &str) -> Option<(usize, bool)> {
    let key = normalize_key_identifier(key)?;
    for (console, profile) in [(1, &config.console1), (2, &config.console2)] {
        if key == profile.quick_save {
            return Some((console, true));
        }
        if key == profile.quick_load {
            return Some((console, false));
        }
    }
    None
}

#[cfg(feature = "gui")]
fn binding_owner(config: &InputConfig, key: &str) -> Option<(usize, &'static str)> {
    for (console, profile) in [(1, &config.console1), (2, &config.console2)] {
        for (action, binding) in [
            ("left", &profile.left),
            ("right", &profile.right),
            ("up", &profile.up),
            ("down", &profile.down),
            ("a", &profile.a),
            ("b", &profile.b),
            ("select", &profile.select),
            ("start", &profile.start),
            ("quick-save", &profile.quick_save),
            ("quick-load", &profile.quick_load),
        ] {
            if binding == key {
                return Some((console, action));
            }
        }
    }
    None
}

#[cfg(feature = "gui")]
fn rebind_conflict(
    config: &InputConfig,
    console: usize,
    action: &str,
    key: &str,
) -> Option<(usize, &'static str)> {
    binding_owner(config, key).filter(|owner| *owner != (console, action))
}

#[cfg(feature = "gui")]
#[derive(Debug, PartialEq, Eq)]
enum RebindError {
    Conflict(usize, &'static str),
    InvalidAction,
}

#[cfg(feature = "gui")]
fn apply_rebind(
    config: &mut InputConfig,
    console: usize,
    action: &str,
    key: &str,
) -> Result<(), RebindError> {
    let key = normalize_key_identifier(key).ok_or(RebindError::InvalidAction)?;
    if key == "Escape" {
        return Err(RebindError::InvalidAction);
    }
    if let Some((used_console, used_action)) = rebind_conflict(config, console, action, &key) {
        return Err(RebindError::Conflict(used_console, used_action));
    }
    let profile = match console {
        1 => &mut config.console1,
        2 => &mut config.console2,
        _ => return Err(RebindError::InvalidAction),
    };
    match action {
        "left" => profile.left = key,
        "right" => profile.right = key,
        "up" => profile.up = key,
        "down" => profile.down = key,
        "a" => profile.a = key,
        "b" => profile.b = key,
        "select" => profile.select = key,
        "start" => profile.start = key,
        "quick-save" => profile.quick_save = key,
        "quick-load" => profile.quick_load = key,
        _ => return Err(RebindError::InvalidAction),
    }
    Ok(())
}

#[cfg(all(test, feature = "gui"))]
mod input_tests {
    use super::{
        format_library_internal, normalize_key_identifier, quick_action_for_key, Button,
        InputSource, InputState, RomSource, ACTIVE_LANGUAGE,
    };
    use clap::Parser;
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn boost_speeds_are_numeric_and_bounded() {
        for speed in [1, 2, 3, 4] {
            assert_eq!(super::valid_boost_speed(speed), Some(speed as u32));
        }
        for speed in [0, 5, 6, 7, 8, 9, 10, 11, 12, 13, 20] {
            assert_eq!(super::valid_boost_speed(speed), None);
        }
    }

    #[test]
    fn legacy_boost_speed_is_migrated_to_four() {
        let mut config = InputConfig::default();
        config.boost_speed = 12;
        assert!(super::sanitize_input_config(&mut config));
        assert_eq!(config.boost_speed, 4);
        assert!(!super::sanitize_input_config(&mut config));
    }

    #[test]
    fn rates_use_interval_deltas_not_process_totals() {
        let rate = super::rate_per_second(60, std::time::Duration::from_secs(1));
        assert!((rate - 60.0).abs() < f64::EPSILON);
        let rate = super::rate_per_second(120, std::time::Duration::from_secs(2));
        assert!((rate - 60.0).abs() < f64::EPSILON);
    }

    #[test]
    #[ignore = "explicit performance benchmark"]
    fn worker_throughput_benchmark() {
        let rom_path =
            std::env::var("FOXMOULATOR_BENCH_ROM").unwrap_or_else(|_| "test-roms/sample.gb".into());
        let rom = std::fs::read(rom_path).expect("benchmark ROM");
        let systems = super::SharedSystems::new(super::DualSystems {
            left: foxmulator_core::System::from_rom(rom),
            right: foxmulator_core::System::new(false),
            cable: foxmulator_core::LinkCable::new(),
            left_loaded: true,
            right_loaded: false,
            right_visible: false,
            link_enabled: false,
            left_cart: "benchmark".into(),
            right_cart: "No cartridge".into(),
        });
        let pixels = super::FrameBuffers::white();
        let frames = std::sync::Arc::new(std::sync::Mutex::new(super::FrameBuffers::new(
            pixels.clone(),
            pixels,
        )));
        let control = std::sync::Arc::new(super::FrameControl::new());
        let stats = std::sync::Arc::new(super::WorkerStats::new());
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let speed = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(12));
        let worker_systems = systems.clone();
        let worker_frames = std::sync::Arc::clone(&frames);
        let worker_control = std::sync::Arc::clone(&control);
        let worker_stats = std::sync::Arc::clone(&stats);
        let worker_stop = std::sync::Arc::clone(&stop);
        let worker_speed = std::sync::Arc::clone(&speed);
        let worker = std::thread::spawn(move || {
            super::run_emulation_worker(
                worker_systems,
                worker_frames,
                worker_control,
                worker_stats,
                worker_stop,
                worker_speed,
                None,
                #[cfg(feature = "audio")]
                std::sync::Arc::new(std::sync::Mutex::new(super::AudioMixSettings::from(
                    &super::InputConfig::default(),
                ))),
            );
        });
        let started = std::time::Instant::now();
        std::thread::sleep(std::time::Duration::from_secs(2));
        stop.store(true, std::sync::atomic::Ordering::Release);
        worker.join().expect("benchmark worker exits");
        let seconds = started.elapsed().as_secs_f64();
        eprintln!(
            "worker benchmark: frames={:.1}/s cycles={:.1}/s emulation_ms={:.1}/s quantum_ms={:.1}/s quantum_calls={:.1}/s",
            stats.completed_frames.load(std::sync::atomic::Ordering::Relaxed) as f64 / seconds,
            stats.cycles.load(std::sync::atomic::Ordering::Relaxed) as f64 / seconds,
            stats.emulation_us.load(std::sync::atomic::Ordering::Relaxed) as f64 / seconds / 1000.0,
            stats.quantum_us.load(std::sync::atomic::Ordering::Relaxed) as f64 / seconds / 1000.0,
            stats.quantum_calls.load(std::sync::atomic::Ordering::Relaxed) as f64 / seconds,
        );
    }

    #[test]
    fn library_model_text_formats_internal_title_once() {
        let directory = std::env::temp_dir().join(format!(
            "foxmulator-library-i18n-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut manager = super::i18n::LanguageManager::from_directory(directory.clone());
        manager.select_id(super::i18n::ENGLISH_ID);
        ACTIVE_LANGUAGE.with(|active| {
            *active.borrow_mut() = Some(Rc::new(RefCell::new(manager)));
        });
        assert_eq!(
            format_library_internal("PM_CRYSTAL"),
            "Internal: PM_CRYSTAL"
        );
        assert!(!format_library_internal("PM_CRYSTAL").contains("{value}"));
        let slots = super::save_state_slot_labels(None, None);
        assert_eq!(slots[0], "Slot 0 · Quick Save");
        for slot in 1..=10 {
            assert_eq!(slots[slot], format!("Slot {slot}"));
        }
        ACTIVE_LANGUAGE.with(|active| *active.borrow_mut() = None);
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn virtual_press_remains_held_until_its_release() {
        let mut input = InputState::default();
        assert_eq!(
            input.set(1, InputSource::Virtual, Button::Right, true),
            Some(true)
        );
        for _ in 0..180 {
            assert_eq!(
                input.set(1, InputSource::Virtual, Button::Right, true),
                None
            );
        }
        assert_eq!(
            input.set(1, InputSource::Virtual, Button::Right, false),
            Some(false)
        );
    }

    #[test]
    fn slot_occupancy_refresh_is_independent_of_selection() {
        let root =
            std::env::temp_dir().join(format!("foxmulator-slot-occupancy-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&root);
        let battery = root.join("game.cgb_12345678.sav");
        let identity = "cgb:12345678";
        let empty = super::save_state_slot_occupancy(Some(&battery), Some(identity));
        assert_eq!(empty, vec![false; super::save_slots::SLOT_COUNT]);

        let system = foxmulator_core::System::from_rom(vec![0; 0x8000]);
        super::save_slots::save_slot_at(&system, &battery, identity, "Test", 5)
            .expect("save slot 5");
        let one = super::save_state_slot_occupancy(Some(&battery), Some(identity));
        assert!(one[5]);
        assert_eq!(one.iter().filter(|occupied| **occupied).count(), 1);

        for slot in 0..super::save_slots::SLOT_COUNT {
            super::save_slots::save_slot_at(&system, &battery, identity, "Test", slot)
                .expect("save slot");
        }
        assert_eq!(
            super::save_state_slot_occupancy(Some(&battery), Some(identity)),
            vec![true; super::save_slots::SLOT_COUNT]
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn reset_frame_generation_requires_a_complete_current_frame() {
        let old = vec![foxmulator_core::Pixel(0xff11_2233); super::FrameBuffers::PIXELS];
        let mut frames = super::FrameBuffers::new(old.clone(), old.clone());
        let control = super::FrameControl::new();
        let generation = control.begin_reset(1);
        frames.reset_console(1, generation);
        assert!(frames.left.iter().all(|pixel| pixel.0 == 0xffff_ffff));
        assert_eq!(frames.right, old);
        assert!(!frames.publish_if_current(
            1,
            generation.saturating_sub(1),
            Some(vec![
                foxmulator_core::Pixel(0xff44_5566);
                super::FrameBuffers::PIXELS
            ])
        ));
        assert!(!frames.publish_if_current(1, generation, None));
        let current = vec![foxmulator_core::Pixel(0xff77_8899); super::FrameBuffers::PIXELS];
        assert!(frames.publish_if_current(1, generation, Some(current.clone())));
        control.finish_reset_if_current(1, generation);
        assert_eq!(frames.left, current);

        let next_generation = control.begin_reset(1);
        frames.reset_console(1, next_generation);
        assert!(!frames.publish_if_current(1, generation, Some(old)));
    }

    #[test]
    fn library_search_matches_display_filename_and_accented_metadata() {
        let state = super::LibraryState::default();
        let entries = vec![
            super::LibrarySearchEntry {
                source: RomSource::File(std::path::PathBuf::from("red.gb")),
                commercial: "Red Title".into(),
                internal: "RED TITLE".into(),
                system: "Game Boy".into(),
                search_text: super::fold_search_text("Red Title RED TITLE red.gb"),
            },
            super::LibrarySearchEntry {
                source: RomSource::File(std::path::PathBuf::from("crystal.gbc")),
                commercial: "Crystal Title".into(),
                internal: "CRYSTAL TITLE".into(),
                system: "Game Boy Color".into(),
                search_text: super::fold_search_text("Crystal Title CRYSTAL TITLE crystal.gbc"),
            },
            super::LibrarySearchEntry {
                source: RomSource::File(std::path::PathBuf::from("sample.gb")),
                commercial: "Sample Title".into(),
                internal: "SAMPLE TITLE".into(),
                system: "Game Boy".into(),
                search_text: super::fold_search_text("Sample Title SAMPLE TITLE sample.gb"),
            },
        ];
        let paths = |query: &str| {
            super::visible_library_entries(&entries, &state, query)
                .into_iter()
                .map(|entry| entry.source.id())
                .collect::<Vec<_>>()
        };
        assert_eq!(paths("title"), vec!["red.gb", "crystal.gbc", "sample.gb"]);
        assert_eq!(paths("crystal title"), vec!["crystal.gbc"]);
        assert_eq!(paths("sample"), vec!["sample.gb"]);
        assert_eq!(paths("crystal.gbc"), vec!["crystal.gbc"]);
        assert_eq!(paths("filename.gbc"), Vec::<String>::new());
        assert_eq!(paths(""), vec!["red.gb", "crystal.gbc", "sample.gb"]);
    }

    #[test]
    fn library_search_preserves_metadata_and_order_after_filtering() {
        let state = super::LibraryState::default();
        let entries = vec![
            super::LibrarySearchEntry {
                source: RomSource::File(std::path::PathBuf::from("first.gb")),
                commercial: "First".into(),
                internal: "FIRST".into(),
                system: "Game Boy".into(),
                search_text: "shared first.gb".into(),
            },
            super::LibrarySearchEntry {
                source: RomSource::File(std::path::PathBuf::from("second.gb")),
                commercial: "Second".into(),
                internal: "SECOND".into(),
                system: "Game Boy Color".into(),
                search_text: "shared second.gb".into(),
            },
        ];
        let visible = super::visible_library_entries(&entries, &state, "shared");
        let (labels, internals, systems, ids) = super::library_model_data(&visible, &state);
        assert_eq!(labels.len(), 2);
        assert_eq!(internals[0], "FIRST");
        assert_eq!(systems[1], "Game Boy Color");
        assert_eq!(ids[0], "first.gb");
        assert_eq!(ids[1], "second.gb");
    }

    #[test]
    fn input_sources_and_consoles_remain_independent() {
        let mut input = InputState::default();
        assert_eq!(
            input.set(1, InputSource::Keyboard, Button::A, true),
            Some(true)
        );
        assert_eq!(input.set(1, InputSource::Virtual, Button::A, true), None);
        assert_eq!(
            input.set(2, InputSource::Virtual, Button::A, true),
            Some(true)
        );
        assert_eq!(input.set(1, InputSource::Virtual, Button::A, false), None);
        assert_eq!(
            input.set(1, InputSource::Keyboard, Button::A, false),
            Some(false)
        );
        assert_eq!(
            input.set(2, InputSource::Virtual, Button::A, false),
            Some(false)
        );
    }

    #[test]
    fn virtual_cancel_releases_only_virtual_buttons() {
        let mut input = InputState::default();
        assert_eq!(
            input.set(1, InputSource::Virtual, Button::Right, true),
            Some(true)
        );
        assert_eq!(
            input.set(1, InputSource::Keyboard, Button::A, true),
            Some(true)
        );
        assert_eq!(
            input.release_source(1, InputSource::Virtual),
            vec![Button::Right]
        );
        assert!(input.release_source(1, InputSource::Virtual).is_empty());
        assert_eq!(
            input.set(1, InputSource::Keyboard, Button::A, false),
            Some(false)
        );
    }

    #[test]
    fn geometry_keeps_integer_framebuffer_sizes_for_all_scales() {
        for scale in 1..=5 {
            let geometry = super::calculate_geometry(super::GeometryInput {
                requested_scale: scale,
                consoles: super::CONSOLE_COUNT_SINGLE,
                controls_visible: true,
                monitor_physical_width: 3840,
                monitor_physical_height: 2160,
                system_scale_factor: 1.0,
            });
            assert_eq!(geometry.effective_scale, scale);
            assert_eq!(geometry.console_width, super::shell_width(u32::from(scale)));
            assert_eq!(geometry.console_height, 144 * u32::from(scale) + 334);
        }
    }

    #[test]
    fn geometry_uses_compact_base_size_for_one_and_two_x() {
        let one = super::calculate_geometry(super::GeometryInput {
            requested_scale: 1,
            consoles: 1,
            controls_visible: true,
            monitor_physical_width: 1300,
            monitor_physical_height: 1200,
            system_scale_factor: 1.0,
        });
        let two = super::calculate_geometry(super::GeometryInput {
            requested_scale: 2,
            consoles: 1,
            controls_visible: true,
            monitor_physical_width: 1920,
            monitor_physical_height: 1200,
            system_scale_factor: 1.0,
        });
        assert_eq!(one.window_width, two.window_width);
        assert_eq!(one.window_height, 542);
        assert_eq!(two.window_height, 686);
        assert_eq!(one.minimum_window_width, two.window_width);
        assert_eq!(one.minimum_window_height, two.window_height);
    }

    #[test]
    fn geometry_lays_out_two_consoles_without_overlap() {
        let geometry = super::calculate_geometry(super::GeometryInput {
            requested_scale: 3,
            consoles: 2,
            controls_visible: true,
            monitor_physical_width: 3840,
            monitor_physical_height: 2160,
            system_scale_factor: 1.0,
        });
        assert_eq!(geometry.effective_scale, 3);
        assert_eq!(geometry.window_width, 2 * geometry.console_width + 16 + 32);
        assert_eq!(geometry.window_height, 830);
    }

    #[test]
    fn geometry_reduces_requested_scale_but_preserves_preference() {
        let geometry = super::calculate_geometry(super::GeometryInput {
            requested_scale: 4,
            consoles: 2,
            controls_visible: true,
            monitor_physical_width: 1300,
            monitor_physical_height: 1200,
            system_scale_factor: 1.0,
        });
        assert_eq!(geometry.requested_scale, 4);
        assert_eq!(geometry.effective_scale, 3);
        assert_eq!(
            geometry.reason,
            Some(super::GeometryReason::RequestedScaleReduced)
        );
    }

    #[test]
    fn geometry_restores_scale_when_second_console_is_removed() {
        let dual = super::calculate_geometry(super::GeometryInput {
            requested_scale: 4,
            consoles: 2,
            controls_visible: true,
            monitor_physical_width: 1300,
            monitor_physical_height: 1200,
            system_scale_factor: 1.0,
        });
        let single = super::calculate_geometry(super::GeometryInput {
            requested_scale: 4,
            consoles: 1,
            controls_visible: true,
            monitor_physical_width: 1920,
            monitor_physical_height: 1200,
            system_scale_factor: 1.0,
        });
        assert_eq!(dual.effective_scale, 3);
        assert_eq!(single.effective_scale, 4);
    }

    #[test]
    fn geometry_accounts_for_hidpi_monitor_scale() {
        let geometry = super::calculate_geometry(super::GeometryInput {
            requested_scale: 5,
            consoles: 1,
            controls_visible: true,
            monitor_physical_width: 3840,
            monitor_physical_height: 2160,
            system_scale_factor: 2.0,
        });
        assert_eq!(geometry.available_width, 1843);
        assert_eq!(geometry.available_height, 1036);
        assert_eq!(geometry.effective_scale, 4);
    }

    #[test]
    fn geometry_reports_impossible_small_monitor_without_fractional_scale() {
        let geometry = super::calculate_geometry(super::GeometryInput {
            requested_scale: 5,
            consoles: 1,
            controls_visible: true,
            monitor_physical_width: 320,
            monitor_physical_height: 240,
            system_scale_factor: 1.0,
        });
        assert_eq!(geometry.effective_scale, 1);
        assert_eq!(
            geometry.reason,
            Some(super::GeometryReason::MinimumGeometryDoesNotFit)
        );
        assert!(geometry.minimum_window_width <= geometry.available_width);
        assert!(geometry.minimum_window_height <= geometry.available_height);
    }

    #[test]
    fn geometry_supports_two_consoles_at_two_x() {
        let geometry = super::calculate_geometry(super::GeometryInput {
            requested_scale: 2,
            consoles: 2,
            controls_visible: true,
            monitor_physical_width: 1920,
            monitor_physical_height: 1200,
            system_scale_factor: 1.0,
        });
        assert_eq!(geometry.effective_scale, 2);
        assert_eq!(geometry.window_width, 760);
    }

    #[test]
    fn geometry_supports_two_consoles_at_four_x() {
        let geometry = super::calculate_geometry(super::GeometryInput {
            requested_scale: 4,
            consoles: 2,
            controls_visible: true,
            monitor_physical_width: 2560,
            monitor_physical_height: 1440,
            system_scale_factor: 1.0,
        });
        assert_eq!(geometry.effective_scale, 4);
        assert_eq!(geometry.window_width, 1376);
    }

    #[test]
    fn geometry_supports_two_consoles_at_five_x_on_large_monitor() {
        let geometry = super::calculate_geometry(super::GeometryInput {
            requested_scale: 5,
            consoles: 2,
            controls_visible: true,
            monitor_physical_width: 3840,
            monitor_physical_height: 2160,
            system_scale_factor: 1.0,
        });
        assert_eq!(geometry.effective_scale, 5);
        assert_eq!(geometry.window_width, 1696);
    }

    #[test]
    fn geometry_changes_with_monitor_resolution() {
        let small = super::calculate_geometry(super::GeometryInput {
            requested_scale: 5,
            consoles: 1,
            controls_visible: true,
            monitor_physical_width: 1280,
            monitor_physical_height: 720,
            system_scale_factor: 1.0,
        });
        let large = super::calculate_geometry(super::GeometryInput {
            requested_scale: 5,
            consoles: 1,
            controls_visible: true,
            monitor_physical_width: 2560,
            monitor_physical_height: 1440,
            system_scale_factor: 1.0,
        });
        assert!(small.effective_scale < large.effective_scale);
    }

    #[test]
    fn geometry_changes_with_active_monitor() {
        let monitor_a = super::calculate_geometry(super::GeometryInput {
            requested_scale: 5,
            consoles: 2,
            controls_visible: true,
            monitor_physical_width: 1300,
            monitor_physical_height: 1200,
            system_scale_factor: 1.0,
        });
        let monitor_b = super::calculate_geometry(super::GeometryInput {
            requested_scale: 5,
            consoles: 2,
            controls_visible: true,
            monitor_physical_width: 3840,
            monitor_physical_height: 2160,
            system_scale_factor: 1.0,
        });
        assert!(monitor_a.effective_scale < monitor_b.effective_scale);
    }

    #[test]
    fn geometry_minimum_changes_when_controls_are_hidden() {
        let visible = super::calculate_geometry(super::GeometryInput {
            requested_scale: 2,
            consoles: 1,
            controls_visible: true,
            monitor_physical_width: 1920,
            monitor_physical_height: 1200,
            system_scale_factor: 1.0,
        });
        let hidden = super::calculate_geometry(super::GeometryInput {
            requested_scale: 2,
            consoles: 1,
            controls_visible: false,
            monitor_physical_width: 1920,
            monitor_physical_height: 1200,
            system_scale_factor: 1.0,
        });
        assert!(hidden.minimum_window_height < visible.minimum_window_height);
    }

    #[test]
    fn geometry_keeps_compact_horizontal_margin() {
        assert_eq!(super::shell_width(2), 356);
        assert_eq!(super::shell_width(3), 504);
        assert_eq!(super::shell_width(5), 824);
    }

    #[test]
    fn geometry_never_reports_a_fitting_window_above_available_area() {
        let geometry = super::calculate_geometry(super::GeometryInput {
            requested_scale: 5,
            consoles: 2,
            controls_visible: true,
            monitor_physical_width: 1300,
            monitor_physical_height: 1200,
            system_scale_factor: 1.0,
        });
        assert!(geometry.window_width <= geometry.available_width);
        assert!(geometry.window_height <= geometry.available_height);
    }

    #[test]
    fn geometry_uses_one_effective_scale_for_both_consoles() {
        let geometry = super::calculate_geometry(super::GeometryInput {
            requested_scale: 4,
            consoles: 2,
            controls_visible: true,
            monitor_physical_width: 1300,
            monitor_physical_height: 1200,
            system_scale_factor: 1.0,
        });
        assert_eq!(
            geometry.console_width,
            super::shell_width(u32::from(geometry.effective_scale))
        );
    }

    #[test]
    fn geometry_does_not_overwrite_requested_scale() {
        let geometry = super::calculate_geometry(super::GeometryInput {
            requested_scale: 5,
            consoles: 2,
            controls_visible: true,
            monitor_physical_width: 1300,
            monitor_physical_height: 1200,
            system_scale_factor: 1.0,
        });
        assert_eq!(geometry.requested_scale, 5);
        assert!(geometry.effective_scale < geometry.requested_scale);
    }

    #[test]
    fn geometry_only_returns_supported_integer_scales() {
        for requested in 1..=5 {
            let geometry = super::calculate_geometry(super::GeometryInput {
                requested_scale: requested,
                consoles: 1,
                controls_visible: true,
                monitor_physical_width: 1920,
                monitor_physical_height: 1200,
                system_scale_factor: 1.0,
            });
            assert!((1..=5).contains(&geometry.effective_scale));
        }
    }

    #[test]
    fn geometry_is_deterministic_for_manual_resize_events() {
        let input = super::GeometryInput {
            requested_scale: 3,
            consoles: 1,
            controls_visible: true,
            monitor_physical_width: 1920,
            monitor_physical_height: 1200,
            system_scale_factor: 1.0,
        };
        assert_eq!(
            super::calculate_geometry(input),
            super::calculate_geometry(input)
        );
    }

    #[test]
    fn geometry_minimum_for_dual_mode_includes_gap() {
        let geometry = super::calculate_geometry(super::GeometryInput {
            requested_scale: 1,
            consoles: 2,
            controls_visible: true,
            monitor_physical_width: 1920,
            monitor_physical_height: 1200,
            system_scale_factor: 1.0,
        });
        assert_eq!(geometry.minimum_window_width, 760);
        assert_eq!(geometry.minimum_window_height, 686);
    }

    #[test]
    fn settings_menu_toggle_is_reversible() {
        assert!(super::next_settings_open(false));
        assert!(!super::next_settings_open(true));
        assert!(!super::next_settings_open(super::next_settings_open(false)));
    }

    use super::{
        apply_rebind, binding_owner, profile_for_key, rebind_conflict, InputConfig, RebindError,
    };

    #[test]
    fn keyboard_profiles_never_share_face_buttons() {
        let config = InputConfig::default();
        assert_eq!(profile_for_key(&config, "x"), Some((1, Button::A)));
        assert_eq!(profile_for_key(&config, "u"), Some((2, Button::A)));
        assert_ne!(
            profile_for_key(&config, "z").map(|entry| entry.0),
            profile_for_key(&config, "u").map(|entry| entry.0)
        );
    }

    #[test]
    fn unknown_key_is_not_broadcast() {
        assert_eq!(profile_for_key(&InputConfig::default(), "q"), None);
    }

    #[test]
    fn default_console1_uses_conventional_keyboard_keys() {
        let config = InputConfig::default();
        assert_eq!(config.console1.left, "ArrowLeft");
        assert_eq!(config.console1.right, "ArrowRight");
        assert_eq!(config.console1.up, "ArrowUp");
        assert_eq!(config.console1.down, "ArrowDown");
        assert_eq!(binding_owner(&config, "x"), Some((1, "a")));
        assert_eq!(binding_owner(&config, "z"), Some((1, "b")));
        assert_eq!(binding_owner(&config, "Enter"), Some((1, "start")));
        assert_eq!(binding_owner(&config, "Backspace"), Some((1, "select")));
        assert_eq!(binding_owner(&config, "ArrowLeft"), Some((1, "left")));
        assert_eq!(binding_owner(&config, "ArrowDown"), Some((1, "down")));
    }

    #[test]
    fn slint_special_key_codes_normalize_to_persisted_identifiers() {
        assert_eq!(
            super::normalize_key_identifier("\u{F702}").as_deref(),
            Some("ArrowLeft")
        );
        assert_eq!(
            super::normalize_key_identifier("\u{F703}").as_deref(),
            Some("ArrowRight")
        );
        assert_eq!(
            super::normalize_key_identifier("\u{F700}").as_deref(),
            Some("ArrowUp")
        );
        assert_eq!(
            super::normalize_key_identifier("\u{F701}").as_deref(),
            Some("ArrowDown")
        );
        assert_eq!(
            super::normalize_key_identifier("\u{000a}").as_deref(),
            Some("Enter")
        );
        assert_eq!(
            super::normalize_key_identifier("\u{0008}").as_deref(),
            Some("Backspace")
        );
        assert_eq!(
            super::normalize_key_identifier("\u{001b}").as_deref(),
            Some("Escape")
        );
    }

    #[test]
    fn normalized_special_keys_reach_the_correct_console_button() {
        let config = InputConfig::default();
        assert_eq!(
            profile_for_key(&config, "\u{F702}"),
            Some((1, Button::Left))
        );
        assert_eq!(
            profile_for_key(&config, "\u{F703}"),
            Some((1, Button::Right))
        );
        assert_eq!(profile_for_key(&config, "\u{F700}"), Some((1, Button::Up)));
        assert_eq!(
            profile_for_key(&config, "\u{F701}"),
            Some((1, Button::Down))
        );
        assert_eq!(
            profile_for_key(&config, "\u{000a}"),
            Some((1, Button::Start))
        );
        assert_eq!(
            profile_for_key(&config, "\u{0008}"),
            Some((1, Button::Select))
        );
        assert_eq!(profile_for_key(&config, "i"), Some((2, Button::Up)));
    }

    #[test]
    fn invalid_or_empty_bindings_are_repaired_without_losing_valid_values() {
        let mut config = InputConfig::default();
        config.console1.left.clear();
        config.console1.right = "not-a-key-name".into();
        config.console1.a = "q".into();
        config.console2.down = "\u{F701}".into();
        assert!(super::sanitize_input_config(&mut config));
        assert_eq!(config.console1.left, "ArrowLeft");
        assert_eq!(config.console1.right, "ArrowRight");
        assert_eq!(config.console1.a, "q");
        assert_eq!(config.console2.down, "ArrowDown");
    }

    #[test]
    fn rebind_conflicts_are_reported_without_overwriting() {
        let config = InputConfig::default();
        assert_eq!(rebind_conflict(&config, 1, "b", "x"), Some((1, "a")));
        assert_eq!(rebind_conflict(&config, 1, "a", "x"), None);
        assert_eq!(rebind_conflict(&config, 2, "a", "x"), Some((1, "a")));
    }

    #[test]
    fn consecutive_rebinds_update_both_consoles_without_losing_the_first() {
        let mut config = InputConfig::default();
        apply_rebind(&mut config, 1, "a", "q").expect("C1 A rebinds");
        apply_rebind(&mut config, 2, "a", "r").expect("C2 A rebinds");
        assert_eq!(config.console1.a, "q");
        assert_eq!(config.console2.a, "r");
    }

    #[test]
    fn special_keys_are_assignable_and_conflicts_do_not_change_the_target() {
        let mut config = InputConfig::default();
        apply_rebind(&mut config, 1, "select", "\u{0008}").expect("Backspace is assignable");
        let before = config.console1.b.clone();
        assert_eq!(
            apply_rebind(&mut config, 1, "b", "\u{0008}"),
            Err(RebindError::Conflict(1, "select"))
        );
        assert_eq!(config.console1.b, before);
        assert_eq!(apply_rebind(&mut config, 1, "up", "\u{F700}"), Ok(()));
        assert_eq!(config.console1.up, "ArrowUp");
        let previous = config.console1.a.clone();
        assert_eq!(
            apply_rebind(&mut config, 1, "a", "\u{001b}"),
            Err(RebindError::InvalidAction)
        );
        assert_eq!(config.console1.a, previous);
    }

    #[test]
    fn input_config_roundtrips_as_json() {
        let config = InputConfig::default();
        let encoded = serde_json::to_string(&config).expect("config serializes");
        let decoded: InputConfig = serde_json::from_str(&encoded).expect("config loads");
        assert_eq!(decoded.console1.a, "x");
        assert_eq!(decoded.console1.b, "z");
        assert_eq!(decoded.console1.start, "Enter");
        assert_eq!(decoded.console1.select, "Backspace");
        assert_eq!(decoded.console1.left, "ArrowLeft");
        assert_eq!(decoded.console1.right, "ArrowRight");
        assert_eq!(decoded.console1.up, "ArrowUp");
        assert_eq!(decoded.console1.down, "ArrowDown");
        assert_eq!(decoded.console2.gamepad_slot, 2);
        assert_eq!(decoded.console1.quick_save, "F5");
        assert_eq!(decoded.console1.quick_load, "F6");
        assert_eq!(decoded.console2.quick_save, "F9");
        assert_eq!(decoded.console2.quick_load, "F10");
    }

    #[test]
    fn quick_shortcuts_normalize_and_are_console_specific() {
        let config = InputConfig::default();
        assert_eq!(quick_action_for_key(&config, "F5"), Some((1, true)));
        assert_eq!(quick_action_for_key(&config, "F6"), Some((1, false)));
        assert_eq!(quick_action_for_key(&config, "F9"), Some((2, true)));
        assert_eq!(quick_action_for_key(&config, "F10"), Some((2, false)));
        assert_eq!(normalize_key_identifier("\u{F708}").as_deref(), Some("F5"));
        assert_eq!(normalize_key_identifier("\u{F70D}").as_deref(), Some("F10"));
    }

    #[test]
    fn library_state_roundtrips_favorites_recent_and_cover() {
        let mut state = super::LibraryState::default();
        state.favorites.insert("game.gb".into());
        state.recent.push("game.gb".into());
        state.covers.insert("game.gb".into(), "cover.png".into());
        let encoded = serde_json::to_string(&state).expect("library serializes");
        let decoded: super::LibraryState = serde_json::from_str(&encoded).expect("library loads");
        assert!(decoded.favorites.contains("game.gb"));
        assert_eq!(decoded.recent, vec!["game.gb"]);
        assert_eq!(
            decoded.covers.get("game.gb").map(String::as_str),
            Some("cover.png")
        );
    }

    #[test]
    fn battery_path_is_shared_between_consoles_and_identity_stable() {
        let path = RomSource::File(std::path::PathBuf::from("sample.gb"));
        assert_eq!(
            super::console_save_path(&path, 1),
            super::console_save_path(&path, 2)
        );
        assert!(super::console_save_path(&path, 1).extension().is_some());
    }

    #[test]
    fn zip_save_anchor_stays_beside_archive_and_avoids_direct_collision() {
        let direct = RomSource::File(std::path::PathBuf::from("roms/sample.gb"));
        let zipped = RomSource::Zip {
            archive: std::path::PathBuf::from("roms/sample.zip"),
            entry: "folder/sample.gb".into(),
        };
        let direct_save = super::console_save_path(&direct, 1);
        let zip_save = super::console_save_path(&zipped, 1);
        assert_eq!(zip_save.parent(), Some(std::path::Path::new("roms")));
        assert_ne!(direct_save, zip_save);
        assert!(zip_save.to_string_lossy().contains("zip_"));
    }

    #[test]
    fn rom2_argument_selects_an_independent_second_cartridge() {
        let cli = super::Cli::try_parse_from(["foxmulator", "first.gb", "--rom2", "second.gb"])
            .expect("legacy ROM syntax parses");
        assert_eq!(
            cli.positional_rom1,
            Some(std::path::PathBuf::from("first.gb"))
        );
        assert_eq!(cli.rom2, Some(std::path::PathBuf::from("second.gb")));
    }

    #[test]
    fn cli_accepts_new_launch_options_and_dash_prefixed_positional_paths() {
        let cli = super::Cli::try_parse_from([
            "foxmulator",
            "--rom1",
            "one.gb",
            "--rom2",
            "two.gb",
            "--link",
            "--fullscreen",
            "--mute",
            "--library",
            "roms",
        ])
        .expect("new CLI options parse");
        assert_eq!(cli.rom1, Some(std::path::PathBuf::from("one.gb")));
        assert_eq!(cli.rom2, Some(std::path::PathBuf::from("two.gb")));
        assert!(cli.link && cli.fullscreen && cli.mute);
        assert_eq!(cli.library, Some(std::path::PathBuf::from("roms")));

        let dash_path = super::Cli::try_parse_from(["foxmulator", "--", "-game.gb"])
            .expect("-- allows a dash-prefixed positional path");
        assert_eq!(
            dash_path.positional_rom1,
            Some(std::path::PathBuf::from("-game.gb"))
        );
    }

    #[test]
    fn cli_help_and_version_are_handled_before_application_startup() {
        use clap::error::ErrorKind;
        assert_eq!(
            super::Cli::try_parse_from(["foxmulator", "--help"])
                .expect_err("help exits through clap")
                .kind(),
            ErrorKind::DisplayHelp
        );
        assert_eq!(
            super::Cli::try_parse_from(["foxmulator", "--version"])
                .expect_err("version exits through clap")
                .kind(),
            ErrorKind::DisplayVersion
        );
    }

    #[test]
    fn cli_rejects_unknown_missing_duplicate_and_incompatible_arguments() {
        use clap::error::ErrorKind;
        assert_eq!(
            super::Cli::try_parse_from(["foxmulator", "--unknown"])
                .expect_err("unknown option is rejected")
                .kind(),
            ErrorKind::UnknownArgument
        );
        assert!(super::Cli::try_parse_from(["foxmulator", "--rom2"]).is_err());
        assert!(
            super::Cli::try_parse_from(["foxmulator", "--rom2", "one.gb", "--rom2", "two.gb"])
                .is_err()
        );
        assert!(super::Cli::try_parse_from(["foxmulator", "one.gb", "--rom1", "two.gb"]).is_err());
    }

    #[test]
    fn cli_validation_requires_files_directories_and_link_roms() {
        let cli = super::Cli::try_parse_from(["foxmulator", "--link"]).unwrap();
        assert!(super::validate_cli(cli)
            .unwrap_err()
            .contains("--link requiere"));
        let cli = super::Cli::try_parse_from(["foxmulator", "missing.gb"]).unwrap();
        assert!(super::validate_cli(cli)
            .unwrap_err()
            .contains("ROM1 no existe"));
        let cli =
            super::Cli::try_parse_from(["foxmulator", "--library", "missing-folder"]).unwrap();
        assert!(super::validate_cli(cli)
            .unwrap_err()
            .contains("biblioteca no existe"));
    }

    #[test]
    fn integer_scale_preserves_native_dimensions_and_bounds() {
        let geometry = super::calculate_geometry(super::GeometryInput {
            requested_scale: 5,
            consoles: 1,
            controls_visible: true,
            monitor_physical_width: 3840,
            monitor_physical_height: 2160,
            system_scale_factor: 1.0,
        });
        assert_eq!((160 * 5, 144 * 5), (800, 720));
        assert_eq!(geometry.effective_scale, 5);
        assert!(geometry.window_width >= 800);
        assert!(geometry.window_height >= 720);
    }

    #[test]
    fn new_display_preferences_are_persistent_and_backward_compatible() {
        let config = InputConfig::default();
        assert!(config.auto_scale);
        assert_eq!(config.screen_scale, 2);
        let old = r#"{"console1":{"left":"f","right":"h","up":"t","down":"g","a":"z","b":"x","select":"c","start":"v","gamepad_slot":1},"console2":{"left":"j","right":"l","up":"i","down":"k","a":"u","b":"o","select":"p","start":";","gamepad_slot":2}}"#;
        let decoded: InputConfig = serde_json::from_str(old).expect("old config loads");
        assert!(decoded.auto_scale);
        assert_eq!(decoded.screen_scale, 2);
        assert_eq!(decoded.console1.a, "z");
        assert_eq!(decoded.console1.start, "v");
    }

    #[test]
    fn appearance_preferences_roundtrip_custom_colors_and_translucency() {
        let config = InputConfig {
            shell_color: "#123456".into(),
            accent_color: "#abcdef".into(),
            translucent_shell: false,
            translucent_tint: 85,
            ..InputConfig::default()
        };
        let encoded = serde_json::to_string(&config).expect("appearance serializes");
        let decoded: InputConfig = serde_json::from_str(&encoded).expect("appearance loads");
        assert_eq!(decoded.shell_color, "#123456");
        assert_eq!(decoded.accent_color, "#abcdef");
        assert!(!decoded.translucent_shell);
        assert_eq!(decoded.translucent_tint, 85);
        assert!(super::is_hex_color("#abcdef"));
        assert!(!super::is_hex_color("abcdef"));
    }

    #[test]
    fn audio_preferences_roundtrip_with_independent_mutes() {
        let config = InputConfig {
            master_volume: 72,
            console1_volume: 35,
            console2_volume: 88,
            console1_muted: true,
            console2_muted: false,
            ..InputConfig::default()
        };
        let encoded = serde_json::to_string(&config).expect("audio config serializes");
        let decoded: InputConfig = serde_json::from_str(&encoded).expect("audio config loads");
        assert_eq!(decoded.master_volume, 72);
        assert_eq!(decoded.console1_volume, 35);
        assert_eq!(decoded.console2_volume, 88);
        assert!(decoded.console1_muted);
        assert!(!decoded.console2_muted);
    }

    #[test]
    fn ejecting_one_console_preserves_the_other_and_clears_held_inputs() {
        let mut dual = super::DualSystems {
            left: foxmulator_core::System::from_rom(super::fallback_rom()),
            right: foxmulator_core::System::from_rom(super::fallback_rom()),
            cable: foxmulator_core::LinkCable::new(),
            left_loaded: true,
            right_loaded: true,
            right_visible: true,
            link_enabled: true,
            left_cart: "Left".into(),
            right_cart: "Right".into(),
        };
        let right_before = dual.right.clone();
        super::eject_console(&mut dual, 1, None).expect("left ejects");
        assert!(!dual.left_loaded);
        assert_eq!(dual.left_cart, "No cartridge");
        assert!(dual.right_loaded);
        assert_eq!(dual.right_cart, "Right");
        assert_eq!(dual.right.cpu.pc, right_before.cpu.pc);
        assert!(dual.link_enabled);

        let mut inputs = InputState::default();
        inputs.set(1, InputSource::Keyboard, Button::A, true);
        inputs.set(1, InputSource::Virtual, Button::Right, true);
        assert_eq!(inputs.release_console(1), vec![Button::Right, Button::A]);
    }

    #[test]
    fn failed_save_cancels_ejection_without_losing_the_cartridge() {
        let mut dual = super::DualSystems {
            left: foxmulator_core::System::from_rom(super::fallback_rom()),
            right: foxmulator_core::System::new(false),
            cable: foxmulator_core::LinkCable::new(),
            left_loaded: true,
            right_loaded: false,
            right_visible: false,
            link_enabled: false,
            left_cart: "Left".into(),
            right_cart: "No cartridge".into(),
        };
        let path = std::env::temp_dir()
            .join("foxmulator-missing-save-parent")
            .join("save.sav");
        assert!(super::eject_console(&mut dual, 1, Some(&path)).is_err());
        assert!(dual.left_loaded);
        assert_eq!(dual.left_cart, "Left");
    }

    #[test]
    fn ejecting_console_two_preserves_console_one_and_writes_sram() {
        let mut right = foxmulator_core::System::from_rom(super::fallback_rom());
        right.mmu.external_ram[0] = 0xa5;
        let mut dual = super::DualSystems {
            left: foxmulator_core::System::from_rom(super::fallback_rom()),
            right,
            cable: foxmulator_core::LinkCable::new(),
            left_loaded: true,
            right_loaded: true,
            right_visible: true,
            link_enabled: true,
            left_cart: "Left".into(),
            right_cart: "Right".into(),
        };
        let save = std::env::temp_dir().join(format!(
            "foxmulator-eject-{}-{}.sav",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));
        super::eject_console(&mut dual, 2, Some(&save)).expect("right ejects after save");
        let saved = std::fs::read(&save).expect("SRAM was saved");
        assert_eq!(saved.first(), Some(&0xa5));
        assert!(dual.left_loaded);
        assert!(!dual.right_loaded);
        assert!(dual.link_enabled);
        std::fs::remove_file(save).expect("removes only test save");
    }

    #[test]
    fn idle_worker_does_not_execute_or_hold_the_system_without_cartridges() {
        use std::sync::atomic::AtomicBool;
        use std::sync::Arc;

        let systems = super::SharedSystems::new(super::DualSystems {
            left: foxmulator_core::System::from_rom(super::fallback_rom()),
            right: foxmulator_core::System::from_rom(super::fallback_rom()),
            cable: foxmulator_core::LinkCable::new(),
            left_loaded: false,
            right_loaded: false,
            right_visible: false,
            link_enabled: false,
            left_cart: "No cartridge".into(),
            right_cart: "No cartridge".into(),
        });
        let frames = Arc::new(std::sync::Mutex::new(super::FrameBuffers::new(
            Vec::new(),
            Vec::new(),
        )));
        let frame_control = Arc::new(super::FrameControl::new());
        let stats = Arc::new(super::WorkerStats::new());
        let stop = Arc::new(AtomicBool::new(false));
        let worker_systems = systems.clone();
        let worker_frames = Arc::clone(&frames);
        let worker_frame_control = Arc::clone(&frame_control);
        let worker_stats = Arc::clone(&stats);
        let worker_stop = Arc::clone(&stop);
        let worker_speed = Arc::new(std::sync::atomic::AtomicU32::new(1));
        #[cfg(feature = "audio")]
        let worker_audio_mix = Arc::new(std::sync::Mutex::new(super::AudioMixSettings::from(
            &super::InputConfig::default(),
        )));
        let worker = std::thread::spawn(move || {
            super::run_emulation_worker(
                worker_systems,
                worker_frames,
                worker_frame_control,
                worker_stats,
                worker_stop,
                worker_speed,
                None,
                #[cfg(feature = "audio")]
                worker_audio_mix,
            );
        });
        std::thread::sleep(std::time::Duration::from_millis(25));
        stop.store(true, std::sync::atomic::Ordering::Release);
        worker.join().expect("idle worker exits");
        assert_eq!(stats.cycles.load(std::sync::atomic::Ordering::Relaxed), 0);
        let _guard = systems.borrow();
    }
}

#[cfg(all(test, feature = "gui"))]
mod library_identity_tests {
    use super::{catalog_match, parse_rdb_crc_names, rom_crc32};

    #[test]
    fn crc32_matches_standard_vector() {
        assert_eq!(rom_crc32(b"123456789"), 0xcbf4_3926);
    }

    #[test]
    fn rdb_index_reads_name_and_crc_fields() {
        let mut data = vec![0; 16];
        data.extend_from_slice(&[
            0x83, 0xa4, b'n', b'a', b'm', b'e', 0xa4, b'G', b'a', b'm', b'e',
        ]);
        data.extend_from_slice(&[0xa3, b'c', b'r', b'c', 0xc4, 0x04, 0x12, 0x34, 0x56, 0x78]);
        data.extend_from_slice(&[0xa4, b's', b'i', b'z', b'e', 0xcc, 0x01, 0xc0]);
        let index = parse_rdb_crc_names(&data);
        assert_eq!(index.get(&0x1234_5678), Some(&"Game".to_string()));
    }

    #[test]
    fn catalog_matching_ignores_rom_and_image_extensions() {
        let catalog = [
            "Sample Color Title.png".to_owned(),
            "Sample Mono Title.png".to_owned(),
        ];
        assert_eq!(
            catalog_match(&["Sample Color Title.gbc".to_owned()], &catalog,),
            Some("Sample Color Title".to_owned())
        );
        assert_eq!(
            catalog_match(&["Sample Mono Title.gb".to_owned()], &catalog),
            Some("Sample Mono Title".to_owned())
        );
    }
}

#[cfg(all(test, feature = "gui", feature = "audio"))]
mod audio_tests {
    use super::{mix_pcm_samples, AudioMixSettings, PcmRing};

    #[test]
    fn pcm_ring_is_fifo_and_bounded() {
        let ring = PcmRing::new();
        let input = (0..128).map(|value| value as f32).collect::<Vec<_>>();
        assert!(ring.push(&input));
        let mut output = [0.0; 128];
        assert_eq!(ring.pop_into(&mut output), input.len());
        assert_eq!(&output[..], &input[..]);
        assert_eq!(ring.len(), 0);
        assert!(!ring.push(&vec![0.0; PcmRing::CAPACITY + 1]));
    }

    #[test]
    fn pcm_mix_applies_independent_gains_and_preserves_stereo_order() {
        let settings = AudioMixSettings {
            master_volume: 100,
            console1_volume: 100,
            console2_volume: 50,
            console1_muted: false,
            console2_muted: false,
        };
        let mixed = mix_pcm_samples(&[0.8, -0.8], &[0.4, -0.4], settings);
        assert_eq!(mixed, vec![0.5, -0.5]);
    }

    #[test]
    fn pcm_mix_mute_recovers_configured_volume_and_clamps_output() {
        let muted = AudioMixSettings {
            master_volume: 100,
            console1_volume: 80,
            console2_volume: 100,
            console1_muted: true,
            console2_muted: false,
        };
        assert_eq!(mix_pcm_samples(&[1.0], &[1.0], muted), vec![0.5]);
        let restored = AudioMixSettings {
            console1_muted: false,
            master_volume: 100,
            console1_volume: 100,
            console2_volume: 100,
            console2_muted: false,
        };
        assert_eq!(mix_pcm_samples(&[4.0], &[4.0], restored), vec![1.0]);
    }
}

#[cfg(all(feature = "gui", feature = "gamepad"))]
struct GamepadRouter {
    gilrs: gilrs::Gilrs,
    assignments: [Option<usize>; 2],
    order: [usize; 2],
}

#[cfg(all(feature = "gui", feature = "gamepad"))]
fn assign_gamepad(
    assignments: &mut [Option<usize>; 2],
    order: [usize; 2],
    id: usize,
) -> Option<usize> {
    if assignments[0] == Some(id) {
        return Some(0);
    }
    if assignments[1] == Some(id) {
        return Some(1);
    }
    for console in order {
        if assignments[console].is_none() {
            assignments[console] = Some(id);
            return Some(console);
        }
    }
    None
}

#[cfg(all(feature = "gui", feature = "gamepad"))]
impl GamepadRouter {
    fn new(order: [usize; 2]) -> Option<Self> {
        Some(Self {
            gilrs: gilrs::Gilrs::new().ok()?,
            assignments: [None, None],
            order,
        })
    }

    fn target(&mut self, id: gilrs::GamepadId) -> Option<usize> {
        assign_gamepad(&mut self.assignments, self.order, usize::from(id))
    }

    fn release(&mut self, id: gilrs::GamepadId) -> Option<usize> {
        let id = usize::from(id);
        for (console, assignment) in self.assignments.iter_mut().enumerate() {
            if *assignment == Some(id) {
                *assignment = None;
                return Some(console);
            }
        }
        None
    }
}

#[cfg(all(feature = "gui", feature = "gamepad"))]
fn gamepad_button(button: gilrs::Button) -> Option<Button> {
    use gilrs::Button as PadButton;
    Some(match button {
        PadButton::South => Button::A,
        PadButton::East => Button::B,
        PadButton::Select => Button::Select,
        PadButton::Start => Button::Start,
        PadButton::DPadLeft => Button::Left,
        PadButton::DPadRight => Button::Right,
        PadButton::DPadUp => Button::Up,
        PadButton::DPadDown => Button::Down,
        _ => return None,
    })
}

#[cfg(all(test, feature = "gui", feature = "gamepad"))]
mod gamepad_tests {
    use super::{assign_gamepad, gamepad_button, Button};

    #[test]
    fn gamepads_assign_once_and_never_broadcast() {
        let mut assignments = [None, None];
        assert_eq!(assign_gamepad(&mut assignments, [0, 1], 11), Some(0));
        assert_eq!(assign_gamepad(&mut assignments, [0, 1], 22), Some(1));
        assert_eq!(assign_gamepad(&mut assignments, [0, 1], 11), Some(0));
        assert_eq!(assign_gamepad(&mut assignments, [0, 1], 33), None);
        assert_eq!(assignments, [Some(11), Some(22)]);
    }

    #[test]
    fn released_gamepad_slot_can_be_reassigned() {
        let mut assignments = [Some(11), Some(22)];
        assignments[0] = None;
        assert_eq!(assign_gamepad(&mut assignments, [0, 1], 33), Some(0));
        assert_eq!(assignments, [Some(33), Some(22)]);
    }

    #[test]
    fn gamepad_buttons_map_to_game_boy_buttons() {
        use gilrs::Button as PadButton;

        assert_eq!(gamepad_button(PadButton::South), Some(Button::A));
        assert_eq!(gamepad_button(PadButton::East), Some(Button::B));
        assert_eq!(gamepad_button(PadButton::DPadUp), Some(Button::Up));
        assert_eq!(gamepad_button(PadButton::DPadDown), Some(Button::Down));
        assert_eq!(gamepad_button(PadButton::Mode), None);
    }
}

/// Build a minimal valid ROM that loops forever with NOP on bank 0.
fn fallback_rom() -> Vec<u8> {
    let mut rom = vec![0u8; 0x8000];
    // Entry point at 0x0100: infinite JR -2 loop.
    rom[0x0100] = 0x18; // JR
    rom[0x0101] = 0xFE; // -2
                        // Cartridge header: ROM only, no RAM, DMG.
    rom[0x0147] = 0x00;
    rom[0x0148] = 0x00;
    rom[0x0149] = 0x00;
    rom
}

fn console_save_path(source: &RomSource, _console: usize) -> std::path::PathBuf {
    let identity = save_identity(source);
    save_slots::canonical_battery_path(source.physical_path(), &identity)
}

fn save_identity(source: &RomSource) -> String {
    let identity = rom_identity_source(source).unwrap_or_else(|| "unknown".into());
    let RomSource::Zip { entry, .. } = source else {
        return identity;
    };
    // The ROM checksum alone is insufficient for two entries in one archive
    // (or for an archive beside a direct ROM with the same basename). Keep the
    // existing direct-ROM naming unchanged and add a deterministic entry tag
    // only for ZIP origins.
    let mut hash = 0x811c9dc5_u32;
    for byte in entry.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x01000193);
    }
    format!("{identity}.zip_{hash:08X}")
}

fn load_console_ram(mmu: &mut foxmulator_core::mmu::Mmu, source: &RomSource, console: usize) {
    let canonical = console_save_path(source, console);
    let path = if rom_identity_source(source).is_some() {
        save_slots::resolved_battery_path(source.physical_path(), &save_identity(source))
    } else {
        canonical
    };
    let _ = save_slots::migrate_battery(mmu, source.physical_path(), &path);
    let _ = mmu.load_ram(&path);
}

#[derive(Debug, Parser)]
#[command(
    name = "FoxMulator Next",
    version,
    about = "Emulador de Game Boy y Game Boy Color"
)]
struct Cli {
    /// ROM de la Consola 1 (forma posicional compatible).
    #[arg(value_name = "ROM1", index = 1, conflicts_with = "rom1")]
    positional_rom1: Option<std::path::PathBuf>,

    /// ROM de la Consola 1.
    #[arg(long, value_name = "PATH", conflicts_with = "positional_rom1")]
    rom1: Option<std::path::PathBuf>,

    /// ROM de la Consola 2.
    #[arg(long, value_name = "PATH")]
    rom2: Option<std::path::PathBuf>,

    /// Inicia el Cable Link; requiere ROM1 y ROM2 válidas.
    #[arg(long)]
    link: bool,

    /// Inicia la ventana en pantalla completa.
    #[arg(long)]
    fullscreen: bool,

    /// Inicia con el audio silenciado sin modificar las preferencias.
    #[arg(long)]
    mute: bool,

    /// Carpeta inicial de la biblioteca sin modificar las preferencias.
    #[arg(long, value_name = "PATH")]
    library: Option<std::path::PathBuf>,
}

#[derive(Debug, PartialEq, Eq)]
struct LaunchOptions {
    rom1: Option<RomSource>,
    rom2: Option<RomSource>,
    link: bool,
    fullscreen: bool,
    mute: bool,
    library: Option<std::path::PathBuf>,
}

fn validate_rom(path: std::path::PathBuf, label: &str) -> Result<RomSource, String> {
    if !path.is_file() {
        return Err(format!(
            "{label} no existe o no es un archivo: {}",
            path.display()
        ));
    }
    RomSource::from_path(path)
        .map_err(|error| format!("{label} no se puede leer o no contiene una ROM válida: {error}"))
}

fn validate_library(path: std::path::PathBuf) -> Result<std::path::PathBuf, String> {
    if !path.is_dir() {
        return Err(format!(
            "La carpeta de biblioteca no existe: {}",
            path.display()
        ));
    }
    std::fs::read_dir(&path)
        .map(|_| path.clone())
        .map_err(|error| {
            format!(
                "La carpeta de biblioteca no se puede leer ({}): {error}",
                path.display()
            )
        })
}

fn validate_cli(cli: Cli) -> Result<LaunchOptions, String> {
    let rom1 = cli.rom1.or(cli.positional_rom1);
    let rom1 = rom1.map(|path| validate_rom(path, "ROM1")).transpose()?;
    let rom2 = cli
        .rom2
        .map(|path| validate_rom(path, "ROM2"))
        .transpose()?;
    if cli.link && (rom1.is_none() || rom2.is_none()) {
        return Err("--link requiere una ROM1 y una ROM2 válidas".into());
    }
    let library = cli.library.map(validate_library).transpose()?;
    Ok(LaunchOptions {
        rom1,
        rom2,
        link: cli.link,
        fullscreen: cli.fullscreen,
        mute: cli.mute,
        library,
    })
}

fn load_system(rom_source: Option<&RomSource>) -> (System, Option<std::path::PathBuf>) {
    let rom = rom_source
        .and_then(|source| source.load().ok())
        .unwrap_or_else(fallback_rom);
    let mut system = System::from_rom_with_embedded_boot(rom)
        .unwrap_or_else(|error| panic!("integrated SameBoy boot ROM failed: {error}"));
    if let Some(source) = rom_source {
        load_console_ram(&mut system.mmu, source, 1);
    }
    (
        system,
        rom_source.map(|source| console_save_path(source, 1)),
    )
}

#[cfg(feature = "gui")]
fn rom_title(rom: &[u8]) -> String {
    rom.get(0x134..0x144)
        .unwrap_or_default()
        .iter()
        .take_while(|byte| **byte != 0)
        .map(|byte| {
            if byte.is_ascii_graphic() {
                *byte as char
            } else {
                ' '
            }
        })
        .collect::<String>()
        .trim_end()
        .to_string()
}

#[cfg(feature = "gui")]
fn rom_label_source(source: &RomSource) -> Option<String> {
    let rom = source.load().ok()?;
    if rom.len() < 0x150 {
        return None;
    }
    let platform = if System::cgb_mode_for_rom(&rom) {
        "CGB"
    } else {
        "DMG"
    };
    let title = rom_title(&rom);
    let fallback = source
        .physical_path()
        .file_stem()?
        .to_string_lossy()
        .into_owned();
    let title = if title.is_empty() { fallback } else { title };
    Some(format!("{title}\n{platform}"))
}

#[cfg(feature = "gui")]
#[derive(Debug)]
enum LibraryScanEvent {
    Progress(ScanProgress),
    Completed(rom_source::ScanReport, Vec<LibrarySearchEntry>),
    Cancelled,
    Failed(String),
}

#[cfg(feature = "gui")]
struct LibraryScanController {
    generation: AtomicU64,
    cancel: Mutex<Option<Arc<AtomicBool>>>,
    events: Mutex<Vec<(u64, LibraryScanEvent)>>,
}

#[cfg(feature = "gui")]
impl LibraryScanController {
    fn new() -> Self {
        Self {
            generation: AtomicU64::new(0),
            cancel: Mutex::new(None),
            events: Mutex::new(Vec::new()),
        }
    }

    fn start(self: &Arc<Self>, root: std::path::PathBuf, state: LibraryState) -> u64 {
        let generation = self.generation.fetch_add(1, Ordering::AcqRel) + 1;
        let cancel = Arc::new(AtomicBool::new(false));
        if let Some(previous) = self
            .cancel
            .lock()
            .expect("scan mutex poisoned")
            .replace(cancel.clone())
        {
            previous.store(true, Ordering::Release);
        }
        self.events
            .lock()
            .expect("scan events mutex poisoned")
            .retain(|(id, _)| *id == generation);
        let controller = Arc::clone(self);
        std::thread::spawn(move || {
            let outcome = rom_source::scan_with_progress(&root, &cancel, |progress| {
                controller.push(generation, LibraryScanEvent::Progress(progress));
            });
            let event = match outcome {
                ScanOutcome::Completed(report) => {
                    if cancel.load(Ordering::Acquire) {
                        LibraryScanEvent::Cancelled
                    } else {
                        let index = build_library_search_index(&report.sources, &state);
                        if cancel.load(Ordering::Acquire) {
                            LibraryScanEvent::Cancelled
                        } else {
                            LibraryScanEvent::Completed(report, index)
                        }
                    }
                }
                ScanOutcome::Cancelled => LibraryScanEvent::Cancelled,
                ScanOutcome::Failed(error) => LibraryScanEvent::Failed(error),
            };
            controller.push(generation, event);
        });
        generation
    }

    fn cancel(&self) {
        if let Some(token) = self.cancel.lock().expect("scan mutex poisoned").as_ref() {
            token.store(true, Ordering::Release);
        }
    }

    fn push(&self, generation: u64, event: LibraryScanEvent) {
        self.events
            .lock()
            .expect("scan events mutex poisoned")
            .push((generation, event));
    }

    fn take_events(&self) -> Vec<(u64, LibraryScanEvent)> {
        std::mem::take(&mut *self.events.lock().expect("scan events mutex poisoned"))
    }

    fn is_current(&self, generation: u64) -> bool {
        self.generation.load(Ordering::Acquire) == generation
    }
}

#[cfg(feature = "gui")]
fn request_library_scan(
    window: &AppWindow,
    root: std::path::PathBuf,
    persist_folder: bool,
    pending: &Rc<RefCell<Option<(std::path::PathBuf, bool)>>>,
    controller: &Arc<LibraryScanController>,
    state: &Rc<RefCell<LibraryState>>,
) {
    *pending.borrow_mut() = Some((root.clone(), persist_folder));
    window.set_library_scan_active(true);
    window.set_library_scan_indeterminate(true);
    window.set_library_scan_progress(0.0);
    window.set_library_scan_status(active_text("library_scan_searching").into());
    controller.start(root, state.borrow().clone());
}

#[cfg(feature = "gui")]
fn save_state_slot_labels(
    _path: Option<&std::path::Path>,
    _identity: Option<&str>,
) -> Vec<slint::SharedString> {
    (0..save_slots::SLOT_COUNT)
        .map(|slot| {
            if slot == 0 {
                active_slot_label(0)
            } else {
                active_slot_label(slot)
            }
            .into()
        })
        .collect()
}

#[cfg(feature = "gui")]
fn save_state_slot_occupancy(path: Option<&std::path::Path>, identity: Option<&str>) -> Vec<bool> {
    (0..save_slots::SLOT_COUNT)
        .map(|slot| {
            path.zip(identity)
                .and_then(|(path, identity)| save_slots::inspect_slot_at(path, identity, slot))
                .is_some()
        })
        .collect()
}

#[cfg(feature = "gui")]
fn refresh_save_state_ui(
    window: &AppWindow,
    console: usize,
    path: Option<&std::path::Path>,
    cart: &str,
    selected: usize,
) {
    let identity = path.and_then(save_slots::identity_from_battery);
    let selected = selected.min(save_slots::SLOT_COUNT - 1);
    let metadata = path
        .zip(identity.as_deref())
        .and_then(|(path, identity)| save_slots::inspect_slot_at(path, identity, selected));
    let details = match metadata {
        Some(ref metadata) => format!(
            "{}\n{}\n{} · {} · {} bytes · {}",
            active_slot_label(metadata.slot),
            metadata.rom_title,
            active_text("slot_occupied"),
            metadata.saved_at,
            metadata.size,
            metadata.format
        ),
        None => format!(
            "{}\n{}\n{}",
            active_slot_label(selected),
            cart,
            active_text("slot_empty")
        ),
    };
    window.set_save_state_console(console as i32);
    window.set_save_state_selected_slot(selected as i32);
    window.set_save_state_slots(slint::ModelRc::new(slint::VecModel::from(
        save_state_slot_labels(path, identity.as_deref()),
    )));
    window.set_save_state_slot_occupied(slint::ModelRc::new(slint::VecModel::from(
        save_state_slot_occupancy(path, identity.as_deref()),
    )));
    window.set_save_state_details(details.into());
    window.set_save_state_can_load(metadata.is_some());
    window.set_save_state_can_delete(metadata.is_some());
}

#[cfg(feature = "gui")]
fn refresh_save_state_if_active(
    window: &AppWindow,
    dual: &DualSystems,
    left_path: Option<&std::path::Path>,
    right_path: Option<&std::path::Path>,
    active_console: usize,
    selected_slot: usize,
    affected_console: usize,
) {
    if active_console != affected_console {
        return;
    }
    let (path, other, cart, loaded) = if affected_console == 1 {
        (
            left_path,
            right_path,
            dual.left_cart.clone(),
            dual.left_loaded,
        )
    } else {
        (
            right_path,
            left_path,
            dual.right_cart.clone(),
            dual.right_loaded,
        )
    };
    let storage = if loaded {
        path.map(|path| state_storage_path(path, affected_console, other))
    } else {
        None
    };
    refresh_save_state_ui(
        window,
        affected_console,
        storage.as_deref(),
        &cart,
        selected_slot,
    );
}

#[cfg(feature = "gui")]
fn state_storage_path(
    battery_path: &std::path::Path,
    console: usize,
    other_battery_path: Option<&std::path::Path>,
) -> std::path::PathBuf {
    if other_battery_path == Some(battery_path) {
        battery_path.with_extension(format!("instance{console}.sav"))
    } else {
        battery_path.to_path_buf()
    }
}

#[cfg(feature = "gui")]
struct DualSystems {
    left: System,
    right: System,
    cable: LinkCable,
    left_loaded: bool,
    right_loaded: bool,
    right_visible: bool,
    link_enabled: bool,
    left_cart: String,
    right_cart: String,
}

#[cfg(feature = "gui")]
fn state_label(state: foxmulator_core::EmuState) -> String {
    match state {
        foxmulator_core::EmuState::Running => active_text("app_running"),
        foxmulator_core::EmuState::Paused => active_text("app_paused"),
        foxmulator_core::EmuState::Stopped => active_text("app_stopped"),
    }
}

#[cfg(feature = "gui")]
fn visible_state_label(loaded: bool, state: foxmulator_core::EmuState) -> String {
    if loaded {
        state_label(state)
    } else {
        active_text("app_no_cartridge")
    }
}

#[cfg(feature = "gui")]
fn next_settings_open(current: bool) -> bool {
    !current
}

#[cfg(feature = "gui")]
fn replace_console(
    dual: &mut DualSystems,
    console: usize,
    source: &RomSource,
) -> Result<std::path::PathBuf, String> {
    let rom = source.load().map_err(|error| error.to_string())?;
    let mut system = System::from_rom_with_embedded_boot(rom)
        .map_err(|error| format!("integrated SameBoy boot ROM failed: {error}"))?;
    load_console_ram(&mut system.mmu, source, console);
    let save_path = console_save_path(source, console);
    let label = rom_label_source(source).unwrap_or_else(|| source.display_name());
    match console {
        1 => {
            dual.left = system;
            dual.left_cart = label;
            dual.left_loaded = true;
        }
        2 => {
            dual.right = system;
            dual.right_cart = label;
            dual.right_loaded = true;
        }
        _ => return Err(format!("invalid console {console}")),
    }
    dual.cable = LinkCable::new();
    Ok(save_path)
}

#[cfg(feature = "gui")]
fn eject_console(
    dual: &mut DualSystems,
    console: usize,
    save_path: Option<&std::path::Path>,
) -> Result<(), String> {
    let (system, loaded, cart) = match console {
        1 => (&mut dual.left, &mut dual.left_loaded, &mut dual.left_cart),
        2 => (
            &mut dual.right,
            &mut dual.right_loaded,
            &mut dual.right_cart,
        ),
        _ => return Err(format!("invalid console {console}")),
    };
    if !*loaded {
        return Err(format!("Console {console} has no cartridge"));
    }
    if let Some(path) = save_path {
        system
            .mmu
            .save_ram(path)
            .map_err(|error| format!("Unable to save Console {console} RAM: {error}"))?;
    }
    // An empty System is an explicit no-cartridge slot; no fallback ROM is run.
    *system = System::new(false);
    *loaded = false;
    *cart = "No cartridge".to_string();
    Ok(())
}

#[cfg(feature = "gui")]
#[derive(Clone)]
struct SharedSystems(Arc<Mutex<DualSystems>>);

#[cfg(feature = "gui")]
impl SharedSystems {
    fn new(dual: DualSystems) -> Self {
        Self(Arc::new(Mutex::new(dual)))
    }

    fn borrow(&self) -> MutexGuard<'_, DualSystems> {
        self.0.lock().expect("systems mutex poisoned")
    }

    fn borrow_mut(&self) -> MutexGuard<'_, DualSystems> {
        self.0.lock().expect("systems mutex poisoned")
    }
}

#[cfg(feature = "gui")]
fn set_input_button(
    systems: &SharedSystems,
    inputs: &Rc<RefCell<InputState>>,
    console: usize,
    source: InputSource,
    button: Button,
    pressed: bool,
) {
    let Some(effective_pressed) = inputs.borrow_mut().set(console, source, button, pressed) else {
        return;
    };
    let mut dual = systems.borrow_mut();
    match console {
        1 => dual.left.set_button(button, effective_pressed),
        2 => dual.right.set_button(button, effective_pressed),
        _ => {}
    }
}

#[cfg(feature = "gui")]
fn release_input_source(
    systems: &SharedSystems,
    inputs: &Rc<RefCell<InputState>>,
    console: usize,
    source: InputSource,
) {
    let releases = inputs.borrow_mut().release_source(console, source);
    let mut dual = systems.borrow_mut();
    for button in releases {
        match console {
            1 => dual.left.set_button(button, false),
            2 => dual.right.set_button(button, false),
            _ => {}
        }
    }
}

/// Convert the core framebuffer (0xAARRGGBB) into a Slint RGBA image.
#[cfg(feature = "gui")]
fn render_frame(system: &System) -> slint::Image {
    render_pixels(system.framebuffer())
}

#[cfg(feature = "gui")]
fn render_pixels(pixels: &[foxmulator_core::Pixel]) -> slint::Image {
    use foxmulator_core::Pixel;

    let mut buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(160, 144);
    for (i, Pixel(pixel)) in pixels.iter().enumerate() {
        let a = ((pixel >> 24) & 0xFF) as u8;
        let r = ((pixel >> 16) & 0xFF) as u8;
        let g = ((pixel >> 8) & 0xFF) as u8;
        let b = (pixel & 0xFF) as u8;
        let out = &mut buffer.make_mut_bytes()[i * 4..i * 4 + 4];
        out[0] = r;
        out[1] = g;
        out[2] = b;
        out[3] = a;
    }
    slint::Image::from_rgba8(buffer)
}

#[cfg(feature = "gui")]
struct FrameBuffers {
    left: Vec<foxmulator_core::Pixel>,
    right: Vec<foxmulator_core::Pixel>,
    left_generation: u64,
    right_generation: u64,
    sequence: u64,
}

#[cfg(feature = "gui")]
impl FrameBuffers {
    const WIDTH: usize = 160;
    const HEIGHT: usize = 144;
    const PIXELS: usize = Self::WIDTH * Self::HEIGHT;

    fn new(left: Vec<foxmulator_core::Pixel>, right: Vec<foxmulator_core::Pixel>) -> Self {
        Self {
            left,
            right,
            left_generation: 0,
            right_generation: 0,
            sequence: 0,
        }
    }

    fn white() -> Vec<foxmulator_core::Pixel> {
        vec![foxmulator_core::Pixel(0xFFFF_FFFF); Self::PIXELS]
    }

    fn reset_console(&mut self, console: usize, generation: u64) {
        match console {
            1 => {
                self.left = Self::white();
                self.left_generation = generation;
            }
            2 => {
                self.right = Self::white();
                self.right_generation = generation;
            }
            _ => {}
        }
    }

    fn publish_if_current(
        &mut self,
        console: usize,
        generation: u64,
        frame: Option<Vec<foxmulator_core::Pixel>>,
    ) -> bool {
        let Some(frame) = frame else {
            return false;
        };
        if frame.len() != Self::PIXELS {
            return false;
        }
        match console {
            1 if self.left_generation == generation => {
                self.left = frame;
                true
            }
            2 if self.right_generation == generation => {
                self.right = frame;
                true
            }
            _ => false,
        }
    }
}

#[cfg(feature = "gui")]
struct FrameControl {
    left_generation: AtomicU64,
    right_generation: AtomicU64,
    left_reset_generation: AtomicU64,
    right_reset_generation: AtomicU64,
}

#[cfg(feature = "gui")]
impl FrameControl {
    fn new() -> Self {
        Self {
            left_generation: AtomicU64::new(0),
            right_generation: AtomicU64::new(0),
            left_reset_generation: AtomicU64::new(0),
            right_reset_generation: AtomicU64::new(0),
        }
    }

    fn begin_reset(&self, console: usize) -> u64 {
        let (generation, reset_generation) = match console {
            1 => (&self.left_generation, &self.left_reset_generation),
            2 => (&self.right_generation, &self.right_reset_generation),
            _ => return 0,
        };
        let next = generation.fetch_add(1, Ordering::AcqRel) + 1;
        reset_generation.store(next, Ordering::Release);
        next
    }

    fn generation(&self, console: usize) -> u64 {
        match console {
            1 => self.left_generation.load(Ordering::Acquire),
            2 => self.right_generation.load(Ordering::Acquire),
            _ => 0,
        }
    }

    fn finish_reset_if_current(&self, console: usize, generation: u64) {
        let reset_generation = match console {
            1 => &self.left_reset_generation,
            2 => &self.right_reset_generation,
            _ => return,
        };
        let _ =
            reset_generation.compare_exchange(generation, 0, Ordering::AcqRel, Ordering::Acquire);
    }
}

#[cfg(feature = "gui")]
struct WorkerStats {
    completed_frames: AtomicU64,
    iterations: AtomicU64,
    published: AtomicU64,
    consumed: AtomicU64,
    replaced: AtomicU64,
    frame_pending: AtomicBool,
    cycles: AtomicU64,
    peripheral_cycles: AtomicU64,
    frame_time_sum_us: AtomicU64,
    frame_time_max_us: AtomicU64,
    emulation_us: AtomicU64,
    audio_us: AtomicU64,
    publish_us: AtomicU64,
    systems_wait_us: AtomicU64,
    frame_wait_us: AtomicU64,
    sleep_us: AtomicU64,
    quantum_us: AtomicU64,
    quantum_calls: AtomicU64,
}

#[cfg(feature = "gui")]
impl WorkerStats {
    fn new() -> Self {
        Self {
            completed_frames: AtomicU64::new(0),
            iterations: AtomicU64::new(0),
            published: AtomicU64::new(0),
            consumed: AtomicU64::new(0),
            replaced: AtomicU64::new(0),
            frame_pending: AtomicBool::new(false),
            cycles: AtomicU64::new(0),
            peripheral_cycles: AtomicU64::new(0),
            frame_time_sum_us: AtomicU64::new(0),
            frame_time_max_us: AtomicU64::new(0),
            emulation_us: AtomicU64::new(0),
            audio_us: AtomicU64::new(0),
            publish_us: AtomicU64::new(0),
            systems_wait_us: AtomicU64::new(0),
            frame_wait_us: AtomicU64::new(0),
            sleep_us: AtomicU64::new(0),
            quantum_us: AtomicU64::new(0),
            quantum_calls: AtomicU64::new(0),
        }
    }
}

#[cfg(feature = "gui")]
fn run_emulation_worker(
    systems: SharedSystems,
    frames: Arc<Mutex<FrameBuffers>>,
    frame_control: Arc<FrameControl>,
    stats: Arc<WorkerStats>,
    stop: Arc<AtomicBool>,
    speed: Arc<AtomicU32>,
    #[cfg(feature = "audio")] audio: Option<AudioProducer>,
    #[cfg(feature = "audio")] audio_mix: Arc<Mutex<AudioMixSettings>>,
) {
    const FRAME_T_CYCLES: u32 = 70_224;
    const NATIVE_FRAME_NS: u64 = 1_000_000_000 * FRAME_T_CYCLES as u64 / 4_194_304;
    let mut deadline = std::time::Instant::now();
    let mut next_visual_publish = std::time::Instant::now();
    let visual_period = std::time::Duration::from_nanos(1_000_000_000 / 60);
    let quantum = cooperative_quantum();
    #[cfg(feature = "audio")]
    let mut previous_speed = 1u32;

    while !stop.load(Ordering::Acquire) {
        let frame_started = std::time::Instant::now();
        let left_generation = frame_control.generation(1);
        let right_generation = frame_control.generation(2);
        let systems_wait_started = std::time::Instant::now();
        let mut dual = systems.borrow_mut();
        stats.systems_wait_us.fetch_add(
            systems_wait_started.elapsed().as_micros() as u64,
            Ordering::Relaxed,
        );
        let emulation_started = std::time::Instant::now();
        let mut left_cycles = 0u32;
        let mut right_cycles = 0u32;
        let left_peripheral_start = dual.left.mmu.peripheral_ticks;
        let right_peripheral_start = dual.right.mmu.peripheral_ticks;
        while (dual.left_loaded
            && dual.left.state == foxmulator_core::EmuState::Running
            && dual
                .left
                .mmu
                .peripheral_ticks
                .wrapping_sub(left_peripheral_start)
                < u64::from(FRAME_T_CYCLES))
            || (dual.right_loaded
                && dual.right.state == foxmulator_core::EmuState::Running
                && dual
                    .right
                    .mmu
                    .peripheral_ticks
                    .wrapping_sub(right_peripheral_start)
                    < u64::from(FRAME_T_CYCLES))
        {
            if dual.left_loaded
                && dual.left.state == foxmulator_core::EmuState::Running
                && dual
                    .left
                    .mmu
                    .peripheral_ticks
                    .wrapping_sub(left_peripheral_start)
                    < u64::from(FRAME_T_CYCLES)
            {
                left_cycles = left_cycles.saturating_add(dual.left.step_instruction());
            }
            if dual.right_loaded
                && dual.right.state == foxmulator_core::EmuState::Running
                && dual
                    .right
                    .mmu
                    .peripheral_ticks
                    .wrapping_sub(right_peripheral_start)
                    < u64::from(FRAME_T_CYCLES)
            {
                right_cycles = right_cycles.saturating_add(dual.right.step_instruction());
            }
            if dual.left_loaded && dual.right_loaded && dual.link_enabled {
                let DualSystems {
                    left, right, cable, ..
                } = &mut *dual;
                cable.pump(left, right);
            }
            if left_cycles == 0 && right_cycles == 0 {
                break;
            }
        }
        let left_peripheral_cycles = dual
            .left
            .mmu
            .peripheral_ticks
            .wrapping_sub(left_peripheral_start);
        stats.emulation_us.fetch_add(
            emulation_started.elapsed().as_micros() as u64,
            Ordering::Relaxed,
        );
        let active_speed = speed.load(Ordering::Acquire).clamp(1, 12);
        #[cfg(feature = "audio")]
        if active_speed != previous_speed && active_speed == 1 {
            if let Some(audio) = audio.as_ref() {
                audio.clear();
            }
        }
        #[cfg(feature = "audio")]
        if active_speed == 1 {
            let audio_started = std::time::Instant::now();
            if let Some(audio) = audio.as_ref() {
                let settings = *audio_mix.lock().expect("audio mix mutex poisoned");
                play_audio(&mut dual, audio, settings);
            }
            stats.audio_us.fetch_add(
                audio_started.elapsed().as_micros() as u64,
                Ordering::Relaxed,
            );
        }
        let left_ready = dual.left.ppu.take_frame_ready();
        let right_ready = dual.right.ppu.take_frame_ready();
        let visual_due = std::time::Instant::now() >= next_visual_publish;
        let left = (visual_due && left_ready).then(|| dual.left.framebuffer().to_vec());
        let right = (visual_due && right_ready).then(|| dual.right.framebuffer().to_vec());
        if visual_due {
            next_visual_publish += visual_period;
            if next_visual_publish < std::time::Instant::now() {
                next_visual_publish = std::time::Instant::now() + visual_period;
            }
        }
        drop(dual);
        let mut published = false;
        {
            let publish_started = std::time::Instant::now();
            let frame_wait_started = std::time::Instant::now();
            let mut output = frames.lock().expect("frame mutex poisoned");
            stats.frame_wait_us.fetch_add(
                frame_wait_started.elapsed().as_micros() as u64,
                Ordering::Relaxed,
            );
            if frame_control.generation(1) == left_generation
                && output.publish_if_current(1, left_generation, left)
            {
                frame_control.finish_reset_if_current(1, left_generation);
                published = true;
            }
            if frame_control.generation(2) == right_generation
                && output.publish_if_current(2, right_generation, right)
            {
                frame_control.finish_reset_if_current(2, right_generation);
                published = true;
            }
            if published {
                output.sequence = output.sequence.wrapping_add(1);
            }
            stats.publish_us.fetch_add(
                publish_started.elapsed().as_micros() as u64,
                Ordering::Relaxed,
            );
        }
        if published {
            if stats.frame_pending.swap(true, Ordering::AcqRel) {
                stats.replaced.fetch_add(1, Ordering::Relaxed);
            }
            stats.published.fetch_add(1, Ordering::Relaxed);
        }
        let elapsed_us = frame_started.elapsed().as_micros() as u64;
        stats.iterations.fetch_add(1, Ordering::Relaxed);
        if left_ready || right_ready {
            stats.completed_frames.fetch_add(1, Ordering::Relaxed);
        }
        stats
            .cycles
            .fetch_add(left_cycles as u64, Ordering::Relaxed);
        stats
            .peripheral_cycles
            .fetch_add(left_peripheral_cycles, Ordering::Relaxed);
        stats
            .frame_time_sum_us
            .fetch_add(elapsed_us, Ordering::Relaxed);
        stats
            .frame_time_max_us
            .fetch_max(elapsed_us, Ordering::Relaxed);

        #[cfg(feature = "audio")]
        {
            previous_speed = active_speed;
        }
        let frame_period =
            std::time::Duration::from_nanos(NATIVE_FRAME_NS / u64::from(active_speed));
        deadline += frame_period;
        if let Some(remaining) = deadline.checked_duration_since(std::time::Instant::now()) {
            let sleep_started = std::time::Instant::now();
            std::thread::sleep(remaining);
            stats.sleep_us.fetch_add(
                sleep_started.elapsed().as_micros() as u64,
                Ordering::Relaxed,
            );
        } else {
            deadline = std::time::Instant::now();
            // When the requested speed is beyond the machine's throughput,
            // yield between complete emulation frames so the Slint event loop
            // remains schedulable. This is not a timing source or a busy wait.
        }
        if active_speed > 1 {
            // Do not let an accelerated worker monopolize the scheduler when
            // emulation itself consumes the complete accelerated budget.
            let quantum_started = std::time::Instant::now();
            std::thread::sleep(quantum);
            stats.quantum_us.fetch_add(
                quantum_started.elapsed().as_micros() as u64,
                Ordering::Relaxed,
            );
            stats.quantum_calls.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[cfg(feature = "gui")]
fn shell_theme(index: usize) -> (slint::Color, slint::Color, &'static str) {
    const THEMES: [(&str, &str, &str); 11] = [
        ("#9161b6", "#d8b4ff", "Atómico"),
        ("#6256b5", "#b6a5ff", "Uva"),
        ("#a1c9d6", "#e2f5ff", "Hielo"),
        ("#147c88", "#8bf1e5", "Océano"),
        ("#aa4a71", "#ffb6cf", "Cereza"),
        ("#289b8d", "#b2fbe9", "Turquesa"),
        ("#cc8830", "#ffe1a2", "Solar"),
        ("#757582", "#c9cee3", "Humo"),
        ("#bbc4d2", "#f0f7ff", "Cristal"),
        ("#4d5769", "#bbc8dd", "Grafito"),
        ("#608c48", "#d2f6a3", "Lima"),
    ];
    let (shell, accent, name) = THEMES[index % THEMES.len()];
    (parse_hex_color(shell), parse_hex_color(accent), name)
}

#[cfg(feature = "gui")]
fn parse_hex_color(value: &str) -> slint::Color {
    let raw = value.trim().trim_start_matches('#');
    if raw.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&raw[0..2], 16),
            u8::from_str_radix(&raw[2..4], 16),
            u8::from_str_radix(&raw[4..6], 16),
        ) {
            return slint::Color::from_rgb_u8(r, g, b);
        }
    }
    slint::Color::from_rgb_u8(0x91, 0x61, 0xb6)
}

#[cfg(feature = "gui")]
fn is_hex_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value.as_bytes()[1..]
            .iter()
            .all(|byte| byte.is_ascii_hexdigit())
}

#[cfg(all(feature = "gui", feature = "audio"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AudioMixSettings {
    master_volume: u8,
    console1_volume: u8,
    console2_volume: u8,
    console1_muted: bool,
    console2_muted: bool,
}

#[cfg(all(feature = "gui", feature = "audio"))]
impl From<&InputConfig> for AudioMixSettings {
    fn from(config: &InputConfig) -> Self {
        Self {
            master_volume: config.master_volume.clamp(0, 100),
            console1_volume: config.console1_volume.clamp(0, 100),
            console2_volume: config.console2_volume.clamp(0, 100),
            console1_muted: config.console1_muted,
            console2_muted: config.console2_muted,
        }
    }
}

#[cfg(all(feature = "gui", feature = "audio"))]
fn effective_audio_settings(config: &InputConfig, runtime_mute: [bool; 2]) -> AudioMixSettings {
    let mut settings = AudioMixSettings::from(config);
    settings.console1_muted |= runtime_mute[0];
    settings.console2_muted |= runtime_mute[1];
    settings
}

#[cfg(all(feature = "gui", feature = "audio"))]
struct PcmRing {
    slots: Box<[UnsafeCell<f32>]>,
    capacity: usize,
    head: AtomicUsize,
    tail: AtomicUsize,
}

#[cfg(all(feature = "gui", feature = "audio"))]
unsafe impl Send for PcmRing {}
#[cfg(all(feature = "gui", feature = "audio"))]
unsafe impl Sync for PcmRing {}

#[cfg(all(feature = "gui", feature = "audio"))]
impl PcmRing {
    const CAPACITY: usize = 8192;

    fn new() -> Self {
        let slots = (0..Self::CAPACITY)
            .map(|_| UnsafeCell::new(0.0))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        Self {
            slots,
            capacity: Self::CAPACITY,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }

    fn len(&self) -> usize {
        self.tail
            .load(Ordering::Acquire)
            .saturating_sub(self.head.load(Ordering::Acquire))
    }

    fn push(&self, samples: &[f32]) -> bool {
        let tail = self.tail.load(Ordering::Relaxed);
        let head = self.head.load(Ordering::Acquire);
        if samples.len() > self.capacity.saturating_sub(tail.saturating_sub(head)) {
            return false;
        }
        for (index, sample) in samples.iter().copied().enumerate() {
            // SAFETY: the single producer owns the range between tail and head.
            unsafe { *self.slots[(tail + index) % self.capacity].get() = sample };
        }
        self.tail
            .store(tail.saturating_add(samples.len()), Ordering::Release);
        true
    }

    fn pop_into(&self, output: &mut [f32]) -> usize {
        let head = self.head.load(Ordering::Relaxed);
        let tail = self.tail.load(Ordering::Acquire);
        let count = output.len().min(tail.saturating_sub(head));
        for (index, sample) in output.iter_mut().take(count).enumerate() {
            // SAFETY: the single consumer owns the range between head and tail.
            *sample = unsafe { *self.slots[(head + index) % self.capacity].get() };
        }
        self.head
            .store(head.saturating_add(count), Ordering::Release);
        count
    }
}

#[cfg(all(feature = "gui", feature = "audio"))]
#[derive(Clone)]
struct AudioProducer {
    ring: Arc<PcmRing>,
    dropped_chunks: Arc<AtomicU64>,
    max_queued_samples: Arc<AtomicUsize>,
}

#[cfg(all(feature = "gui", feature = "audio"))]
struct AudioSink {
    producer: AudioProducer,
    underruns: Arc<AtomicU64>,
    written_samples: Arc<AtomicU64>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

#[cfg(all(feature = "gui", feature = "audio"))]
impl AudioSink {
    fn new() -> Option<Self> {
        let mut process = std::process::Command::new("paplay")
            .args([
                "--raw",
                "--format=float32le",
                "--rate=44100",
                "--channels=2",
                "--stream-name=FoxMulator",
            ])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .ok()?;
        let mut input = process.stdin.take()?;
        let ring = Arc::new(PcmRing::new());
        let dropped_chunks = Arc::new(AtomicU64::new(0));
        let underruns = Arc::new(AtomicU64::new(0));
        let written_samples = Arc::new(AtomicU64::new(0));
        let max_queued_samples = Arc::new(AtomicUsize::new(0));
        let stop = Arc::new(AtomicBool::new(false));
        let thread_ring = Arc::clone(&ring);
        let thread_underruns = Arc::clone(&underruns);
        let thread_written = Arc::clone(&written_samples);
        let thread_stop = Arc::clone(&stop);
        let thread = std::thread::spawn(move || {
            let mut samples = [0.0f32; 2048];
            let mut bytes = [0u8; 2048 * 4];
            let mut started = false;
            let mut starved = false;
            let mut last_samples = std::time::Instant::now();
            const TARGET_SAMPLES: usize = 3_528; // 40 ms at 44.1 kHz stereo.
            while !thread_stop.load(Ordering::Acquire) {
                if !started && thread_ring.len() < TARGET_SAMPLES {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                    continue;
                }
                let count = thread_ring.pop_into(&mut samples);
                if count == 0 {
                    if started
                        && !starved
                        && last_samples.elapsed() >= std::time::Duration::from_millis(30)
                    {
                        thread_underruns.fetch_add(1, Ordering::Relaxed);
                        starved = true;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(1));
                    continue;
                }
                started = true;
                starved = false;
                last_samples = std::time::Instant::now();
                for (index, sample) in samples.iter().copied().take(count).enumerate() {
                    bytes[index * 4..index * 4 + 4].copy_from_slice(&sample.to_le_bytes());
                }
                if input.write_all(&bytes[..count * 4]).is_err() {
                    break;
                }
                thread_written.fetch_add(count as u64, Ordering::Relaxed);
            }
            let _ = process.kill();
        });
        Some(Self {
            producer: AudioProducer {
                ring,
                dropped_chunks,
                max_queued_samples,
            },
            underruns,
            written_samples,
            stop,
            thread: Some(thread),
        })
    }

    fn producer(&self) -> AudioProducer {
        self.producer.clone()
    }
}

#[cfg(all(feature = "gui", feature = "audio"))]
impl AudioProducer {
    fn submit(&self, samples: &[f32]) {
        if !self.ring.push(samples) {
            self.dropped_chunks.fetch_add(1, Ordering::Relaxed);
            return;
        }
        let queued = self.ring.len();
        self.max_queued_samples.fetch_max(queued, Ordering::Relaxed);
    }

    fn clear(&self) {
        let tail = self.ring.tail.load(Ordering::Acquire);
        self.ring.head.store(tail, Ordering::Release);
    }
}

#[cfg(all(feature = "gui", feature = "audio"))]
impl Drop for AudioSink {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(all(feature = "gui", feature = "audio"))]
fn mix_pcm_samples(left: &[f32], right: &[f32], settings: AudioMixSettings) -> Vec<f32> {
    let left_gain = if settings.console1_muted {
        0.0
    } else {
        f32::from(settings.console1_volume) / 100.0
    };
    let right_gain = if settings.console2_muted {
        0.0
    } else {
        f32::from(settings.console2_volume) / 100.0
    };
    let master_gain = f32::from(settings.master_volume) / 100.0;
    let len = left.len().max(right.len());
    (0..len)
        .map(|index| {
            let mixed = (left.get(index).copied().unwrap_or(0.0) * left_gain
                + right.get(index).copied().unwrap_or(0.0) * right_gain)
                * 0.5;
            (mixed * master_gain).clamp(-1.0, 1.0)
        })
        .collect()
}

#[cfg(all(feature = "gui", feature = "audio"))]
fn play_audio(dual: &mut DualSystems, producer: &AudioProducer, settings: AudioMixSettings) {
    let left = dual.left.take_audio_samples();
    let right = if dual.right_loaded {
        dual.right.take_audio_samples()
    } else {
        Vec::new()
    };
    if left.is_empty() && right.is_empty() {
        return;
    }
    let mixed = mix_pcm_samples(&left, &right, settings);
    producer.submit(&mixed);
}

#[cfg(feature = "gui")]
fn main() -> Result<(), slint::PlatformError> {
    let cli = Cli::parse();
    let launch = match validate_cli(cli) {
        Ok(launch) => launch,
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(2);
        }
    };
    let (initial_system, initial_save_path) = load_system(launch.rom1.as_ref());
    let left_rom_path = launch.rom1.clone();
    let right_rom_path = launch.rom2.clone();
    let initial_left_cart = left_rom_path
        .as_ref()
        .and_then(rom_label_source)
        .unwrap_or_else(|| "No cartridge".to_string());
    let (right, initial_right_save_path, initial_right_cart) =
        if let Some(path) = right_rom_path.as_ref() {
            if let Ok(rom) = path.load() {
                let mut system = System::from_rom_with_embedded_boot(rom)
                    .unwrap_or_else(|error| panic!("integrated SameBoy boot ROM failed: {error}"));
                load_console_ram(&mut system.mmu, path, 2);
                if let Some(label) = rom_label_source(path) {
                    (system, Some(console_save_path(path, 2)), label)
                } else {
                    (
                        System::from_rom(fallback_rom()),
                        None,
                        "No cartridge".to_string(),
                    )
                }
            } else {
                (
                    System::from_rom(fallback_rom()),
                    None,
                    "No cartridge".to_string(),
                )
            }
        } else {
            (
                System::from_rom(fallback_rom()),
                None,
                "No cartridge".to_string(),
            )
        };
    let systems = SharedSystems::new(DualSystems {
        left: initial_system,
        right,
        cable: LinkCable::new(),
        left_loaded: left_rom_path.is_some() && initial_left_cart != "No cartridge",
        right_loaded: right_rom_path.is_some() && initial_right_cart != "No cartridge",
        right_visible: false,
        link_enabled: launch.link,
        left_cart: initial_left_cart,
        right_cart: initial_right_cart,
    });
    let save_path = Rc::new(RefCell::new(initial_save_path));
    let right_save_path: Rc<RefCell<Option<std::path::PathBuf>>> =
        Rc::new(RefCell::new(initial_right_save_path));
    let selected_save_console = Rc::new(RefCell::new(1usize));
    let selected_save_slot = Rc::new(RefCell::new(0usize));
    {
        let dual = systems.borrow_mut();
        if let (Some(path), Some(identity)) = (
            save_path.borrow().as_ref(),
            left_rom_path.as_ref().and_then(rom_identity_source),
        ) {
            let _ = save_slots::migrate_legacy_state(&dual.left, path, &identity, &dual.left_cart);
        }
        if let (Some(path), Some(identity)) = (
            right_save_path.borrow().as_ref(),
            right_rom_path.as_ref().and_then(rom_identity_source),
        ) {
            let _ =
                save_slots::migrate_legacy_state(&dual.right, path, &identity, &dual.right_cart);
        }
    }
    #[cfg(all(feature = "gui", feature = "audio"))]
    let audio_sink = AudioSink::new();

    let input_config = Rc::new(RefCell::new(load_input_config()));
    let initial_speed = std::env::var("FOXMOULATOR_BENCH_SPEED")
        .ok()
        .and_then(|value| value.parse::<i32>().ok())
        .and_then(valid_boost_speed)
        .unwrap_or(1);
    let emulation_speed = Arc::new(AtomicU32::new(initial_speed));
    select_renderer(&input_config.borrow().renderer)?;
    let window = AppWindow::new()?;
    if launch.fullscreen {
        apply_fullscreen(&window);
    }
    let language_manager = Rc::new(RefCell::new(i18n::LanguageManager::from_environment()));
    ACTIVE_LANGUAGE.with(|active| *active.borrow_mut() = Some(Rc::clone(&language_manager)));
    {
        let configured_language = input_config.borrow().language.clone();
        let selected = {
            let mut manager = language_manager.borrow_mut();
            manager.select_id(&configured_language)
        };
        if !selected {
            let mut config = input_config.borrow_mut();
            config.language = i18n::SPANISH_ID.into();
            save_input_config(&config);
        }
        {
            let manager = language_manager.borrow();
            let snapshot = language_snapshot(&manager);
            drop(manager);
            apply_language(&window, snapshot);
        }
    }
    #[cfg(feature = "audio")]
    let runtime_mute = Rc::new(RefCell::new([launch.mute, launch.mute]));
    #[cfg(feature = "audio")]
    let audio_mix = Arc::new(Mutex::new(effective_audio_settings(
        &input_config.borrow(),
        [launch.mute, launch.mute],
    )));
    {
        let config = input_config.borrow();
        let right_loaded = systems.borrow().right_loaded;
        systems.borrow_mut().right_visible =
            right_loaded && (config.second_console || right_rom_path.is_some() || launch.link);
        window.set_bindings_console_1(bindings_text(&config.console1).into());
        window.set_bindings_console_2(bindings_text(&config.console2).into());
        window.set_show_virtual_controls(config.show_virtual_controls);
        window.set_lcd_grid_enabled(config.lcd_grid);
        window.set_screen_scale(config.screen_scale.clamp(1, 5) as i32);
        window.set_boost_label(format!("{}×", config.boost_speed.clamp(1, 4)).into());
        window.set_renderer_selection(config.renderer.clone().into());
        window.set_boost_default_speed(config.boost_speed.clamp(1, 4) as i32);
        window.set_auto_scale(config.auto_scale);
        window.set_translucent_shell(config.translucent_shell);
        window.set_translucent_tint(config.translucent_tint.clamp(25, 90) as i32);
        refresh_display_tint(&window);
        let dual = systems.borrow();
        window.set_second_console_enabled(dual.right_visible);
        window.set_link_enabled(dual.link_enabled);
        refresh_save_state_if_active(
            &window,
            &dual,
            save_path.borrow().as_deref(),
            right_save_path.borrow().as_deref(),
            1,
            0,
            1,
        );
        window.set_cart_left(dual.left_cart.clone().into());
        window.set_cart_right(dual.right_cart.clone().into());
        window.set_state_left(visible_state_label(dual.left_loaded, dual.left.state).into());
        window.set_state_right(visible_state_label(dual.right_loaded, dual.right.state).into());
        window.set_left_loaded(dual.left_loaded);
        window.set_right_loaded(dual.right_loaded);
        window.set_master_volume(config.master_volume.clamp(0, 100) as i32);
        window.set_console1_volume(config.console1_volume.clamp(0, 100) as i32);
        window.set_console2_volume(config.console2_volume.clamp(0, 100) as i32);
        window.set_console1_muted(config.console1_muted || launch.mute);
        window.set_console2_muted(config.console2_muted || launch.mute);
    }
    let language_window = window.as_weak();
    let language_manager_select = Rc::clone(&language_manager);
    let language_config = Rc::clone(&input_config);
    let language_systems = systems.clone();
    let language_left_path = Rc::clone(&save_path);
    let language_right_path = Rc::clone(&right_save_path);
    let language_selected_console = Rc::clone(&selected_save_console);
    let language_selected_slot = Rc::clone(&selected_save_slot);
    let language_library_state = Rc::new(RefCell::new(Option::<Rc<RefCell<LibraryState>>>::None));
    let language_library_index = Rc::new(RefCell::new(
        Option::<Rc<RefCell<Vec<LibrarySearchEntry>>>>::None,
    ));
    let language_library_state_ref = Rc::clone(&language_library_state);
    let language_library_index_ref = Rc::clone(&language_library_index);
    window.on_select_language(move |name| {
        let selected_id = {
            let mut manager = language_manager_select.borrow_mut();
            if manager.select_name(name.as_str()) {
                Some(manager.selected_id().to_owned())
            } else {
                None
            }
        };
        if let Some(selected_id) = selected_id {
            {
                let mut config = language_config.borrow_mut();
                config.language = selected_id;
                save_input_config(&config);
            }
            if let Some(window) = language_window.upgrade() {
                {
                    let manager = language_manager_select.borrow();
                    let snapshot = language_snapshot(&manager);
                    drop(manager);
                    apply_language(&window, snapshot);
                }
                let dual = language_systems.borrow();
                refresh_save_state_if_active(
                    &window,
                    &dual,
                    language_left_path.borrow().as_deref(),
                    language_right_path.borrow().as_deref(),
                    *language_selected_console.borrow(),
                    *language_selected_slot.borrow(),
                    *language_selected_console.borrow(),
                );
                if let (Some(state), Some(index)) = (
                    language_library_state_ref.borrow().clone(),
                    language_library_index_ref.borrow().clone(),
                ) {
                    let state = state.borrow();
                    let index = index.borrow();
                    set_library_models(&window, &index, &state);
                }
            }
        }
    });
    let reload_window = window.as_weak();
    let reload_manager = Rc::clone(&language_manager);
    let reload_systems = systems.clone();
    let reload_left_path = Rc::clone(&save_path);
    let reload_right_path = Rc::clone(&right_save_path);
    let reload_selected_console = Rc::clone(&selected_save_console);
    let reload_selected_slot = Rc::clone(&selected_save_slot);
    let reload_library_state_ref = Rc::clone(&language_library_state);
    let reload_library_index_ref = Rc::clone(&language_library_index);
    window.on_reload_languages(move || {
        {
            let mut manager = reload_manager.borrow_mut();
            manager.reload();
        }
        if let Some(window) = reload_window.upgrade() {
            {
                let manager = reload_manager.borrow();
                let snapshot = language_snapshot(&manager);
                drop(manager);
                apply_language(&window, snapshot);
            }
            let dual = reload_systems.borrow();
            refresh_save_state_if_active(
                &window,
                &dual,
                reload_left_path.borrow().as_deref(),
                reload_right_path.borrow().as_deref(),
                *reload_selected_console.borrow(),
                *reload_selected_slot.borrow(),
                *reload_selected_console.borrow(),
            );
            if let (Some(state), Some(index)) = (
                reload_library_state_ref.borrow().clone(),
                reload_library_index_ref.borrow().clone(),
            ) {
                let state = state.borrow();
                let index = index.borrow();
                set_library_models(&window, &index, &state);
            }
        }
    });
    let layout_request = Rc::new(RefCell::new(LayoutRequest::default()));
    apply_layout_size(
        &window,
        &layout_request,
        window.get_second_console_enabled(),
        window.get_show_virtual_controls(),
        true,
    );
    let initial_frames = {
        let dual = systems.borrow();
        FrameBuffers::new(
            dual.left.framebuffer().to_vec(),
            dual.right.framebuffer().to_vec(),
        )
    };
    let frames = Arc::new(Mutex::new(initial_frames));
    let frame_control = Arc::new(FrameControl::new());
    let pending_rebind: Rc<RefCell<Option<(usize, String)>>> = Rc::new(RefCell::new(None));
    let rebind_blocked_key: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let active_inputs = Rc::new(RefCell::new(InputState::default()));
    let library_state = Rc::new(RefCell::new(load_library_state()));
    let preferences = Rc::new(RefCell::new(load_preferences()));
    let selected_library_path: Rc<RefCell<Option<RomSource>>> = Rc::new(RefCell::new(None));
    let editing_library_path: Rc<RefCell<Option<RomSource>>> = Rc::new(RefCell::new(None));
    let library_scan_controller = Arc::new(LibraryScanController::new());
    let pending_library_scan: Rc<RefCell<Option<(std::path::PathBuf, bool)>>> =
        Rc::new(RefCell::new(None));
    let editing_cover_file: Rc<RefCell<Option<std::path::PathBuf>>> = Rc::new(RefCell::new(None));
    let editing_cover_reset = Rc::new(RefCell::new(false));
    #[cfg(all(feature = "gui", feature = "gamepad"))]
    let mut gamepads = {
        let config = input_config.borrow();
        let order = [
            config.console1.gamepad_slot.saturating_sub(1).min(1) as usize,
            config.console2.gamepad_slot.saturating_sub(1).min(1) as usize,
        ];
        GamepadRouter::new(order)
    };
    let library_root = launch
        .library
        .clone()
        .or_else(|| std::env::var_os("FOX_LIBRARY_DIR").map(std::path::PathBuf::from))
        .or_else(|| {
            preferences
                .borrow()
                .library_folder
                .as_ref()
                .map(std::path::PathBuf::from)
        })
        .or_else(|| {
            launch
                .rom1
                .as_ref()
                .and_then(|source| source.physical_path().parent().map(Into::into))
        })
        .unwrap_or_else(|| std::path::PathBuf::from("test-roms"));
    // The window starts with the last valid in-memory library (empty on a cold
    // start). The configured folder is scanned only after the event loop can
    // draw and process input.
    let library_paths = Rc::new(RefCell::new(Vec::<RomSource>::new()));
    let library_index = Rc::new(RefCell::new(build_library_search_index(
        &library_paths.borrow(),
        &library_state.borrow(),
    )));
    *language_library_state.borrow_mut() = Some(Rc::clone(&library_state));
    *language_library_index.borrow_mut() = Some(Rc::clone(&library_index));
    {
        let state = library_state.borrow();
        set_library_models(&window, &library_index.borrow(), &state);
        window.set_recent_games(slint::ModelRc::new(slint::VecModel::from(recent_labels(
            &state,
        ))));
    }
    if let Some(folder) = preferences.borrow().library_folder.clone() {
        if !std::path::Path::new(&folder).is_dir() {
            window.set_status(
                active_format(
                    "status_library_unavailable",
                    &[("path", &folder.to_string())],
                )
                .into(),
            );
        }
    }
    let mut shell_index = 0usize;
    {
        let config = input_config.borrow();
        window.set_shell_color(parse_hex_color(&config.shell_color));
        window.set_accent_color(parse_hex_color(&config.accent_color));
        window.set_shell_color_hex(config.shell_color.clone().into());
        window.set_accent_color_hex(config.accent_color.clone().into());
    }
    window.set_status(
        if systems.borrow().left_loaded {
            "Running"
        } else {
            "Esperando cartucho en Consola 1"
        }
        .into(),
    );
    {
        let dual = systems.borrow();
        window.set_frame_left(render_frame(&dual.left));
        window.set_frame_right(render_frame(&dual.right));
    }

    let pressed = {
        let systems = systems.clone();
        let inputs = Rc::clone(&active_inputs);
        move |console: i32, name: slint::SharedString| {
            if let Some(button) = button_from_name(&name) {
                set_input_button(
                    &systems,
                    &inputs,
                    console as usize,
                    InputSource::Virtual,
                    button,
                    true,
                );
            }
        }
    };
    window.on_console_button_pressed(pressed);

    let released = {
        let systems = systems.clone();
        let inputs = Rc::clone(&active_inputs);
        move |console: i32, name: slint::SharedString| {
            if let Some(button) = button_from_name(&name) {
                set_input_button(
                    &systems,
                    &inputs,
                    console as usize,
                    InputSource::Virtual,
                    button,
                    false,
                );
            }
        }
    };
    window.on_console_button_released(released);

    let key_system = systems.clone();
    let key_inputs = Rc::clone(&active_inputs);
    let key_config = Rc::clone(&input_config);
    let key_pending = Rc::clone(&pending_rebind);
    let key_blocked = Rc::clone(&rebind_blocked_key);
    let key_window = window.as_weak();
    let quick_held = Rc::new(RefCell::new(std::collections::HashSet::<String>::new()));
    let quick_held_keys = Rc::clone(&quick_held);
    window.on_key_input(move |key, pressed| {
        let blocked_key = key_blocked.borrow().clone();
        if let Some(blocked) = blocked_key {
            if !pressed && blocked == key.as_str() {
                *key_blocked.borrow_mut() = None;
            }
            return;
        }
        if pressed {
            let pending = key_pending.borrow_mut().take();
            if let Some((console, action)) = pending {
                *key_blocked.borrow_mut() = Some(key.to_string());
                release_input_source(&key_system, &key_inputs, console, InputSource::Keyboard);
                release_input_source(&key_system, &key_inputs, console, InputSource::Virtual);
                if key.as_str() == "Escape" {
                    if let Some(window) = key_window.upgrade() {
                        window.set_status(active_text("status_capture_cancelled").into());
                    }
                    return;
                }
                let result = {
                    let mut config = key_config.borrow_mut();
                    match apply_rebind(&mut config, console, &action, &key) {
                        Ok(()) => {
                            save_input_config(&config);
                            Ok((
                                bindings_text(&config.console1),
                                bindings_text(&config.console2),
                            ))
                        }
                        Err(error) => Err(error),
                    }
                };
                let (bindings1, bindings2) = match result {
                    Ok(bindings) => bindings,
                    Err(RebindError::Conflict(used_console, used_action)) => {
                        *key_pending.borrow_mut() = Some((console, action));
                        if let Some(window) = key_window.upgrade() {
                            window.set_status(
                                format!(
                                    "Tecla ya asignada a C{used_console} {used_action}; elegí otra"
                                )
                                .into(),
                            );
                        }
                        return;
                    }
                    Err(RebindError::InvalidAction) => {
                        if let Some(window) = key_window.upgrade() {
                            window.set_status(active_text("status_invalid_key").into());
                        }
                        return;
                    }
                };
                if let Some(window) = key_window.upgrade() {
                    window.set_bindings_console_1(bindings1.into());
                    window.set_bindings_console_2(bindings2.into());
                    window.set_status(active_text("status_controls_saved").into());
                }
                return;
            }
            let quick = {
                let config = key_config.borrow();
                quick_action_for_key(&config, &key)
            };
            if let Some((console, save)) = quick {
                if quick_held_keys.borrow_mut().insert(key.to_string()) {
                    if let Some(window) = key_window.upgrade() {
                        if save {
                            window.invoke_save_state_for_console(console as i32);
                        } else {
                            window.invoke_load_state_for_console(console as i32);
                        }
                    }
                }
                return;
            }
        } else {
            quick_held_keys.borrow_mut().remove(key.as_str());
        }
        let binding = {
            let config = key_config.borrow();
            profile_for_key(&config, &key)
        };
        let Some((console, button)) = binding else {
            return;
        };
        set_input_button(
            &key_system,
            &key_inputs,
            console,
            InputSource::Keyboard,
            button,
            pressed,
        );
    });

    let release_virtual_systems = systems.clone();
    let virtual_input_state = Rc::clone(&active_inputs);
    window.on_release_virtual_inputs(move || {
        release_input_source(
            &release_virtual_systems,
            &virtual_input_state,
            1,
            InputSource::Virtual,
        );
        release_input_source(
            &release_virtual_systems,
            &virtual_input_state,
            2,
            InputSource::Virtual,
        );
    });

    let release_keyboard_systems = systems.clone();
    let release_keyboard_inputs = Rc::clone(&active_inputs);
    window.on_release_keyboard_inputs(move || {
        release_input_source(
            &release_keyboard_systems,
            &release_keyboard_inputs,
            1,
            InputSource::Keyboard,
        );
        release_input_source(
            &release_keyboard_systems,
            &release_keyboard_inputs,
            2,
            InputSource::Keyboard,
        );
    });

    let tab_window = window.as_weak();
    window.on_select_control_tab(move |tab| {
        if let Some(window) = tab_window.upgrade() {
            window.set_control_tab(tab);
        }
    });

    let rebind_pending = Rc::clone(&pending_rebind);
    let rebind_blocked = Rc::clone(&rebind_blocked_key);
    let rebind_systems = systems.clone();
    let rebind_inputs = Rc::clone(&active_inputs);
    let rebind_window = window.as_weak();
    window.on_begin_rebind(move |action| {
        let mut parts = action.splitn(2, ':');
        let Some(console) = parts.next().and_then(|value| value.parse::<usize>().ok()) else {
            return;
        };
        let Some(action) = parts.next() else { return };
        if (1..=2).contains(&console) {
            release_input_source(
                &rebind_systems,
                &rebind_inputs,
                console,
                InputSource::Keyboard,
            );
            release_input_source(
                &rebind_systems,
                &rebind_inputs,
                console,
                InputSource::Virtual,
            );
            *rebind_blocked.borrow_mut() = None;
            *rebind_pending.borrow_mut() = Some((console, action.to_string()));
            if let Some(window) = rebind_window.upgrade() {
                window.set_status(
                    format!("Presioná una tecla para C{console} {action} (Escape cancela)").into(),
                );
            }
        }
    });

    let restore_window = window.as_weak();
    let restore_config = Rc::clone(&input_config);
    let restore_systems = systems.clone();
    let restore_inputs = Rc::clone(&active_inputs);
    let restore_pending = Rc::clone(&pending_rebind);
    let restore_blocked = Rc::clone(&rebind_blocked_key);
    window.on_restore_console1_controls(move || {
        *restore_pending.borrow_mut() = None;
        *restore_blocked.borrow_mut() = None;
        release_input_source(&restore_systems, &restore_inputs, 1, InputSource::Keyboard);
        release_input_source(&restore_systems, &restore_inputs, 1, InputSource::Virtual);
        let mut config = restore_config.borrow_mut();
        config.console1 = default_console1_profile();
        save_input_config(&config);
        if let Some(window) = restore_window.upgrade() {
            window.set_bindings_console_1(bindings_text(&config.console1).into());
            window.set_status(active_text("status_defaults_restored").into());
        }
    });

    let reset_console_window = window.as_weak();
    let reset_console_system = systems.clone();
    let reset_console_frames = Arc::clone(&frames);
    let reset_console_control = Arc::clone(&frame_control);
    window.on_reset_console(move |console| {
        let generation = reset_console_control.begin_reset(console as usize);
        let blank = FrameBuffers::white();
        {
            let mut output = reset_console_frames.lock().expect("frame mutex poisoned");
            output.reset_console(console as usize, generation);
        }
        let blank_image = render_pixels(&blank);
        if let Some(window) = reset_console_window.upgrade() {
            if console == 1 {
                window.set_frame_left(blank_image.clone());
            } else if console == 2 {
                window.set_frame_right(blank_image);
            }
        }
        let mut dual = reset_console_system.borrow_mut();
        match console {
            1 => {
                dual.left.ppu.take_frame_ready();
                dual.left.reset();
            }
            2 => {
                dual.right.ppu.take_frame_ready();
                dual.right.reset();
            }
            _ => return,
        }
        if let Some(window) = reset_console_window.upgrade() {
            window.set_status(
                active_format("status_reset", &[("console", &console.to_string())]).into(),
            );
        }
    });

    let eject_window = window.as_weak();
    let eject_system = systems.clone();
    let eject_left_save_path = Rc::clone(&save_path);
    let eject_right_save_path = Rc::clone(&right_save_path);
    let eject_selected_console = Rc::clone(&selected_save_console);
    let eject_selected_slot = Rc::clone(&selected_save_slot);
    let eject_inputs = Rc::clone(&active_inputs);
    window.on_eject_console(move |console| {
        let save_path = match console {
            1 => eject_left_save_path.borrow().clone(),
            2 => eject_right_save_path.borrow().clone(),
            _ => return,
        };
        let battery_written = {
            let dual = eject_system.borrow();
            match console {
                1 => dual.left.mmu.has_battery(),
                2 => dual.right.mmu.has_battery(),
                _ => false,
            }
        };
        let result = {
            let mut dual = eject_system.borrow_mut();
            eject_console(&mut dual, console as usize, save_path.as_deref())
        };
        let Some(window) = eject_window.upgrade() else {
            return;
        };
        match result {
            Ok(()) => {
                eject_inputs.borrow_mut().release_console(console as usize);
                match console {
                    1 => *eject_left_save_path.borrow_mut() = None,
                    2 => *eject_right_save_path.borrow_mut() = None,
                    _ => return,
                }
                let dual = eject_system.borrow();
                match console {
                    1 => {
                        window.set_cart_left(dual.left_cart.clone().into());
                        window.set_state_left(active_text("app_no_cartridge").into());
                        window.set_left_loaded(false);
                        window.set_frame_left(render_frame(&dual.left));
                    }
                    2 => {
                        window.set_cart_right(dual.right_cart.clone().into());
                        window.set_state_right(active_text("app_no_cartridge").into());
                        window.set_right_loaded(false);
                        window.set_frame_right(render_frame(&dual.right));
                    }
                    _ => return,
                }
                refresh_save_state_if_active(
                    &window,
                    &dual,
                    eject_left_save_path.borrow().as_deref(),
                    eject_right_save_path.borrow().as_deref(),
                    *eject_selected_console.borrow(),
                    *eject_selected_slot.borrow(),
                    console as usize,
                );
                let battery = if battery_written {
                    active_format(
                        "status_battery_written",
                        &[(
                            "path",
                            &save_path
                                .as_deref()
                                .map(|path| path.display().to_string())
                                .unwrap_or_default(),
                        )],
                    )
                } else {
                    active_text("status_no_battery")
                };
                window.set_status(
                    if dual.link_enabled {
                        active_format(
                            "status_console_ejected_link",
                            &[("console", &console.to_string()), ("battery", &battery)],
                        )
                    } else {
                        active_format(
                            "status_console_ejected",
                            &[("console", &console.to_string()), ("battery", &battery)],
                        )
                    }
                    .into(),
                );
            }
            Err(error) => window
                .set_status(active_format("status_operation_error", &[("error", &error)]).into()),
        }
    });

    let open_window = window.as_weak();
    let open_system = systems.clone();
    let open_save_path = Rc::clone(&save_path);
    let open_right_save_path = Rc::clone(&right_save_path);
    let open_selected_console = Rc::clone(&selected_save_console);
    let open_selected_slot = Rc::clone(&selected_save_slot);
    let open_preferences = Rc::clone(&preferences);
    window.on_open_rom(move || {
        let last_directory = open_preferences
            .borrow()
            .last_rom_directory
            .clone()
            .map(std::path::PathBuf::from);
        let mut dialog =
            rfd::FileDialog::new().add_filter("Game Boy ROM or ZIP", &["gb", "gbc", "zip"]);
        if let Some(directory) = last_directory.filter(|path| path.is_dir()) {
            dialog = dialog.set_directory(directory);
        }
        let Some(path) = dialog.pick_file() else {
            return;
        };
        if let Some(directory) = path.parent() {
            let mut preferences = open_preferences.borrow_mut();
            preferences.last_rom_directory = Some(directory.to_string_lossy().into_owned());
            save_preferences(&preferences);
        }
        if let Some(old_path) = open_save_path.borrow().as_ref() {
            let mut dual = open_system.borrow_mut();
            let _ = save_slots::save_battery(&mut dual.left.mmu, old_path);
            if let Some(right_path) = open_right_save_path.borrow().as_ref() {
                let _ = save_slots::save_battery(&mut dual.right.mmu, right_path);
            }
        }
        let Ok(source) = RomSource::from_path(path.clone()) else {
            if let Some(window) = open_window.upgrade() {
                window.set_status(active_text("status_unable_read_rom").into());
            }
            return;
        };
        let mut dual = open_system.borrow_mut();
        let Ok(new_save_path) = replace_console(&mut dual, 1, &source) else {
            if let Some(window) = open_window.upgrade() {
                window.set_status(active_text("status_unable_read_rom").into());
            }
            return;
        };
        *open_save_path.borrow_mut() = Some(new_save_path);
        if let Some(window) = open_window.upgrade() {
            window.set_status(
                active_format("status_loaded", &[("path", &source.display_name())]).into(),
            );
            window.set_cart_left(dual.left_cart.clone().into());
            window.set_state_left(visible_state_label(dual.left_loaded, dual.left.state).into());
            window.set_left_loaded(true);
            window.set_frame_left(render_frame(&dual.left));
            refresh_save_state_if_active(
                &window,
                &dual,
                open_save_path.borrow().as_deref(),
                open_right_save_path.borrow().as_deref(),
                *open_selected_console.borrow(),
                *open_selected_slot.borrow(),
                1,
            );
        }
    });

    let open_second_window = window.as_weak();
    let open_second_system = systems.clone();
    let open_second_save_path = Rc::clone(&right_save_path);
    let open_second_left_save_path = Rc::clone(&save_path);
    let open_second_selected_console = Rc::clone(&selected_save_console);
    let open_second_selected_slot = Rc::clone(&selected_save_slot);
    let open_second_preferences = Rc::clone(&preferences);
    window.on_open_rom_for_console(move |console| {
        if console != 2 {
            return;
        }
        let last_directory = open_second_preferences
            .borrow()
            .last_rom_directory
            .clone()
            .map(std::path::PathBuf::from);
        let mut dialog =
            rfd::FileDialog::new().add_filter("Game Boy ROM or ZIP", &["gb", "gbc", "zip"]);
        if let Some(directory) = last_directory.filter(|path| path.is_dir()) {
            dialog = dialog.set_directory(directory);
        }
        let Some(path) = dialog.pick_file() else {
            return;
        };
        if let Some(directory) = path.parent() {
            let mut preferences = open_second_preferences.borrow_mut();
            preferences.last_rom_directory = Some(directory.to_string_lossy().into_owned());
            save_preferences(&preferences);
        }
        if let Some(old_path) = open_second_save_path.borrow().as_ref() {
            let _ =
                save_slots::save_battery(&mut open_second_system.borrow_mut().right.mmu, old_path);
        }
        let Ok(source) = RomSource::from_path(path.clone()) else {
            if let Some(window) = open_second_window.upgrade() {
                window.set_status(active_text("status_unable_read_rom").into());
            }
            return;
        };
        let mut dual = open_second_system.borrow_mut();
        let Ok(new_save_path) = replace_console(&mut dual, 2, &source) else {
            if let Some(window) = open_second_window.upgrade() {
                window.set_status(active_text("status_unable_read_rom").into());
            }
            return;
        };
        *open_second_save_path.borrow_mut() = Some(new_save_path);
        if let Some(window) = open_second_window.upgrade() {
            window.set_cart_right(dual.right_cart.clone().into());
            window.set_state_right(visible_state_label(dual.right_loaded, dual.right.state).into());
            window.set_right_loaded(true);
            window.set_frame_right(render_frame(&dual.right));
            window.set_status(
                active_format("status_loaded", &[("path", &source.display_name())]).into(),
            );
            refresh_save_state_if_active(
                &window,
                &dual,
                open_second_left_save_path.borrow().as_deref(),
                open_second_save_path.borrow().as_deref(),
                *open_second_selected_console.borrow(),
                *open_second_selected_slot.borrow(),
                2,
            );
        }
    });

    let library_window = window.as_weak();
    let library_preferences_for_scan = Rc::clone(&preferences);
    let library_state_for_scan = Rc::clone(&library_state);
    let library_scan_pending = Rc::clone(&pending_library_scan);
    let library_scan_controller_for_scan = Arc::clone(&library_scan_controller);
    window.on_scan_library(move || {
        let Some(folder) = library_preferences_for_scan
            .borrow()
            .library_folder
            .clone()
            .map(std::path::PathBuf::from)
        else {
            if let Some(window) = library_window.upgrade() {
                window.set_status(active_text("status_select_folder").into());
            }
            return;
        };
        if !folder.is_dir() {
            if let Some(window) = library_window.upgrade() {
                window.set_status(
                    active_format(
                        "status_library_unavailable",
                        &[("path", &folder.display().to_string())],
                    )
                    .into(),
                );
            }
            return;
        }
        if let Some(window) = library_window.upgrade() {
            request_library_scan(
                &window,
                folder,
                false,
                &library_scan_pending,
                &library_scan_controller_for_scan,
                &library_state_for_scan,
            );
        }
    });

    let choose_library_window = window.as_weak();
    let choose_library_pending = Rc::clone(&pending_library_scan);
    let choose_library_controller = Arc::clone(&library_scan_controller);
    let choose_library_state = Rc::clone(&library_state);
    window.on_choose_library_folder(move || {
        let Some(folder) = rfd::FileDialog::new().pick_folder() else {
            return;
        };
        if let Some(window) = choose_library_window.upgrade() {
            request_library_scan(
                &window,
                folder,
                true,
                &choose_library_pending,
                &choose_library_controller,
                &choose_library_state,
            );
        }
    });

    let cancel_library_controller = Arc::clone(&library_scan_controller);
    window.on_cancel_library_scan(move || cancel_library_controller.cancel());

    let search_window = window.as_weak();
    let search_index = Rc::clone(&library_index);
    let search_state = Rc::clone(&library_state);
    window.on_search_library(move |query| {
        let query = query.to_string();
        let state = search_state.borrow();
        if let Some(window) = search_window.upgrade() {
            window.set_library_query(query.clone().into());
            set_library_models(&window, &search_index.borrow(), &state);
        }
    });

    let library_window = window.as_weak();
    let library_system = systems.clone();
    let library_save_path = Rc::clone(&save_path);
    let library_right_save_path = Rc::clone(&right_save_path);
    let library_selected_console = Rc::clone(&selected_save_console);
    let library_selected_slot = Rc::clone(&selected_save_slot);
    let library_paths_for_select = Rc::clone(&library_paths);
    let library_state_for_select = Rc::clone(&library_state);
    let selected_for_select = Rc::clone(&selected_library_path);
    window.on_select_library(move |label| {
        let Some(path) = library_paths_for_select
            .borrow()
            .iter()
            .find(|source| source.id() == label.as_str())
            .cloned()
        else {
            return;
        };
        if let Some(old_path) = library_save_path.borrow().as_ref() {
            let mut dual = library_system.borrow_mut();
            let _ = save_slots::save_battery(&mut dual.left.mmu, old_path);
            if let Some(right_path) = library_right_save_path.borrow().as_ref() {
                let _ = save_slots::save_battery(&mut dual.right.mmu, right_path);
            }
        }
        let mut dual = library_system.borrow_mut();
        let Ok(new_save_path) = replace_console(&mut dual, 1, &path) else {
            if let Some(window) = library_window.upgrade() {
                window.set_status(active_text("status_unable_read_rom").into());
            }
            return;
        };
        *selected_for_select.borrow_mut() = Some(path.clone());
        {
            let mut state = library_state_for_select.borrow_mut();
            let key = path.id();
            state.recent.retain(|recent| recent != &key);
            state.recent.insert(0, key);
            state.recent.truncate(20);
            save_library_state(&state);
        }
        if let Some(window) = library_window.upgrade() {
            window.set_cart_left(dual.left_cart.clone().into());
            window.set_frame_left(render_frame(&dual.left));
        }
        *library_save_path.borrow_mut() = Some(new_save_path);
        if let Some(window) = library_window.upgrade() {
            window.set_library_panel_visible(false);
            window.set_status(
                active_format("status_loaded", &[("path", &path.display_name())]).into(),
            );
            window.set_frame_left(render_frame(&dual.left));
            window.set_frame_right(render_frame(&dual.right));
            refresh_save_state_if_active(
                &window,
                &dual,
                library_save_path.borrow().as_deref(),
                library_right_save_path.borrow().as_deref(),
                *library_selected_console.borrow(),
                *library_selected_slot.borrow(),
                1,
            );
            let state = library_state_for_select.borrow();
            window.set_recent_games(slint::ModelRc::new(slint::VecModel::from(recent_labels(
                &state,
            ))));
            if let Some(cover) = rom_identity_source(&path)
                .and_then(|key| state.covers.get(&key))
                .or_else(|| state.covers.get(&path.id()))
            {
                let cover_path = std::path::Path::new(cover);
                if cover_path.is_file() {
                    if let Ok(image) = slint::Image::load_from_path(cover_path) {
                        window.set_selected_cover(image);
                    }
                }
            }
        }
    });

    let second_library_window = window.as_weak();
    let second_library_system = systems.clone();
    let second_library_save_path = Rc::clone(&right_save_path);
    let second_library_left_save_path = Rc::clone(&save_path);
    let second_library_selected_console = Rc::clone(&selected_save_console);
    let second_library_selected_slot = Rc::clone(&selected_save_slot);
    let second_library_paths = Rc::clone(&library_paths);
    window.on_select_library_for_console(move |label, console| {
        if console != 2 {
            return;
        }
        let Some(path) = second_library_paths
            .borrow()
            .iter()
            .find(|source| source.id() == label.as_str())
            .cloned()
        else {
            return;
        };
        if let Some(old_path) = second_library_save_path.borrow().as_ref() {
            let _ = second_library_system
                .borrow_mut()
                .right
                .mmu
                .save_ram(old_path);
        }
        let mut dual = second_library_system.borrow_mut();
        let Ok(new_save_path) = replace_console(&mut dual, 2, &path) else {
            if let Some(window) = second_library_window.upgrade() {
                window.set_status(active_text("status_unable_read_rom").into());
            }
            return;
        };
        *second_library_save_path.borrow_mut() = Some(new_save_path);
        if let Some(window) = second_library_window.upgrade() {
            window.set_library_panel_visible(false);
            window.set_cart_right(dual.right_cart.clone().into());
            window.set_frame_right(render_frame(&dual.right));
            window.set_status(
                active_format("status_loaded", &[("path", &path.display_name())]).into(),
            );
            refresh_save_state_if_active(
                &window,
                &dual,
                second_library_left_save_path.borrow().as_deref(),
                second_library_save_path.borrow().as_deref(),
                *second_library_selected_console.borrow(),
                *second_library_selected_slot.borrow(),
                2,
            );
        }
    });

    let edit_window = window.as_weak();
    let edit_paths = Rc::clone(&library_paths);
    let edit_state = Rc::clone(&library_state);
    let edit_path = Rc::clone(&editing_library_path);
    let edit_cover = Rc::clone(&editing_cover_file);
    let edit_reset = Rc::clone(&editing_cover_reset);
    window.on_edit_library(move |id| {
        let Some(path) = edit_paths
            .borrow()
            .iter()
            .find(|source| source.id() == id.as_str())
            .cloned()
        else {
            return;
        };
        *edit_path.borrow_mut() = Some(path.clone());
        *edit_cover.borrow_mut() = None;
        *edit_reset.borrow_mut() = false;
        if let Some(window) = edit_window.upgrade() {
            let bytes = path.load().unwrap_or_default();
            let crc = if bytes.len() >= 0x150 {
                format!("{:08X}", rom_crc32(&bytes))
            } else {
                "unknown".into()
            };
            let state = edit_state.borrow();
            window.set_library_editor_id(id);
            let internal = library_internal_title_source(&path);
            window.set_library_editor_internal(format_library_internal(&internal).into());
            window.set_library_editor_commercial(commercial_name_source(&path, &state).into());
            window.set_library_editor_system(library_system_name_source(&path).into());
            window.set_library_editor_crc(crc.into());
            window.set_library_editor_catalog(
                if state.manual_covers.contains(&library_key(&path)) {
                    commercial_name_source(&path, &state).into()
                } else {
                    "".into()
                },
            );
            window.set_library_search_results(slint::ModelRc::new(slint::VecModel::from(Vec::<
                slint::SharedString,
            >::new(
            ))));
            window.set_library_editor_cover(slint::Image::default());
            let cover = state
                .covers
                .get(&library_key(&path))
                .and_then(|cover| slint::Image::load_from_path(std::path::Path::new(cover)).ok())
                .or_else(|| cached_cover(&path));
            if let Some(image) = cover {
                window.set_library_editor_cover(image);
            }
            window.set_library_editor_open(true);
        }
    });

    let search_name_window = window.as_weak();
    let search_name_path = Rc::clone(&editing_library_path);
    window.on_search_library_name(move |query| {
        let Some(path) = search_name_path.borrow().clone() else {
            return;
        };
        let Ok(bytes) = path.load() else {
            return;
        };
        let color = System::cgb_mode_for_rom(&bytes);
        let weak = search_name_window.clone();
        std::thread::spawn(move || {
            let catalog = catalog_names(color);
            let normalized = normalized_catalog_name(&query);
            let results = catalog
                .into_iter()
                .filter(|name| normalized_catalog_name(name).contains(&normalized))
                .take(20)
                .map(Into::into)
                .collect::<Vec<slint::SharedString>>();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(window) = weak.upgrade() {
                    window.set_library_search_results(slint::ModelRc::new(slint::VecModel::from(
                        results,
                    )));
                    window.set_status(
                        if normalized.is_empty() {
                            "Escribí un nombre para buscar"
                        } else {
                            "Resultados del catálogo"
                        }
                        .into(),
                    );
                }
            });
        });
    });

    let result_window = window.as_weak();
    let result_path = Rc::clone(&editing_library_path);
    window.on_choose_library_result(move |result| {
        let Some(path) = result_path.borrow().clone() else {
            return;
        };
        if let Some(window) = result_window.upgrade() {
            window.set_library_editor_commercial(result.clone());
            window.set_library_editor_catalog(result.clone());
            window.set_status(active_text("status_downloading_cover").into());
            let weak = window.as_weak();
            std::thread::spawn(move || {
                if let Some(cache) = download_named_cover(&path, &result) {
                    let _ = slint::invoke_from_event_loop(move || {
                        if let Some(window) = weak.upgrade() {
                            if let Ok(image) = slint::Image::load_from_path(&cache) {
                                window.set_library_editor_cover(image);
                            }
                            window.set_status(active_text("library_cover_ready").into());
                        }
                    });
                }
            });
        }
    });

    let cancel_window = window.as_weak();
    let cancel_path = Rc::clone(&editing_library_path);
    let cancel_cover = Rc::clone(&editing_cover_file);
    let cancel_reset = Rc::clone(&editing_cover_reset);
    window.on_cancel_library_edit(move || {
        *cancel_path.borrow_mut() = None;
        *cancel_cover.borrow_mut() = None;
        *cancel_reset.borrow_mut() = false;
        if let Some(window) = cancel_window.upgrade() {
            window.set_library_editor_open(false);
        }
    });

    let reset_window = window.as_weak();
    let reset_path = Rc::clone(&editing_library_path);
    let reset_cover = Rc::clone(&editing_cover_file);
    let reset_reset = Rc::clone(&editing_cover_reset);
    window.on_reset_library_cover(move || {
        if reset_path.borrow().is_none() {
            return;
        }
        *reset_cover.borrow_mut() = None;
        *reset_reset.borrow_mut() = true;
        if let Some(window) = reset_window.upgrade() {
            window.set_library_editor_cover(slint::Image::default());
            window.set_library_editor_catalog("".into());
        }
    });

    let save_edit_window = window.as_weak();
    let save_edit_path = Rc::clone(&editing_library_path);
    let save_edit_state = Rc::clone(&library_state);
    let save_edit_paths = Rc::clone(&library_paths);
    let save_edit_index = Rc::clone(&library_index);
    let save_edit_cover = Rc::clone(&editing_cover_file);
    let save_edit_reset = Rc::clone(&editing_cover_reset);
    window.on_save_library_edit(move || {
        let Some(path) = save_edit_path.borrow().clone() else {
            return;
        };
        let Some(window) = save_edit_window.upgrade() else {
            return;
        };
        let key = library_key(&path);
        let commercial = window.get_library_editor_commercial().to_string();
        let catalog = window.get_library_editor_catalog().to_string();
        let reset = *save_edit_reset.borrow();
        let mut state = save_edit_state.borrow_mut();
        if commercial.trim().is_empty() {
            state.commercial_names.remove(&key);
        } else {
            state.commercial_names.insert(key.clone(), commercial);
        }
        if reset {
            let _ = std::fs::remove_file(cover_cache_path(&path));
            state.covers.remove(&key);
            state.manual_covers.remove(&key);
        } else {
            if let Some(cover) = save_edit_cover.borrow().clone() {
                state
                    .covers
                    .insert(key.clone(), cover.to_string_lossy().into_owned());
            } else if !catalog.is_empty() {
                state.covers.insert(
                    key.clone(),
                    cover_cache_path(&path).to_string_lossy().into_owned(),
                );
            }
            if save_edit_cover.borrow().is_some() || !catalog.is_empty() {
                state.manual_covers.insert(key);
            }
        }
        save_library_state(&state);
        *save_edit_index.borrow_mut() =
            build_library_search_index(&save_edit_paths.borrow(), &state);
        set_library_models(&window, &save_edit_index.borrow(), &state);
        window.set_library_editor_open(false);
        drop(state);
        *save_edit_path.borrow_mut() = None;
        *save_edit_cover.borrow_mut() = None;
        *save_edit_reset.borrow_mut() = false;
        window.set_status(active_text("status_cover_saved").into());
        if reset {
            start_cover_downloads(vec![path], window.as_weak());
        }
    });

    let favorite_window = window.as_weak();
    let favorite_index = Rc::clone(&library_index);
    let favorite_state = Rc::clone(&library_state);
    let favorite_selected = Rc::clone(&selected_library_path);
    window.on_toggle_library_favorite(move || {
        let Some(path) = favorite_selected.borrow().clone() else {
            return;
        };
        let key = path.id();
        let mut state = favorite_state.borrow_mut();
        if !state.favorites.insert(key.clone()) {
            state.favorites.remove(&key);
        }
        save_library_state(&state);
        if let Some(window) = favorite_window.upgrade() {
            set_library_models(&window, &favorite_index.borrow(), &state);
            window.set_status(
                if state.favorites.contains(&key) {
                    "Added to favorites"
                } else {
                    "Removed from favorites"
                }
                .into(),
            );
        }
    });

    let cover_window = window.as_weak();
    let cover_selected = Rc::clone(&editing_library_path);
    let cover_pending = Rc::clone(&editing_cover_file);
    window.on_choose_library_cover(move || {
        if cover_selected.borrow().is_none() {
            return;
        }
        let Some(cover_path) = rfd::FileDialog::new()
            .add_filter("Cover image", &["png", "jpg", "jpeg", "webp"])
            .pick_file()
        else {
            return;
        };
        if let Some(window) = cover_window.upgrade() {
            if let Ok(image) = slint::Image::load_from_path(&cover_path) {
                *cover_pending.borrow_mut() = Some(cover_path.clone());
                window.set_selected_cover(image.clone());
                window.set_library_editor_cover(image);
            } else {
                window.set_status(active_text("library_cover_invalid").into());
                return;
            }
            window.set_status(active_text("library_cover_ready").into());
        }
    });

    let state_window = window.as_weak();
    let state_system = systems.clone();
    let state_save_path = Rc::clone(&save_path);
    let state_right_save_path = Rc::clone(&right_save_path);
    let state_selected_console = Rc::clone(&selected_save_console);
    let state_selected_slot = Rc::clone(&selected_save_slot);
    window.on_save_state(move || {
        let Some(save_path) = state_save_path.borrow().clone() else {
            return;
        };
        let dual = state_system.borrow();
        let identity = save_slots::identity_from_battery(&save_path);
        let right_path = state_right_save_path.borrow().clone();
        let result = identity
            .as_deref()
            .ok_or_else(|| std::io::Error::other("invalid ROM save identity"))
            .and_then(|identity| {
                let storage = state_storage_path(&save_path, 1, right_path.as_deref());
                save_slots::save_slot_at(&dual.left, &storage, identity, &dual.left_cart, 0)
                    .map(|_| ())
            });
        let right_result = state_right_save_path
            .borrow()
            .as_ref()
            .and_then(|path| {
                save_slots::identity_from_battery(path).map(|identity| (path, identity))
            })
            .map(|(path, identity)| {
                let storage = state_storage_path(path, 2, Some(&save_path));
                save_slots::save_slot_at(&dual.right, &storage, &identity, &dual.right_cart, 0)
                    .is_ok()
            })
            .unwrap_or(true);
        if let Some(window) = state_window.upgrade() {
            window.set_status(if result.is_ok() && right_result {
                active_text("state_saved").into()
            } else {
                active_text("status_unable_state").into()
            });
            let active_console = *state_selected_console.borrow();
            refresh_save_state_if_active(
                &window,
                &dual,
                state_save_path.borrow().as_deref(),
                state_right_save_path.borrow().as_deref(),
                active_console,
                *state_selected_slot.borrow(),
                active_console,
            );
        }
    });

    let state_window = window.as_weak();
    let state_system = systems.clone();
    let state_save_path = Rc::clone(&save_path);
    let state_right_save_path = Rc::clone(&right_save_path);
    let state_selected_console = Rc::clone(&selected_save_console);
    let state_selected_slot = Rc::clone(&selected_save_slot);
    window.on_load_state(move || {
        let Some(save_path) = state_save_path.borrow().clone() else {
            return;
        };
        let mut dual = state_system.borrow_mut();
        let right_path = state_right_save_path.borrow().clone();
        let result = save_slots::identity_from_battery(&save_path)
            .ok_or_else(|| std::io::Error::other("invalid ROM save identity"))
            .and_then(|identity| {
                let storage = state_storage_path(&save_path, 1, right_path.as_deref());
                save_slots::load_slot_at(&mut dual.left, &storage, &identity, 0).map(|_| ())
            });
        let right_result = state_right_save_path
            .borrow()
            .as_ref()
            .and_then(|path| {
                save_slots::identity_from_battery(path).map(|identity| (path, identity))
            })
            .map(|(path, identity)| {
                let storage = state_storage_path(path, 2, Some(&save_path));
                save_slots::load_slot_at(&mut dual.right, &storage, &identity, 0).is_ok()
            })
            .unwrap_or(true);
        if let Some(window) = state_window.upgrade() {
            window.set_status(if result.is_ok() && right_result {
                active_text("state_loaded").into()
            } else {
                active_text("status_unable_state").into()
            });
            let active_console = *state_selected_console.borrow();
            refresh_save_state_if_active(
                &window,
                &dual,
                state_save_path.borrow().as_deref(),
                state_right_save_path.borrow().as_deref(),
                active_console,
                *state_selected_slot.borrow(),
                active_console,
            );
        }
    });

    let console_state_window = window.as_weak();
    let console_state_system = systems.clone();
    let console_state_left_path = Rc::clone(&save_path);
    let console_state_right_path = Rc::clone(&right_save_path);
    window.on_save_state_for_console(move |console| {
        let (state_file, result) = match console {
            1 => {
                let Some(path) = console_state_left_path.borrow().clone() else {
                    return;
                };
                let result = console_state_system.borrow();
                let other = console_state_right_path.borrow().clone();
                let storage = state_storage_path(&path, 1, other.as_deref());
                let state_file = save_slots::slot_path_from_battery(&storage, 0)
                    .expect("constant slot 0 is valid");
                let identity = save_slots::identity_from_battery(&path);
                let result = identity
                    .as_deref()
                    .ok_or_else(|| std::io::Error::other("invalid ROM save identity"))
                    .and_then(|identity| {
                        save_slots::save_slot_at(
                            &result.left,
                            &storage,
                            identity,
                            &result.left_cart,
                            0,
                        )
                        .map(|_| ())
                    });
                (state_file, result)
            }
            2 => {
                let Some(path) = console_state_right_path.borrow().clone() else {
                    if let Some(window) = console_state_window.upgrade() {
                        window.set_status(active_text("status_no_cartridge").into());
                    }
                    return;
                };
                let result = console_state_system.borrow();
                let other = console_state_left_path.borrow().clone();
                let storage = state_storage_path(&path, 2, other.as_deref());
                let state_file = save_slots::slot_path_from_battery(&storage, 0)
                    .expect("constant slot 0 is valid");
                let identity = save_slots::identity_from_battery(&path);
                let result = identity
                    .as_deref()
                    .ok_or_else(|| std::io::Error::other("invalid ROM save identity"))
                    .and_then(|identity| {
                        save_slots::save_slot_at(
                            &result.right,
                            &storage,
                            identity,
                            &result.right_cart,
                            0,
                        )
                        .map(|_| ())
                    });
                (state_file, result)
            }
            _ => return,
        };
        if let Some(window) = console_state_window.upgrade() {
            window.set_status(
                if result.is_ok() {
                    active_format(
                        "state_saved",
                        &[("path", &state_file.display().to_string())],
                    )
                } else {
                    active_text("status_unable_state")
                }
                .into(),
            );
        }
    });

    let console_load_window = window.as_weak();
    let console_load_system = systems.clone();
    let console_load_left_path = Rc::clone(&save_path);
    let console_load_right_path = Rc::clone(&right_save_path);
    window.on_load_state_for_console(move |console| {
        let (state_file, result) = match console {
            1 => {
                let Some(path) = console_load_left_path.borrow().clone() else {
                    return;
                };
                let mut dual = console_load_system.borrow_mut();
                let other = console_load_right_path.borrow().clone();
                let storage = state_storage_path(&path, 1, other.as_deref());
                let state_file =
                    save_slots::resolved_slot_path(&storage, 0).expect("constant slot 0 is valid");
                let result = save_slots::identity_from_battery(&path)
                    .ok_or_else(|| std::io::Error::other("invalid ROM save identity"))
                    .and_then(|identity| {
                        save_slots::load_slot_at(&mut dual.left, &storage, &identity, 0).map(|_| ())
                    });
                (state_file, result)
            }
            2 => {
                let Some(path) = console_load_right_path.borrow().clone() else {
                    if let Some(window) = console_load_window.upgrade() {
                        window.set_status(active_text("status_no_cartridge").into());
                    }
                    return;
                };
                let mut dual = console_load_system.borrow_mut();
                let other = console_load_left_path.borrow().clone();
                let storage = state_storage_path(&path, 2, other.as_deref());
                let state_file =
                    save_slots::resolved_slot_path(&storage, 0).expect("constant slot 0 is valid");
                let result = save_slots::identity_from_battery(&path)
                    .ok_or_else(|| std::io::Error::other("invalid ROM save identity"))
                    .and_then(|identity| {
                        save_slots::load_slot_at(&mut dual.right, &storage, &identity, 0)
                            .map(|_| ())
                    });
                (state_file, result)
            }
            _ => return,
        };
        if let Some(window) = console_load_window.upgrade() {
            window.set_status(
                if result.is_ok() {
                    active_format(
                        "state_loaded",
                        &[("path", &state_file.display().to_string())],
                    )
                } else {
                    active_text("status_unable_state")
                }
                .into(),
            );
        }
    });

    let slot_console = Rc::clone(&selected_save_console);
    let slot_selected = Rc::clone(&selected_save_slot);
    let slot_window = window.as_weak();
    let slot_system = systems.clone();
    let slot_left_path = Rc::clone(&save_path);
    let slot_right_path = Rc::clone(&right_save_path);
    window.on_select_save_state_console(move |console| {
        if !(1..=2).contains(&(console as usize)) {
            return;
        }
        *slot_console.borrow_mut() = console as usize;
        *slot_selected.borrow_mut() = 0;
        if let Some(window) = slot_window.upgrade() {
            let dual = slot_system.borrow();
            refresh_save_state_if_active(
                &window,
                &dual,
                slot_left_path.borrow().as_deref(),
                slot_right_path.borrow().as_deref(),
                console as usize,
                0,
                console as usize,
            );
        }
    });

    let select_slot_window = window.as_weak();
    let select_slot_system = systems.clone();
    let select_slot_console = Rc::clone(&selected_save_console);
    let select_slot_selected = Rc::clone(&selected_save_slot);
    let select_slot_left_path = Rc::clone(&save_path);
    let select_slot_right_path = Rc::clone(&right_save_path);
    window.on_select_save_state_slot(move |slot| {
        let slot = slot as usize;
        if slot >= save_slots::SLOT_COUNT {
            return;
        }
        *select_slot_selected.borrow_mut() = slot;
        if let Some(window) = select_slot_window.upgrade() {
            let console = *select_slot_console.borrow();
            let dual = select_slot_system.borrow();
            refresh_save_state_if_active(
                &window,
                &dual,
                select_slot_left_path.borrow().as_deref(),
                select_slot_right_path.borrow().as_deref(),
                console,
                slot,
                console,
            );
        }
    });

    let selected_save_window = window.as_weak();
    let selected_save_system = systems.clone();
    let selected_save_console_ref = Rc::clone(&selected_save_console);
    let selected_save_slot_ref = Rc::clone(&selected_save_slot);
    let selected_save_left_path = Rc::clone(&save_path);
    let selected_save_right_path = Rc::clone(&right_save_path);
    window.on_save_selected_state(move || {
        let console = *selected_save_console_ref.borrow();
        let slot = *selected_save_slot_ref.borrow();
        let dual = selected_save_system.borrow_mut();
        let path = if console == 1 {
            selected_save_left_path.borrow().clone()
        } else {
            selected_save_right_path.borrow().clone()
        };
        let Some(path) = path else {
            if let Some(window) = selected_save_window.upgrade() {
                window.set_status(active_text("status_no_cartridge").into());
            }
            return;
        };
        let Some(identity) = save_slots::identity_from_battery(&path) else {
            return;
        };
        let other = if console == 1 {
            selected_save_right_path.borrow().clone()
        } else {
            selected_save_left_path.borrow().clone()
        };
        let storage = state_storage_path(&path, console, other.as_deref());
        let Ok(state_file) = save_slots::slot_path_from_battery(&storage, slot) else {
            if let Some(window) = selected_save_window.upgrade() {
                window.set_status(active_text("status_invalid_slot").into());
            }
            return;
        };
        if slot != 0 && save_slots::inspect_slot_at(&storage, &identity, slot).is_some() {
            let confirmed = matches!(
                rfd::MessageDialog::new()
                    .set_title(active_text("dialog_overwrite_state_title"))
                    .set_description(active_text("dialog_overwrite_state_description"))
                    .set_buttons(rfd::MessageButtons::YesNo)
                    .show(),
                rfd::MessageDialogResult::Yes
            );
            if !confirmed {
                return;
            }
        }
        let result = if console == 1 {
            save_slots::save_slot_at(&dual.left, &storage, &identity, &dual.left_cart, slot)
        } else {
            save_slots::save_slot_at(&dual.right, &storage, &identity, &dual.right_cart, slot)
        };
        if let Some(window) = selected_save_window.upgrade() {
            window.set_status(
                if result.is_ok() {
                    active_format(
                        "status_slot_saved",
                        &[
                            ("slot", &slot.to_string()),
                            ("path", &state_file.display().to_string()),
                        ],
                    )
                } else {
                    active_format("status_slot_save_failed", &[("slot", &slot.to_string())])
                }
                .into(),
            );
            let cart = if console == 1 {
                &dual.left_cart
            } else {
                &dual.right_cart
            };
            refresh_save_state_ui(&window, console, Some(&storage), cart, slot);
        }
    });

    let selected_load_window = window.as_weak();
    let selected_load_system = systems.clone();
    let selected_load_console = Rc::clone(&selected_save_console);
    let selected_load_slot = Rc::clone(&selected_save_slot);
    let selected_load_left_path = Rc::clone(&save_path);
    let selected_load_right_path = Rc::clone(&right_save_path);
    window.on_load_selected_state(move || {
        let console = *selected_load_console.borrow();
        let slot = *selected_load_slot.borrow();
        let mut dual = selected_load_system.borrow_mut();
        let path = if console == 1 {
            selected_load_left_path.borrow().clone()
        } else {
            selected_load_right_path.borrow().clone()
        };
        let Some(path) = path else {
            return;
        };
        let link_was_enabled = dual.link_enabled;
        if link_was_enabled {
            dual.link_enabled = false;
        }
        let Some(identity) = save_slots::identity_from_battery(&path) else {
            return;
        };
        let other = if console == 1 {
            selected_load_right_path.borrow().clone()
        } else {
            selected_load_left_path.borrow().clone()
        };
        let storage = state_storage_path(&path, console, other.as_deref());
        let Ok(state_file) = save_slots::resolved_slot_path(&storage, slot) else {
            if let Some(window) = selected_load_window.upgrade() {
                window.set_status(active_text("status_invalid_slot").into());
            }
            return;
        };
        let result = if console == 1 {
            save_slots::load_slot_at(&mut dual.left, &storage, &identity, slot)
        } else {
            save_slots::load_slot_at(&mut dual.right, &storage, &identity, slot)
        };
        if let Some(window) = selected_load_window.upgrade() {
            if link_was_enabled {
                window.set_link_enabled(false);
            }
            window.set_status(
                if result.is_ok() {
                    if link_was_enabled {
                        active_format(
                            "status_slot_loaded_link_disabled",
                            &[
                                ("slot", &slot.to_string()),
                                ("path", &state_file.display().to_string()),
                            ],
                        )
                    } else {
                        active_format(
                            "status_slot_loaded",
                            &[
                                ("slot", &slot.to_string()),
                                ("path", &state_file.display().to_string()),
                            ],
                        )
                    }
                } else {
                    active_format("status_slot_load_failed", &[("slot", &slot.to_string())])
                }
                .into(),
            );
        }
    });

    let selected_delete_window = window.as_weak();
    let selected_delete_console = Rc::clone(&selected_save_console);
    let selected_delete_slot = Rc::clone(&selected_save_slot);
    let selected_delete_left_path = Rc::clone(&save_path);
    let selected_delete_right_path = Rc::clone(&right_save_path);
    window.on_delete_selected_state(move || {
        let console = *selected_delete_console.borrow();
        let slot = *selected_delete_slot.borrow();
        let path = if console == 1 {
            selected_delete_left_path.borrow().clone()
        } else {
            selected_delete_right_path.borrow().clone()
        };
        let Some(path) = path else {
            return;
        };
        let other = if console == 1 {
            selected_delete_right_path.borrow().clone()
        } else {
            selected_delete_left_path.borrow().clone()
        };
        let storage = state_storage_path(&path, console, other.as_deref());
        let confirmed = matches!(
            rfd::MessageDialog::new()
                .set_title(active_text("dialog_delete_state_title"))
                .set_description(active_text("dialog_delete_state_description"))
                .set_buttons(rfd::MessageButtons::YesNo)
                .show(),
            rfd::MessageDialogResult::Yes
        );
        if !confirmed {
            return;
        }
        let Ok(state_file) = save_slots::resolved_slot_path(&storage, slot) else {
            if let Some(window) = selected_delete_window.upgrade() {
                window.set_status(active_text("status_invalid_slot").into());
            }
            return;
        };
        let result = save_slots::delete_slot_at(&storage, slot);
        if let Some(window) = selected_delete_window.upgrade() {
            window.set_status(
                if result.is_ok() {
                    active_format(
                        "status_slot_deleted",
                        &[
                            ("slot", &slot.to_string()),
                            ("path", &state_file.display().to_string()),
                        ],
                    )
                } else {
                    active_format("status_slot_delete_failed", &[("slot", &slot.to_string())])
                }
                .into(),
            );
        }
    });

    let external_window = window.as_weak();
    let external_system = systems.clone();
    let external_console = Rc::clone(&selected_save_console);
    let external_slot = Rc::clone(&selected_save_slot);
    let external_left_path = Rc::clone(&save_path);
    let external_right_path = Rc::clone(&right_save_path);
    window.on_load_state_from_file(move || {
        let console = *external_console.borrow();
        let slot = *external_slot.borrow();
        let selected_battery = if console == 1 {
            external_left_path.borrow().clone()
        } else {
            external_right_path.borrow().clone()
        };
        let Some(canonical) = selected_battery else {
            if let Some(window) = external_window.upgrade() {
            window.set_status(active_text("status_no_cartridge").into());
            }
            return;
        };
        let Some(source) = rfd::FileDialog::new()
            .add_filter("Guardados FoxMulator", &["sav", "state"])
            .add_filter("Todos los archivos", &["*"])
            .pick_file()
        else {
            return;
        };
        let kind = match save_slots::identify_external_save(&source) {
            Ok(kind) => kind,
            Err(error) => {
                if let Some(window) = external_window.upgrade() {
                        window.set_status(
                            active_format("status_file_unrecognized", &[("error", &error.to_string())])
                                .into(),
                        );
                }
                return;
            }
        };
        let mut dual = external_system.borrow_mut();
        let cart = if console == 1 { dual.left_cart.clone() } else { dual.right_cart.clone() };
        let Some(identity) = save_slots::identity_from_battery(&canonical) else {
            return;
        };
        match kind {
            save_slots::ExternalSaveKind::Battery => {
                let candidate = match if console == 1 {
                    save_slots::validate_external_battery(&dual.left.mmu, &source)
                } else {
                    save_slots::validate_external_battery(&dual.right.mmu, &source)
                } {
                    Ok(candidate) => candidate,
                    Err(error) => {
                        if let Some(window) = external_window.upgrade() {
                            window.set_status(
                                active_format("status_sram_incompatible", &[("error", &error.to_string())])
                                    .into(),
                            );
                        }
                        return;
                    }
                };
                let confirmed = matches!(
                    rfd::MessageDialog::new()
                        .set_title("Importar partida de batería")
                        .set_description("Se respaldará la partida actual y se reemplazará la SRAM residente. ¿Continuar?")
                        .set_buttons(rfd::MessageButtons::YesNo)
                        .show(),
                    rfd::MessageDialogResult::Yes
                );
                if !confirmed { return; }
                if let Err(error) = save_slots::install_external_battery(&source, &canonical) {
                    if let Some(window) = external_window.upgrade() {
                            window.set_status(
                                active_format("status_import_failed", &[("error", &error.to_string())])
                                    .into(),
                            );
                    }
                    return;
                }
                let same_battery = if console == 1 {
                    external_right_path.borrow().as_ref() == Some(&canonical)
                } else {
                    external_left_path.borrow().as_ref() == Some(&canonical)
                };
                if console == 1 {
                    dual.left.mmu = candidate.clone();
                    if same_battery { dual.right.mmu = candidate; }
                } else {
                    dual.right.mmu = candidate.clone();
                    if same_battery { dual.left.mmu = candidate; }
                }
                if let Some(window) = external_window.upgrade() {
                    window.set_status(
                        active_format(
                            "status_battery_imported",
                            &[
                                ("canonical", &canonical.display().to_string()),
                                ("source", &source.display().to_string()),
                            ],
                        )
                        .into(),
                    );
                    refresh_save_state_ui(&window, console, Some(&canonical), &cart, slot);
                }
            }
            save_slots::ExternalSaveKind::State => {
                let mut restored = if console == 1 { dual.left.clone() } else { dual.right.clone() };
                if let Err(error) = restored.load_state(&source) {
                    if let Some(window) = external_window.upgrade() {
                        window.set_status(
                            active_format(
                                "status_external_state_incompatible",
                                &[("error", &error.to_string())],
                            )
                            .into(),
                        );
                    }
                    return;
                }
                let current_checksum = if console == 1 { dual.left.rom_checksum() } else { dual.right.rom_checksum() };
                if restored.rom_checksum() != current_checksum {
                    if let Some(window) = external_window.upgrade() {
                        window.set_status(active_text("status_state_wrong_rom").into());
                    }
                    return;
                }
                let confirmed = matches!(
                    rfd::MessageDialog::new()
                        .set_title(active_text("dialog_load_state_title"))
                        .set_description(active_text("dialog_load_state_description"))
                        .set_buttons(rfd::MessageButtons::YesNo)
                        .show(),
                    rfd::MessageDialogResult::Yes
                );
                if !confirmed { return; }
                let link_was_enabled = dual.link_enabled;
                if link_was_enabled { dual.link_enabled = false; }
                if console == 1 { dual.left = restored; } else { dual.right = restored; }
                let mut imported = false;
                let other_path = if console == 1 { external_right_path.borrow().clone() } else { external_left_path.borrow().clone() };
                let storage = state_storage_path(&canonical, console, other_path.as_deref());
                let should_import = matches!(
                    rfd::MessageDialog::new()
                        .set_title(active_text("dialog_save_copy_title"))
                        .set_description(active_text("dialog_save_copy_description"))
                        .set_buttons(rfd::MessageButtons::YesNo)
                        .show(),
                    rfd::MessageDialogResult::Yes
                );
                if should_import {
                    let occupied = save_slots::inspect_slot_at(&storage, &identity, slot).is_some();
                    let allowed = !occupied || matches!(
                        rfd::MessageDialog::new()
                        .set_title(active_text("dialog_overwrite_slot_title"))
                        .set_description(active_text("dialog_overwrite_slot_description"))
                            .set_buttons(rfd::MessageButtons::YesNo)
                            .show(),
                        rfd::MessageDialogResult::Yes
                    );
                    if allowed {
                        let _ = if console == 1 {
                            save_slots::save_slot_at(&dual.left, &storage, &identity, &cart, slot)
                        } else {
                            save_slots::save_slot_at(&dual.right, &storage, &identity, &cart, slot)
                        };
                        imported = true;
                    }
                }
                if let Some(window) = external_window.upgrade() {
                    if link_was_enabled { window.set_link_enabled(false); }
                    let status = if imported {
                        active_format("status_external_state_imported", &[("slot", &slot.to_string())])
                    } else {
                        active_format(
                            "status_external_state_loaded",
                            &[("source", &source.display().to_string())],
                        )
                    };
                    window.set_status(status.into());
                    refresh_save_state_ui(&window, console, Some(&storage), &cart, slot);
                }
            }
        }
    });

    let clock_window = window.as_weak();
    let clock_system = systems.clone();
    window.on_serial_clock(move || {
        let mut dual = clock_system.borrow_mut();
        let left_byte = dual.left.mmu.read_byte(SB_ADDR);
        let right_byte = dual.right.mmu.read_byte(SB_ADDR);
        let left_done = dual.left.clock_external_serial(right_byte);
        let right_done = dual.right.clock_external_serial(left_byte);
        if let Some(window) = clock_window.upgrade() {
            window.set_status(if left_done || right_done {
                "External serial clock applied".into()
            } else {
                "No external serial transfer waiting".into()
            });
        }
    });

    let link_window = window.as_weak();
    let link_system = systems.clone();
    window.on_toggle_link_cable(move || {
        let mut dual = link_system.borrow_mut();
        dual.link_enabled = !dual.link_enabled;
        if let Some(window) = link_window.upgrade() {
            window.set_link_enabled(dual.link_enabled);
            window.set_status(
                if dual.link_enabled {
                    "Game Link enabled (serial only)"
                } else {
                    "Game Link disabled"
                }
                .into(),
            );
        }
    });

    let shell_window = window.as_weak();
    let shell_config = Rc::clone(&input_config);
    window.on_next_shell(move || {
        shell_index = shell_index.wrapping_add(1);
        let (shell_color, accent_color, name) = shell_theme(shell_index);
        {
            let mut config = shell_config.borrow_mut();
            config.shell_color = format!(
                "#{:02x}{:02x}{:02x}",
                shell_color.red(),
                shell_color.green(),
                shell_color.blue()
            );
            config.accent_color = format!(
                "#{:02x}{:02x}{:02x}",
                accent_color.red(),
                accent_color.green(),
                accent_color.blue()
            );
            save_input_config(&config);
        }
        if let Some(window) = shell_window.upgrade() {
            window.set_shell_color(shell_color);
            window.set_accent_color(accent_color);
            window.set_shell_color_hex(shell_config.borrow().shell_color.clone().into());
            window.set_accent_color_hex(shell_config.borrow().accent_color.clone().into());
            window.set_status(active_format("status_shell", &[("name", &name)]).into());
        }
    });

    let custom_shell_window = window.as_weak();
    let custom_shell_config = Rc::clone(&input_config);
    window.on_set_shell_color(move |value| {
        if is_hex_color(&value) {
            let mut config = custom_shell_config.borrow_mut();
            config.shell_color = value.to_string();
            save_input_config(&config);
            if let Some(window) = custom_shell_window.upgrade() {
                window.set_shell_color(parse_hex_color(&value));
                window.set_shell_color_hex(value);
            }
        }
    });
    let custom_accent_window = window.as_weak();
    let custom_accent_config = Rc::clone(&input_config);
    window.on_set_accent_color(move |value| {
        if is_hex_color(&value) {
            let mut config = custom_accent_config.borrow_mut();
            config.accent_color = value.to_string();
            save_input_config(&config);
            if let Some(window) = custom_accent_window.upgrade() {
                window.set_accent_color(parse_hex_color(&value));
                window.set_accent_color_hex(value);
            }
        }
    });
    let theme_window = window.as_weak();
    let theme_config = Rc::clone(&input_config);
    window.on_select_theme(move |index| {
        let (shell, accent, _) = shell_theme(index as usize);
        let shell_hex = format!(
            "#{:02x}{:02x}{:02x}",
            shell.red(),
            shell.green(),
            shell.blue()
        );
        let accent_hex = format!(
            "#{:02x}{:02x}{:02x}",
            accent.red(),
            accent.green(),
            accent.blue()
        );
        let mut config = theme_config.borrow_mut();
        config.shell_color = shell_hex.clone();
        config.accent_color = accent_hex.clone();
        save_input_config(&config);
        if let Some(window) = theme_window.upgrade() {
            window.set_shell_color(shell);
            window.set_accent_color(accent);
            window.set_shell_color_hex(shell_hex.into());
            window.set_accent_color_hex(accent_hex.into());
        }
    });

    let pause_window = window.as_weak();
    let pause_system = systems.clone();
    window.on_toggle_pause(move || {
        let mut dual = pause_system.borrow_mut();
        let paused = dual.left.toggle_pause();
        if let Some(window) = pause_window.upgrade() {
            let state = if paused {
                active_text("app_paused")
            } else {
                active_text("app_running")
            };
            window.set_status(
                active_format(
                    "status_console_paused",
                    &[("console", "1"), ("state", &state)],
                )
                .into(),
            );
        }
    });

    let console_pause_window = window.as_weak();
    let console_pause_system = systems.clone();
    window.on_toggle_pause_console(move |console| {
        let mut dual = console_pause_system.borrow_mut();
        let paused = match console {
            1 => dual.left.toggle_pause(),
            2 if dual.right_loaded => dual.right.toggle_pause(),
            _ => return,
        };
        if let Some(window) = console_pause_window.upgrade() {
            window.set_status(
                format!(
                    "Console {console} {}",
                    if paused { "paused" } else { "running" }
                )
                .into(),
            );
        }
    });

    let reset_window = window.as_weak();
    let reset_system = systems.clone();
    let reset_frames = Arc::clone(&frames);
    let reset_control = Arc::clone(&frame_control);
    window.on_reset_game(move || {
        let left_generation = reset_control.begin_reset(1);
        let right_generation = reset_control.begin_reset(2);
        let blank = FrameBuffers::white();
        {
            let mut output = reset_frames.lock().expect("frame mutex poisoned");
            output.reset_console(1, left_generation);
            output.reset_console(2, right_generation);
        }
        let blank_image = render_pixels(&blank);
        if let Some(window) = reset_window.upgrade() {
            window.set_frame_left(blank_image.clone());
            window.set_frame_right(blank_image);
        }
        let mut dual = reset_system.borrow_mut();
        dual.left.ppu.take_frame_ready();
        dual.right.ppu.take_frame_ready();
        dual.left.reset();
        dual.right.reset();
        if let Some(window) = reset_window.upgrade() {
            window.set_status(active_format("status_reset", &[("console", "1/2")]).into());
        }
    });

    let second_window = window.as_weak();
    let second_system = systems.clone();
    let second_save_path = Rc::clone(&right_save_path);
    let second_config = Rc::clone(&input_config);
    let second_inputs = Rc::clone(&active_inputs);
    let second_layout = Rc::clone(&layout_request);
    window.on_toggle_second_console(move || {
        let visible = {
            let mut dual = second_system.borrow_mut();
            if dual.right_loaded {
                if let Some(path) = second_save_path.borrow().as_ref() {
                    let _ = save_slots::save_battery(&mut dual.right.mmu, path);
                }
            }
            dual.right_visible = !dual.right_visible;
            dual.cable = LinkCable::new();
            dual.right_visible
        };
        if !visible {
            release_input_source(&second_system, &second_inputs, 2, InputSource::Virtual);
        }
        {
            let mut config = second_config.borrow_mut();
            config.second_console = visible;
            save_input_config(&config);
        }
        if let Some(window) = second_window.upgrade() {
            window.set_second_console_enabled(visible);
            apply_layout_size(
                &window,
                &second_layout,
                visible,
                window.get_show_virtual_controls(),
                true,
            );
            window.set_status(
                if visible {
                    "Console 2 enabled"
                } else {
                    "Console 2 disabled"
                }
                .into(),
            );
        }
    });

    let controls_window = window.as_weak();
    let controls_config = Rc::clone(&input_config);
    let controls_system = systems.clone();
    let controls_inputs = Rc::clone(&active_inputs);
    let controls_layout = Rc::clone(&layout_request);
    window.on_toggle_virtual_controls(move || {
        let mut config = controls_config.borrow_mut();
        config.show_virtual_controls = !config.show_virtual_controls;
        save_input_config(&config);
        if !config.show_virtual_controls {
            release_input_source(&controls_system, &controls_inputs, 1, InputSource::Virtual);
            release_input_source(&controls_system, &controls_inputs, 2, InputSource::Virtual);
        }
        if let Some(window) = controls_window.upgrade() {
            window.set_show_virtual_controls(config.show_virtual_controls);
            apply_layout_size(
                &window,
                &controls_layout,
                window.get_second_console_enabled(),
                config.show_virtual_controls,
                true,
            );
        }
    });

    let translucent_window = window.as_weak();
    let translucent_config = Rc::clone(&input_config);
    window.on_toggle_translucent_shell(move || {
        let mut config = translucent_config.borrow_mut();
        config.translucent_shell = !config.translucent_shell;
        save_input_config(&config);
        if let Some(window) = translucent_window.upgrade() {
            window.set_translucent_shell(config.translucent_shell);
        }
    });

    let tint_window = window.as_weak();
    let tint_config = Rc::clone(&input_config);
    window.on_adjust_translucent_tint(move |delta| {
        let tint = {
            let mut config = tint_config.borrow_mut();
            config.translucent_tint =
                (i32::from(config.translucent_tint) + delta).clamp(25, 90) as u8;
            save_input_config(&config);
            config.translucent_tint
        };
        if let Some(window) = tint_window.upgrade() {
            window.set_translucent_tint(tint as i32);
            refresh_display_tint(&window);
        }
    });

    let grid_window = window.as_weak();
    let grid_config = Rc::clone(&input_config);
    window.on_toggle_lcd_grid(move || {
        let mut config = grid_config.borrow_mut();
        config.lcd_grid = !config.lcd_grid;
        save_input_config(&config);
        if let Some(window) = grid_window.upgrade() {
            window.set_lcd_grid_enabled(config.lcd_grid);
        }
    });

    let scale_window = window.as_weak();
    let scale_config = Rc::clone(&input_config);
    let scale_layout = Rc::clone(&layout_request);
    window.on_set_screen_scale(move |scale| {
        let previous_scale = scale_config.borrow().screen_scale.clamp(1, 5);
        let scale = scale.clamp(1, 5) as u8;
        if std::env::var_os("FOXMOULATOR_WINDOW_TRACE").is_some() {
            eprintln!(
                "FoxMulator scale change: previous={} new={}",
                previous_scale, scale
            );
        }
        let mut config = scale_config.borrow_mut();
        config.screen_scale = scale;
        config.auto_scale = false;
        save_input_config(&config);
        if let Some(window) = scale_window.upgrade() {
            window.set_screen_scale(scale as i32);
            window.set_auto_scale(false);
            apply_layout_size(
                &window,
                &scale_layout,
                window.get_second_console_enabled(),
                window.get_show_virtual_controls(),
                true,
            );
            window.set_status(
                active_format("status_scale_integer", &[("scale", &scale.to_string())]).into(),
            );
        }
    });

    let renderer_config = Rc::clone(&input_config);
    let renderer_window = window.as_weak();
    window.on_set_renderer(move |renderer| {
        let renderer = renderer.to_string();
        if renderer != "opengl" && renderer != "software" {
            return;
        }
        let mut config = renderer_config.borrow_mut();
        config.renderer = renderer;
        save_input_config(&config);
        if let Some(window) = renderer_window.upgrade() {
            window.set_renderer_selection(config.renderer.clone().into());
            window.set_renderer_pending_restart(true);
            window.set_status(active_text("render_restart_required").into());
        }
    });

    let speed_config = Rc::clone(&input_config);
    let speed_window = window.as_weak();
    let speed_atomic = Arc::clone(&emulation_speed);
    window.on_set_boost_speed(move |requested| {
        let Some(selected) = valid_boost_speed(requested) else {
            return;
        };
        let mut config = speed_config.borrow_mut();
        config.boost_speed = selected;
        save_input_config(&config);
        if let Some(window) = speed_window.upgrade() {
            window.set_boost_label(format!("{}×", selected).into());
            window.set_boost_default_speed(selected as i32);
        }
        if speed_atomic.load(Ordering::Acquire) > 1 {
            speed_atomic.store(selected, Ordering::Release);
            if let Some(window) = speed_window.upgrade() {
                window.set_boost_active(selected > 1);
            }
        }
    });

    let boost_config = Rc::clone(&input_config);
    let boost_window = window.as_weak();
    let boost_atomic = Arc::clone(&emulation_speed);
    window.on_toggle_boost(move || {
        let selected = boost_config.borrow().boost_speed.clamp(1, 4);
        if selected == 1 {
            return;
        }
        let active = boost_atomic.load(Ordering::Acquire) > 1;
        boost_atomic.store(if active { 1 } else { selected }, Ordering::Release);
        if let Some(window) = boost_window.upgrade() {
            window.set_boost_active(!active);
            window.set_boost_label(format!("{}×", selected).into());
        }
    });

    let auto_scale_window = window.as_weak();
    let auto_scale_config = Rc::clone(&input_config);
    window.on_toggle_auto_scale(move || {
        let mut config = auto_scale_config.borrow_mut();
        config.auto_scale = !config.auto_scale;
        save_input_config(&config);
        if let Some(window) = auto_scale_window.upgrade() {
            window.set_auto_scale(config.auto_scale);
            window.set_status(
                if config.auto_scale {
                    active_text("status_scale_auto")
                } else {
                    active_text("status_scale_manual")
                }
                .into(),
            );
        }
    });

    let library_panel_window = window.as_weak();
    window.on_toggle_library_panel(move || {
        if let Some(window) = library_panel_window.upgrade() {
            window.set_library_panel_visible(!window.get_library_panel_visible());
        }
    });

    let diagnostics_window = window.as_weak();
    window.on_toggle_diagnostics(move || {
        if let Some(window) = diagnostics_window.upgrade() {
            window.set_diagnostics_open(!window.get_diagnostics_open());
        }
    });

    let settings_window = window.as_weak();
    let settings_pending = Rc::clone(&pending_rebind);
    let settings_blocked = Rc::clone(&rebind_blocked_key);
    let settings_systems = systems.clone();
    let settings_inputs = Rc::clone(&active_inputs);
    let settings_save_path = Rc::clone(&save_path);
    let settings_right_save_path = Rc::clone(&right_save_path);
    let settings_selected_console = Rc::clone(&selected_save_console);
    let settings_selected_slot = Rc::clone(&selected_save_slot);
    window.on_toggle_settings(move || {
        if let Some(window) = settings_window.upgrade() {
            let open = next_settings_open(window.get_settings_open());
            if !open {
                *settings_pending.borrow_mut() = None;
                *settings_blocked.borrow_mut() = None;
                release_input_source(
                    &settings_systems,
                    &settings_inputs,
                    1,
                    InputSource::Keyboard,
                );
                release_input_source(
                    &settings_systems,
                    &settings_inputs,
                    2,
                    InputSource::Keyboard,
                );
            }
            window.set_settings_open(open);
            if open {
                let dual = settings_systems.borrow();
                refresh_save_state_if_active(
                    &window,
                    &dual,
                    settings_save_path.borrow().as_deref(),
                    settings_right_save_path.borrow().as_deref(),
                    *settings_selected_console.borrow(),
                    *settings_selected_slot.borrow(),
                    *settings_selected_console.borrow(),
                );
            }
        }
    });

    #[cfg(feature = "audio")]
    {
        let volume_window = window.as_weak();
        let volume_config = Rc::clone(&input_config);
        let volume_mix = Arc::clone(&audio_mix);
        let volume_runtime_mute = Rc::clone(&runtime_mute);
        window.on_set_master_volume(move |volume| {
            let mut config = volume_config.borrow_mut();
            config.master_volume = volume.clamp(0, 100) as u8;
            let settings = effective_audio_settings(&config, *volume_runtime_mute.borrow());
            save_input_config(&config);
            *volume_mix.lock().expect("audio mix mutex poisoned") = settings;
            if let Some(window) = volume_window.upgrade() {
                window.set_master_volume(config.master_volume as i32);
            }
        });

        let console_volume_window = window.as_weak();
        let console_volume_config = Rc::clone(&input_config);
        let console_volume_mix = Arc::clone(&audio_mix);
        let console_volume_runtime_mute = Rc::clone(&runtime_mute);
        window.on_set_console_volume(move |console, volume| {
            let mut config = console_volume_config.borrow_mut();
            let volume = volume.clamp(0, 100) as u8;
            match console {
                1 => config.console1_volume = volume,
                2 => config.console2_volume = volume,
                _ => return,
            }
            let settings = effective_audio_settings(&config, *console_volume_runtime_mute.borrow());
            save_input_config(&config);
            *console_volume_mix.lock().expect("audio mix mutex poisoned") = settings;
            if let Some(window) = console_volume_window.upgrade() {
                if console == 1 {
                    window.set_console1_volume(volume as i32);
                } else {
                    window.set_console2_volume(volume as i32);
                }
            }
        });

        let mute_window = window.as_weak();
        let mute_config = Rc::clone(&input_config);
        let mute_mix = Arc::clone(&audio_mix);
        let mute_runtime_mute = Rc::clone(&runtime_mute);
        window.on_toggle_console_mute(move |console| {
            let mut config = mute_config.borrow_mut();
            let index = match console {
                1 => 0,
                2 => 1,
                _ => return,
            };
            let runtime_mutes = *mute_runtime_mute.borrow();
            let current = match console {
                1 => config.console1_muted || runtime_mutes[index],
                2 => config.console2_muted || runtime_mutes[index],
                _ => unreachable!(),
            };
            let muted = !current;
            if console == 1 {
                config.console1_muted = muted;
            } else {
                config.console2_muted = muted;
            }
            mute_runtime_mute.borrow_mut()[index] = false;
            let settings = effective_audio_settings(&config, *mute_runtime_mute.borrow());
            save_input_config(&config);
            *mute_mix.lock().expect("audio mix mutex poisoned") = settings;
            if let Some(window) = mute_window.upgrade() {
                if console == 1 {
                    window.set_console1_muted(muted);
                } else {
                    window.set_console2_muted(muted);
                }
            }
        });
    }

    let worker_stats = Arc::new(WorkerStats::new());
    let worker_stop = Arc::new(AtomicBool::new(false));
    #[cfg(feature = "audio")]
    let audio_producer = audio_sink.as_ref().map(AudioSink::producer);
    #[cfg(feature = "audio")]
    let worker_audio_mix = Arc::clone(&audio_mix);
    let worker_thread = {
        let worker_systems = systems.clone();
        let worker_frames = Arc::clone(&frames);
        let worker_frame_control = Arc::clone(&frame_control);
        let worker_stats = Arc::clone(&worker_stats);
        let worker_stop = Arc::clone(&worker_stop);
        let worker_speed = Arc::clone(&emulation_speed);
        std::thread::spawn(move || {
            run_emulation_worker(
                worker_systems,
                worker_frames,
                worker_frame_control,
                worker_stats,
                worker_stop,
                worker_speed,
                #[cfg(feature = "audio")]
                audio_producer,
                #[cfg(feature = "audio")]
                worker_audio_mix,
            );
        })
    };

    let timer = slint::Timer::default();
    let weak = window.as_weak();
    let input_system = systems.clone();
    let timer_inputs = Rc::clone(&active_inputs);
    let timer_layout = Rc::clone(&layout_request);
    let timer_scan_controller = Arc::clone(&library_scan_controller);
    let timer_scan_pending = Rc::clone(&pending_library_scan);
    let timer_scan_paths = Rc::clone(&library_paths);
    let timer_scan_index = Rc::clone(&library_index);
    let timer_scan_state = Rc::clone(&library_state);
    let timer_scan_preferences = Rc::clone(&preferences);
    let display_frames = Arc::clone(&frames);
    let display_stats = Arc::clone(&worker_stats);
    let display_speed = Arc::clone(&emulation_speed);
    let mut last_completed_frames = 0u64;
    let mut last_published = 0u64;
    let mut last_consumed = 0u64;
    let mut last_image_update_us = 0u64;
    let mut last_emulation_us = 0u64;
    let mut last_audio_us = 0u64;
    let mut last_publish_us = 0u64;
    let mut last_systems_wait_us = 0u64;
    let mut last_frame_wait_us = 0u64;
    let mut last_sleep_us = 0u64;
    let mut last_quantum_us = 0u64;
    let mut last_quantum_calls = 0u64;
    let mut last_display_sequence = 0u64;
    let mut last_peripheral_cycles = 0u64;
    let mut last_metric_cycles = 0u64;
    let mut last_metric_peripheral_cycles = 0u64;
    let mut last_frame_time_sum_us = 0u64;
    let mut metric_started = std::time::Instant::now();
    let mut metrics_logged_at = std::time::Instant::now();
    #[cfg(all(feature = "gui", feature = "audio"))]
    let mut audio_samples_start = 0u64;
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(16),
        move || {
            let Some(window) = weak.upgrade() else {
                return;
            };
            // Re-evaluate the active monitor and its logical scale.  The
            // signature in LayoutRequest prevents this maintenance pass from
            // creating a resize loop during ordinary manual resizing.
            apply_layout_size(
                &window,
                &timer_layout,
                window.get_second_console_enabled(),
                window.get_show_virtual_controls(),
                true,
            );
            apply_pending_native_layout(&window, &timer_layout);
            for (generation, event) in timer_scan_controller.take_events() {
                if !timer_scan_controller.is_current(generation) {
                    continue;
                }
                match event {
                    LibraryScanEvent::Progress(progress) => match progress {
                        ScanProgress::Searching => {
                            window.set_library_scan_active(true);
                            window.set_library_scan_indeterminate(true);
                            window.set_library_scan_status(
                                active_text("library_scan_searching").into(),
                            );
                        }
                        ScanProgress::FilesDiscovered { total } => {
                            window.set_library_scan_indeterminate(total == 0);
                            window.set_library_scan_progress(if total == 0 { 1.0 } else { 0.0 });
                        }
                        ScanProgress::Reading { current, total, name } => {
                            let percent = if total == 0 {
                                0
                            } else {
                                current.saturating_mul(100) / total
                            };
                            window.set_library_scan_indeterminate(false);
                            window.set_library_scan_progress(
                                (current as f32 / total.max(1) as f32).clamp(0.0, 1.0),
                            );
                            window.set_library_scan_status(
                                format!(
                                    "{} · {}",
                                    active_format(
                                        "library_scan_reading_file",
                                        &[("name", &name)],
                                    ),
                                    active_format(
                                        "library_scan_file_counter",
                                        &[
                                            ("current", &current.to_string()),
                                            ("total", &total.to_string()),
                                            ("percent", &percent.to_string()),
                                        ],
                                    )
                                )
                                .into(),
                            );
                        }
                        ScanProgress::InspectingZip { current, total, name } => {
                            let percent = if total == 0 {
                                0
                            } else {
                                current.saturating_mul(100) / total
                            };
                            window.set_library_scan_indeterminate(false);
                            window.set_library_scan_progress(
                                (current as f32 / total.max(1) as f32).clamp(0.0, 1.0),
                            );
                            window.set_library_scan_status(
                                format!(
                                    "{} · {}",
                                    active_format(
                                        "library_scan_inspecting_zip",
                                        &[("name", &name)],
                                    ),
                                    active_format(
                                        "library_scan_file_counter",
                                        &[
                                            ("current", &current.to_string()),
                                            ("total", &total.to_string()),
                                            ("percent", &percent.to_string()),
                                        ],
                                    )
                                )
                                .into(),
                            );
                        }
                    },
                    LibraryScanEvent::Completed(report, index) => {
                        let Some((folder, persist)) = timer_scan_pending.borrow_mut().take() else {
                            continue;
                        };
                        *timer_scan_paths.borrow_mut() = report.sources;
                        *timer_scan_index.borrow_mut() = index;
                        let state = timer_scan_state.borrow();
                        set_library_models(&window, &timer_scan_index.borrow(), &state);
                        if persist {
                            let mut preferences = timer_scan_preferences.borrow_mut();
                            preferences.library_folder = Some(folder.to_string_lossy().into_owned());
                            save_preferences(&preferences);
                        }
                        window.set_library_scan_active(false);
                        window.set_library_scan_indeterminate(false);
                        window.set_library_scan_progress(1.0);
                        let completion = active_format(
                            "library_scan_completed",
                            &[
                                ("games", &timer_scan_paths.borrow().len().to_string()),
                                ("omitted", &report.omitted.to_string()),
                                ("errors", &report.errors.to_string()),
                            ],
                        );
                        window.set_library_scan_status(completion.clone().into());
                        window.set_status(completion.into());
                        start_cover_downloads(timer_scan_paths.borrow().clone(), window.as_weak());
                    }
                    LibraryScanEvent::Cancelled => {
                        timer_scan_pending.borrow_mut().take();
                        window.set_library_scan_active(false);
                        window.set_library_scan_status(
                            active_text("library_scan_cancelled").into(),
                        );
                    }
                    LibraryScanEvent::Failed(error) => {
                        timer_scan_pending.borrow_mut().take();
                        window.set_library_scan_active(false);
                        window.set_library_scan_status(
                            active_format("library_scan_error", &[("error", &error)]).into(),
                        );
                    }
                }
            }
            #[cfg(feature = "gamepad")]
            if let Some(router) = gamepads.as_mut() {
                while let Some(event) = router.gilrs.next_event() {
                    if matches!(event.event, gilrs::EventType::Disconnected) {
                        if let Some(console) = router.release(event.id) {
                            release_input_source(
                                &input_system,
                                &timer_inputs,
                                console + 1,
                                InputSource::Gamepad,
                            );
                        }
                        continue;
                    }
                    let Some(console) = router.target(event.id) else {
                        continue;
                    };
                    let Some((button, pressed)) = (match event.event {
                        gilrs::EventType::ButtonPressed(button, _) => Some((button, true)),
                        gilrs::EventType::ButtonReleased(button, _) => Some((button, false)),
                        _ => None,
                    }) else {
                        continue;
                    };
                    let Some(button) = gamepad_button(button) else {
                        continue;
                    };
                    set_input_button(
                        &input_system,
                        &timer_inputs,
                        console + 1,
                        InputSource::Gamepad,
                        button,
                        pressed,
                    );
                }
            }
            let latest_frames = {
                let output = display_frames.lock().expect("frame mutex poisoned");
                if output.sequence == last_display_sequence {
                    None
                } else {
                    let snapshot = Some((output.left.clone(), output.right.clone(), output.sequence));
                    // The worker can only replace the slot while holding the
                    // same mutex, so clearing this flag here makes overwrite
                    // accounting describe the actual one-slot handoff.
                    display_stats.frame_pending.store(false, Ordering::Release);
                    snapshot
                }
            };
            if let Some((left, right, sequence)) = latest_frames {
                // Pixel conversion and Slint image construction are deliberately
                // outside the publication mutex.
                let image_update_started = std::time::Instant::now();
                if std::env::var_os("FOXMOULATOR_DISABLE_IMAGES").is_none() {
                    window.set_frame_left(render_pixels(&left));
                    window.set_frame_right(render_pixels(&right));
                }
                last_image_update_us = image_update_started.elapsed().as_micros() as u64;
                last_display_sequence = sequence;
                display_stats.consumed.fetch_add(1, Ordering::Relaxed);
            }
            {
                let dual = input_system.borrow();
                window
                    .set_state_left(visible_state_label(dual.left_loaded, dual.left.state).into());
                window.set_state_right(
                    visible_state_label(dual.right_loaded, dual.right.state).into(),
                );
                window.set_left_loaded(dual.left_loaded);
                window.set_right_loaded(dual.right_loaded);
                window.set_second_console_enabled(dual.right_visible);
                window.set_link_enabled(dual.link_enabled);
            }
            let completed_frames_now = display_stats.completed_frames.load(Ordering::Relaxed);
            if completed_frames_now.saturating_sub(last_completed_frames) >= 30 {
                let elapsed = metric_started.elapsed().as_secs_f64().max(0.001);
                let cycles_now = display_stats.cycles.load(Ordering::Relaxed);
                let peripheral_cycles_now =
                    display_stats.peripheral_cycles.load(Ordering::Relaxed);
                let speed = peripheral_cycles_now
                    .saturating_sub(last_peripheral_cycles) as f64
                    / 4_194_304.0
                    / elapsed
                    * 100.0;
                let drops = {
                    #[cfg(all(feature = "gui", feature = "audio"))]
                    {
                        audio_sink
                            .as_ref()
                            .map(|sink| sink.producer.dropped_chunks.load(Ordering::Relaxed))
                            .unwrap_or(0)
                    }
                    #[cfg(not(all(feature = "gui", feature = "audio")))]
                    {
                        0
                    }
                };
                let (queue_ms, underruns, max_queue_ms) = {
                    #[cfg(all(feature = "gui", feature = "audio"))]
                    {
                        let queue = audio_sink
                            .as_ref()
                            .map(|sink| sink.producer.ring.len())
                            .unwrap_or(0);
                        let max_queue = audio_sink
                            .as_ref()
                            .map(|sink| sink.producer.max_queued_samples.load(Ordering::Relaxed))
                            .unwrap_or(0);
                        let underruns = audio_sink
                            .as_ref()
                            .map(|sink| sink.underruns.load(Ordering::Relaxed))
                            .unwrap_or(0);
                        (
                            queue as f64 / (44_100.0 * 2.0) * 1000.0,
                            underruns,
                            max_queue as f64 / (44_100.0 * 2.0) * 1000.0,
                        )
                    }
                    #[cfg(not(all(feature = "gui", feature = "audio")))]
                    {
                        (0.0, 0, 0.0)
                    }
                };
                let elapsed_frame_us = display_stats.frame_time_sum_us.load(Ordering::Relaxed);
                let max_frame_us = display_stats.frame_time_max_us.swap(0, Ordering::Relaxed);
                let interval_frames = completed_frames_now
                    .saturating_sub(last_completed_frames)
                    .max(1);
                let emulated_fps = rate_per_second(interval_frames, metric_started.elapsed());
                let published_now = display_stats.published.load(Ordering::Relaxed);
                let consumed_now = display_stats.consumed.load(Ordering::Relaxed);
                let published_fps = rate_per_second(
                    published_now.saturating_sub(last_published),
                    metric_started.elapsed(),
                );
                let consumed_fps = rate_per_second(
                    consumed_now.saturating_sub(last_consumed),
                    metric_started.elapsed(),
                );
                let avg_frame_ms = elapsed_frame_us.saturating_sub(last_frame_time_sum_us) as f64
                    / interval_frames as f64
                    / 1000.0;
                let max_frame_ms = max_frame_us as f64 / 1000.0;
                let (audio_rate, _audio_samples_end) = {
                    #[cfg(all(feature = "gui", feature = "audio"))]
                    {
                        let samples = audio_sink
                            .as_ref()
                            .map(|sink| sink.written_samples.load(Ordering::Relaxed))
                            .unwrap_or(0);
                        (
                            samples.saturating_sub(audio_samples_start) as f64 / elapsed,
                            samples,
                        )
                    }
                    #[cfg(not(all(feature = "gui", feature = "audio")))]
                    {
                        (0.0, 0)
                    }
                };
                let (left_state, right_loaded, right_visible) = {
                    let dual = input_system.borrow();
                    (
                        visible_state_label(dual.left_loaded, dual.left.state),
                        dual.right_loaded,
                        dual.right_visible,
                    )
                };
                let right_state = if right_loaded {
                    if right_visible {
                        active_text("diagnostics_console_on")
                    } else {
                        active_text("diagnostics_console_hidden")
                    }
                } else {
                    active_text("diagnostics_console_empty")
                };
                window.set_diagnostics(
                    active_format(
                        "diagnostics_summary",
                        &[
                            ("c1", &left_state),
                            ("speed", &format!("{speed:.1}")),
                            ("avg_frame", &format!("{avg_frame_ms:.2}")),
                            ("max_frame", &format!("{max_frame_ms:.2}")),
                            ("audio_rate", &format!("{audio_rate:.0}")),
                            ("queue", &format!("{queue_ms:.1}")),
                            ("max_queue", &format!("{max_queue_ms:.1}")),
                            ("underruns", &underruns.to_string()),
                            ("drops", &drops.to_string()),
                            ("c2", &right_state),
                        ],
                    )
                    .into(),
                );
                if metrics_logged_at.elapsed() >= std::time::Duration::from_secs(1) {
                    let emulation_us = display_stats.emulation_us.load(Ordering::Relaxed);
                    let audio_us = display_stats.audio_us.load(Ordering::Relaxed);
                    let publish_us = display_stats.publish_us.load(Ordering::Relaxed);
                    let systems_wait_us = display_stats.systems_wait_us.load(Ordering::Relaxed);
                    let frame_wait_us = display_stats.frame_wait_us.load(Ordering::Relaxed);
                    let sleep_us = display_stats.sleep_us.load(Ordering::Relaxed);
                    let quantum_us = display_stats.quantum_us.load(Ordering::Relaxed);
                    let quantum_calls = display_stats.quantum_calls.load(Ordering::Relaxed);
                    let interval_elapsed = metrics_logged_at.elapsed().as_secs_f64().max(0.001);
                    let cpu_cycles_s = cycles_now
                        .saturating_sub(last_metric_cycles) as f64
                        / interval_elapsed;
                    let peripheral_cycles_s = peripheral_cycles_now
                        .saturating_sub(last_metric_peripheral_cycles) as f64
                        / interval_elapsed;
                    eprintln!(
                        "FoxMulator metrics: requested={}x emulated_fps={:.1} effective_speed={:.1}x cpu_cycles_s={:.0} peripheral_cycles_s={:.0} published_fps={:.1} consumed_fps={:.1} image_update_ms={:.2} phase_ms=(emu:{:.2} audio:{:.2} pub:{:.2} syswait:{:.2} framewait:{:.2} sleep:{:.2} quantum:{:.2}) quantum_calls={}",
                        display_speed.load(Ordering::Relaxed),
                        emulated_fps,
                        peripheral_cycles_s / 4_194_304.0,
                        cpu_cycles_s,
                        peripheral_cycles_s,
                        published_fps,
                        consumed_fps,
                        last_image_update_us as f64 / 1000.0,
                        (emulation_us.saturating_sub(last_emulation_us) as f64) / 1000.0,
                        (audio_us.saturating_sub(last_audio_us) as f64) / 1000.0,
                        (publish_us.saturating_sub(last_publish_us) as f64) / 1000.0,
                        (systems_wait_us.saturating_sub(last_systems_wait_us) as f64) / 1000.0,
                        (frame_wait_us.saturating_sub(last_frame_wait_us) as f64) / 1000.0,
                        (sleep_us.saturating_sub(last_sleep_us) as f64) / 1000.0,
                        (quantum_us.saturating_sub(last_quantum_us) as f64) / 1000.0,
                        quantum_calls.saturating_sub(last_quantum_calls),
                    );
                    last_emulation_us = emulation_us;
                    last_audio_us = audio_us;
                    last_publish_us = publish_us;
                    last_systems_wait_us = systems_wait_us;
                    last_frame_wait_us = frame_wait_us;
                    last_sleep_us = sleep_us;
                    last_quantum_us = quantum_us;
                    last_quantum_calls = quantum_calls;
                    last_metric_cycles = cycles_now;
                    last_metric_peripheral_cycles = peripheral_cycles_now;
                    metrics_logged_at = std::time::Instant::now();
                }
                last_completed_frames = completed_frames_now;
                last_published = published_now;
                last_consumed = consumed_now;
                last_peripheral_cycles = display_stats.peripheral_cycles.load(Ordering::Relaxed);
                last_frame_time_sum_us = elapsed_frame_us;
                metric_started = std::time::Instant::now();
                #[cfg(all(feature = "gui", feature = "audio"))]
                {
                    audio_samples_start = _audio_samples_end;
                }
            }
        },
    );

    // Queue startup scanning only after all UI callbacks and the repaint timer
    // are installed. The event loop can therefore present the first frame
    // before a slow USB directory is touched by the worker.
    if library_root.is_dir() {
        let startup_window = window.as_weak();
        let startup_controller = Arc::clone(&library_scan_controller);
        let startup_root = library_root.clone();
        let startup_state = library_state.borrow().clone();
        *pending_library_scan.borrow_mut() = Some((startup_root.clone(), false));
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(window) = startup_window.upgrade() {
                window.set_library_scan_active(true);
                window.set_library_scan_indeterminate(true);
                window.set_library_scan_progress(0.0);
                window.set_library_scan_status(active_text("library_scan_searching").into());
                startup_controller.start(startup_root, startup_state);
            }
        });
    }

    let result = window.run();
    worker_stop.store(true, Ordering::Release);
    let _ = worker_thread.join();
    if let Some(path) = save_path.borrow().as_ref() {
        let mut dual = systems.borrow_mut();
        let _ = save_slots::save_battery(&mut dual.left.mmu, path);
        if let Some(right_path) = right_save_path.borrow().as_ref() {
            let _ = save_slots::save_battery(&mut dual.right.mmu, right_path);
        }
    }
    result
}

#[cfg(not(feature = "gui"))]
fn main() {
    let cli = Cli::parse();
    let launch = match validate_cli(cli) {
        Ok(launch) => launch,
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(2);
        }
    };
    let (mut system, save_path) = load_system(launch.rom1.as_ref());
    let cycles = system.step_frame(100_000);
    if let Some(path) = save_path {
        let _ = save_slots::save_battery(&mut system.mmu, &path);
    }
    println!(
        "Headless run: executed {cycles} T-cycles, PC=0x{:04X}",
        system.cpu.pc
    );
}
