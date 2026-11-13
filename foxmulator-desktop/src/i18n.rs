//! Runtime internationalization for the desktop front end.
//!
//! Spanish and US English are compiled into the executable.  XML catalogs are
//! optional extensions discovered below the user's configuration directory.

use quick_xml::events::Event;
use quick_xml::Reader;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

pub const SPANISH_ID: &str = "ES";
pub const ENGLISH_ID: &str = "EN-US";

/// Stable keys shared by the embedded catalogs, the XML template, and UI code.
pub const KEYS: &[&str] = &[
    "app_title",
    "app_load_rom",
    "app_no_cartridge",
    "app_running",
    "app_paused",
    "app_stopped",
    "footer_tagline",
    "menu_settings",
    "menu_close",
    "menu_language",
    "menu_reload_languages",
    "section_game_cartridges",
    "section_save_states",
    "section_screen",
    "section_appearance",
    "section_sound",
    "section_game_link",
    "section_controls",
    "section_advanced",
    "quick_save",
    "quick_load",
    "save_state",
    "load_state",
    "delete_state",
    "load_state_file",
    "slot_empty",
    "slot_occupied",
    "slot_0",
    "slot_1",
    "slot_2",
    "slot_3",
    "slot_4",
    "slot_5",
    "slot_6",
    "slot_7",
    "slot_8",
    "slot_9",
    "slot_10",
    "state_saved",
    "state_loaded",
    "console_one",
    "console_two",
    "console_load",
    "console_reset",
    "console_eject",
    "console_pause",
    "screen_show_controls",
    "screen_show_second_console",
    "screen_show_grid",
    "screen_auto_scale",
    "screen_scale",
    "render_engine",
    "render_opengl",
    "render_software",
    "render_restart_required",
    "emulation_speed",
    "emulation_speed_native",
    "emulation_speed_default_boost",
    "emulation_speed_boost_tooltip",
    "appearance_change_shell",
    "appearance_library",
    "appearance_translucent",
    "display_tint_intensity",
    "appearance_themes",
    "appearance_custom_color",
    "appearance_shell",
    "appearance_accents",
    "sound_general",
    "sound_resume",
    "sound_mute",
    "sound_description",
    "link_connected",
    "link_disconnected",
    "link_manual_clock",
    "controls_keys",
    "controls_quick_save",
    "controls_quick_load",
    "controls_instructions",
    "controls_restore",
    "controls_binding_summary",
    "control_select",
    "control_start",
    "advanced_diagnostics",
    "library_title",
    "library_search",
    "library_scan",
    "library_other_folder",
    "library_games",
    "library_of",
    "library_empty",
    "library_no_results",
    "library_internal",
    "library_edit",
    "library_name",
    "library_search_name_cover",
    "library_change_cover",
    "library_automatic",
    "library_results",
    "library_save",
    "library_cancel",
    "library_scan_searching",
    "library_scan_reading_file",
    "library_scan_inspecting_zip",
    "library_scan_file_counter",
    "library_scan_completed",
    "library_scan_cancel",
    "library_scan_cancelled",
    "library_scan_error",
    "library_favorite",
    "library_cover_invalid",
    "library_cover_ready",
    "status_loaded",
    "status_unable_read_rom",
    "status_no_cartridge",
    "status_reset",
    "status_ejected",
    "status_saved",
    "status_loaded_state",
    "status_unable_state",
    "status_library_unavailable",
    "status_library_folder",
    "status_controls_saved",
    "status_invalid_key",
    "status_defaults_restored",
    "status_select_folder",
    "status_cover_saved",
    "status_downloading_cover",
    "status_language_reloaded",
    "status_language_invalid",
    "manual_slots",
    "tint_intensity_prefix",
    "theme_atomic",
    "theme_grape",
    "theme_ice",
    "theme_ocean",
    "theme_cherry",
    "theme_turquoise",
    "theme_solar",
    "theme_smoke",
    "theme_crystal",
    "theme_graphite",
    "theme_lime",
    "status_capture_cancelled",
    "status_state_wrong_rom",
    "status_rebind_prompt",
    "status_rebind_conflict",
    "status_console_ejected_link",
    "status_console_ejected",
    "status_battery_written",
    "status_no_battery",
    "status_operation_error",
    "status_library_search_prompt",
    "status_library_search_results",
    "status_favorite_added",
    "status_favorite_removed",
    "status_slot_saved",
    "status_slot_save_failed",
    "status_slot_loaded",
    "status_slot_loaded_link_disabled",
    "status_slot_load_failed",
    "status_slot_deleted",
    "status_slot_delete_failed",
    "status_invalid_slot",
    "dialog_overwrite_state_title",
    "dialog_overwrite_state_description",
    "dialog_delete_state_title",
    "dialog_delete_state_description",
    "dialog_import_battery_title",
    "dialog_import_battery_description",
    "dialog_load_state_title",
    "dialog_load_state_description",
    "dialog_save_copy_title",
    "dialog_save_copy_description",
    "dialog_overwrite_slot_title",
    "dialog_overwrite_slot_description",
    "status_file_unrecognized",
    "status_sram_incompatible",
    "status_import_failed",
    "status_battery_imported",
    "status_external_state_incompatible",
    "status_external_state_loaded",
    "status_external_state_imported",
    "status_serial_clock_applied",
    "status_serial_clock_waiting",
    "status_link_enabled",
    "status_link_disabled",
    "status_shell",
    "status_console_paused",
    "status_second_console_enabled",
    "status_second_console_disabled",
    "status_scale_integer",
    "status_scale_auto",
    "status_scale_manual",
    "diagnostics_summary",
    "diagnostics_console_on",
    "diagnostics_console_hidden",
    "diagnostics_console_empty",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LanguageInfo {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug)]
struct Catalog {
    info: LanguageInfo,
    strings: HashMap<String, String>,
}

#[derive(Debug)]
pub struct LanguageManager {
    lang_dir: PathBuf,
    catalogs: HashMap<String, Catalog>,
    languages: Vec<LanguageInfo>,
    selected: String,
    diagnostics: Vec<String>,
}

impl LanguageManager {
    pub fn from_environment() -> Self {
        let config = config_dir();
        let lang_dir = config.join("lang");
        let _ = fs::create_dir_all(&lang_dir);
        let _ = write_template_if_missing(&lang_dir);
        Self::from_directory(lang_dir)
    }

    pub fn from_directory(lang_dir: PathBuf) -> Self {
        let mut manager = Self {
            lang_dir,
            catalogs: HashMap::new(),
            languages: Vec::new(),
            selected: SPANISH_ID.to_owned(),
            diagnostics: Vec::new(),
        };
        manager.register_embedded();
        manager.reload();
        manager
    }

    fn register_embedded(&mut self) {
        self.catalogs.insert(
            SPANISH_ID.to_owned(),
            Catalog {
                info: LanguageInfo {
                    id: SPANISH_ID.into(),
                    name: "Español".into(),
                },
                strings: embedded_spanish(),
            },
        );
        self.catalogs.insert(
            ENGLISH_ID.to_owned(),
            Catalog {
                info: LanguageInfo {
                    id: ENGLISH_ID.into(),
                    name: "English (United States)".into(),
                },
                strings: embedded_english(),
            },
        );
    }

    pub fn reload(&mut self) {
        let previous = self.selected.clone();
        self.register_embedded();
        self.diagnostics.clear();
        let _ = fs::create_dir_all(&self.lang_dir);
        let mut files = fs::read_dir(&self.lang_dir)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .and_then(|x| x.to_str())
                    .is_some_and(|x| x.eq_ignore_ascii_case("xml"))
            })
            .collect::<Vec<_>>();
        files.sort();
        for path in files {
            match parse_catalog(&path) {
                Ok(catalog) if catalog.info.id.eq_ignore_ascii_case(ENGLISH_ID) => {
                    // EN-US.xml is a user-editable template, never an override.
                }
                Ok(catalog) => {
                    let normalized = catalog.info.id.to_ascii_uppercase();
                    if self
                        .catalogs
                        .keys()
                        .any(|id| id.eq_ignore_ascii_case(&normalized))
                    {
                        self.diagnostics
                            .push(format!("duplicate language id in {}", path.display()));
                    } else {
                        self.catalogs.insert(normalized, catalog);
                    }
                }
                Err(error) => self
                    .diagnostics
                    .push(format!("{}: {error}", path.display())),
            }
        }
        self.languages = self
            .catalogs
            .values()
            .map(|catalog| catalog.info.clone())
            .collect();
        self.languages
            .sort_by_key(|language| language.name.to_lowercase());
        self.selected = self
            .languages
            .iter()
            .find(|language| language.id.eq_ignore_ascii_case(&previous))
            .map_or_else(|| SPANISH_ID.into(), |language| language.id.clone());
    }

    pub fn languages(&self) -> &[LanguageInfo] {
        &self.languages
    }
    pub fn selected_id(&self) -> &str {
        &self.selected
    }
    pub fn selected_index(&self) -> i32 {
        self.languages
            .iter()
            .position(|x| x.id == self.selected)
            .unwrap_or(0) as i32
    }
    #[allow(dead_code)]
    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }
    #[allow(dead_code)]
    pub fn select_index(&mut self, index: i32) -> bool {
        let Some(language) = self.languages.get(index.max(0) as usize) else {
            return false;
        };
        self.selected = language.id.clone();
        true
    }

    pub fn select_id(&mut self, id: &str) -> bool {
        let Some(language) = self
            .languages
            .iter()
            .find(|language| language.id.eq_ignore_ascii_case(id))
        else {
            return false;
        };
        self.selected = language.id.clone();
        true
    }

    pub fn select_name(&mut self, name: &str) -> bool {
        let Some(language) = self.languages.iter().find(|language| language.name == name) else {
            return false;
        };
        self.selected = language.id.clone();
        true
    }

    pub fn text(&self, key: &str) -> String {
        let key = canonical_key(key);
        self.lookup(&self.selected, &key)
            .or_else(|| self.lookup_legacy_slot(&self.selected, &key))
            .or_else(|| self.lookup(SPANISH_ID, &key))
            .or_else(|| self.lookup_legacy_slot(SPANISH_ID, &key))
            .unwrap_or_else(|| format!("⟦{key}⟧"))
    }

    #[allow(dead_code)]
    pub fn format(&self, key: &str, params: &[(&str, &str)]) -> String {
        let canonical = canonical_key(key);
        let mut text = self
            .lookup(&self.selected, &canonical)
            .or_else(|| self.lookup_legacy_slot(&self.selected, &canonical))
            .or_else(|| self.lookup(SPANISH_ID, &canonical))
            .or_else(|| self.lookup_legacy_slot(SPANISH_ID, &canonical))
            .unwrap_or_else(|| format!("⟦{canonical}⟧"));
        for (name, value) in params {
            text = text.replace(&format!("{{{name}}}"), value);
        }
        text
    }

    fn lookup_legacy_slot(&self, id: &str, key: &str) -> Option<String> {
        if key
            .strip_prefix("slot_")
            .is_some_and(|suffix| suffix.parse::<u8>().is_ok())
        {
            self.lookup(id, "state_slot")
        } else {
            None
        }
    }

    fn lookup(&self, id: &str, key: &str) -> Option<String> {
        let catalog = self.catalogs.get(id)?;
        if let Some(value) = catalog.strings.get(key) {
            return Some(value.clone());
        }
        let base = id.split('-').next().unwrap_or(id);
        let regional = if base.eq_ignore_ascii_case("en") {
            ENGLISH_ID
        } else if base.eq_ignore_ascii_case("es") {
            SPANISH_ID
        } else {
            return None;
        };
        self.catalogs
            .get(regional)
            .and_then(|fallback| fallback.strings.get(key).cloned())
    }
}

pub fn config_dir() -> PathBuf {
    if let Ok(value) = std::env::var("XDG_CONFIG_HOME") {
        if !value.trim().is_empty() {
            return PathBuf::from(value).join("foxmulator");
        }
    }
    directories::BaseDirs::new()
        .map(|dirs| dirs.config_dir().join("foxmulator"))
        .unwrap_or_else(|| PathBuf::from("foxmulator"))
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphanumeric() || (index > 0 && (byte == b'-' || byte == b'_'))
        })
}

fn canonical_key(key: &str) -> String {
    key.replace('.', "_")
}

fn valid_key(key: &str) -> bool {
    key.as_bytes()
        .first()
        .is_some_and(|byte| byte.is_ascii_lowercase())
        && key.len() <= 64
        && key
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn parse_catalog(path: &Path) -> Result<Catalog, String> {
    let file_id = path
        .file_stem()
        .and_then(|x| x.to_str())
        .ok_or("invalid filename")?;
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    let mut reader = Reader::from_reader(bytes.as_slice());
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut current = String::new();
    let mut language_id = None;
    let mut language_name = None;
    let mut strings = HashMap::new();
    loop {
        match reader
            .read_event_into(&mut buffer)
            .map_err(|error| error.to_string())?
        {
            Event::Start(event) if event.name().as_ref() == b"String" => {
                let id = event
                    .attributes()
                    .flatten()
                    .find(|attribute| attribute.key.as_ref() == b"id")
                    .and_then(|attribute| String::from_utf8(attribute.value.into_owned()).ok())
                    .ok_or("String is missing id")?;
                let key = canonical_key(&id);
                if !valid_key(&key) || strings.contains_key(&key) {
                    return Err(format!("duplicate or invalid key: {id}"));
                }
                let value = reader
                    .read_text(event.name())
                    .map_err(|error| error.to_string())?
                    .decode()
                    .map_err(|error| error.to_string())?
                    .into_owned();
                strings.insert(key, value);
                current.clear();
            }
            Event::Start(event) => {
                current = String::from_utf8_lossy(event.name().as_ref()).into_owned()
            }
            Event::Text(text) => {
                let value = text
                    .decode()
                    .map_err(|error| error.to_string())?
                    .into_owned();
                match current.as_str() {
                    "LanguageID" => language_id = Some(value),
                    "LanguageName" => language_name = Some(value),
                    "String" => return Err("String elements require an id attribute".into()),
                    _ => {}
                }
            }
            Event::Empty(event) if event.name().as_ref() == b"String" => {
                return Err("String elements cannot be empty".into())
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    let id = language_id.ok_or("missing LanguageID")?;
    let name = language_name.ok_or("missing LanguageName")?;
    if !valid_id(&id) {
        return Err("unsafe LanguageID".into());
    }
    if name.trim().is_empty() {
        return Err("empty LanguageName".into());
    }
    if !file_id.eq_ignore_ascii_case(&id) {
        return Err("filename does not match LanguageID".into());
    }
    let embedded = embedded_english();
    for (key, value) in &strings {
        if let Some(reference) = embedded.get(key) {
            if parameter_names(reference) != parameter_names(value) {
                return Err(format!("parameters do not match for key: {key}"));
            }
        }
    }
    Ok(Catalog {
        info: LanguageInfo {
            id: id.to_ascii_uppercase(),
            name,
        },
        strings,
    })
}

fn write_template_if_missing(lang_dir: &Path) -> std::io::Result<()> {
    let path = lang_dir.join("EN-US.xml");
    if path.exists() {
        return Ok(());
    }
    fs::write(
        path,
        embedded_xml(ENGLISH_ID, "English (United States)", &embedded_english()),
    )
}

fn embedded_xml(id: &str, name: &str, strings: &HashMap<String, String>) -> String {
    let mut output = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<Language>\n    <LanguageID>{id}</LanguageID>\n    <LanguageName>{name}</LanguageName>\n");
    for key in KEYS {
        output.push_str(&format!(
            "    <String id=\"{key}\">{}</String>\n",
            xml_escape(strings.get(*key).map_or("", String::as_str))
        ));
    }
    output.push_str("</Language>\n");
    output
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn parameter_names(value: &str) -> Vec<String> {
    let mut names = value
        .split('{')
        .skip(1)
        .filter_map(|part| part.split('}').next())
        .filter(|name| {
            !name.is_empty()
                && name
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric() || character == '_')
        })
        .map(str::to_owned)
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    names
}

fn embedded_spanish() -> HashMap<String, String> {
    let mut catalog = embedded_catalog(&[
    ("render_engine", "Motor de renderizado"), ("render_opengl", "OpenGL"), ("render_software", "Software"), ("render_restart_required", "El cambio se aplicará al reiniciar"), ("emulation_speed", "Velocidad de emulación"), ("emulation_speed_native", "Nativa"), ("emulation_speed_default_boost", "Aceleración predeterminada"), ("emulation_speed_boost_tooltip", "Activar o desactivar aceleración"),
    ("app_title", "FoxMulator Next"), ("app_load_rom", "Carga una ROM desde Opciones"), ("app_no_cartridge", "Sin cartucho"), ("app_running", "Ejecutando"), ("app_paused", "Pausado"),
    ("menu_settings", "Opciones"), ("menu_close", "Cerrar"), ("menu_language", "IDIOMA"), ("menu_reload_languages", "Recargar idiomas"), ("section_game_cartridges", "JUEGO Y CARTUCHOS"), ("section_save_states", "ESTADOS GUARDADOS"), ("section_screen", "PANTALLA"), ("section_appearance", "APARIENCIA"), ("section_sound", "SONIDO"), ("section_game_link", "GAME LINK"), ("section_controls", "CONTROLES"), ("section_advanced", "AVANZADO"),
    ("quick_save", "Quick Save"), ("quick_load", "Quick Load"), ("save_state", "Guardar"), ("load_state", "Cargar"), ("delete_state", "Borrar"), ("load_state_file", "Cargar desde archivo..."), ("slot_empty", "Vacía"), ("slot_occupied", "Ocupada"), ("slot_0", "Ranura 0 · Guardado rápido"), ("slot_1", "Ranura 1"), ("slot_2", "Ranura 2"), ("slot_3", "Ranura 3"), ("slot_4", "Ranura 4"), ("slot_5", "Ranura 5"), ("slot_6", "Ranura 6"), ("slot_7", "Ranura 7"), ("slot_8", "Ranura 8"), ("slot_9", "Ranura 9"), ("slot_10", "Ranura 10"), ("state_saved", "Estado guardado en {path}"), ("state_loaded", "Estado cargado desde {path}"),
    ("console_one", "Consola 1"), ("console_two", "Consola 2"), ("console_load", "Cargar"), ("console_reset", "Reiniciar"), ("console_eject", "Expulsar ROM"), ("console_pause", "Pausa"), ("screen_show_controls", "Mostrar controles en pantalla"), ("screen_show_second_console", "Mostrar segunda consola"), ("screen_show_grid", "Mostrar cuadrícula (solo escala 3×+)"), ("screen_auto_scale", "Escala automática entera"), ("screen_scale", "Escala:"),
    ("appearance_change_shell", "Cambiar carcasa"), ("appearance_library", "Biblioteca"), ("appearance_translucent", "Acabado translúcido"), ("display_tint_intensity", "Intensidad del tinte: {value}%"), ("appearance_themes", "TEMAS ORIGINALES"), ("appearance_custom_color", "COLOR PERSONALIZADO (HEX)"), ("appearance_shell", "Carcasa"), ("appearance_accents", "Acentos"), ("sound_general", "General"), ("sound_resume", "Reactivar"), ("sound_mute", "Silenciar"), ("sound_description", "Las ganancias se aplican al PCM de cada consola antes de la mezcla, sin afectar la emulación."),
    ("link_connected", "Cable Link conectado"), ("link_disconnected", "Cable Link desconectado"), ("link_manual_clock", "Reloj serial manual (diagnóstico)"), ("controls_keys", "Teclas"), ("controls_quick_save", "Quick Save"), ("controls_quick_load", "Quick Load"), ("controls_instructions", "Elegí un control y presioná la nueva tecla. Escape cancela; Backspace y Enter también se pueden asignar."), ("controls_restore", "Restaurar controles predeterminados de C1"), ("advanced_diagnostics", "Diagnósticos de rendimiento"),
    ("library_title", "BIBLIOTECA · elegí C1 o C2"), ("library_search", "Buscar juego..."), ("library_scan", "Scan"), ("library_other_folder", "Otra carpeta"), ("library_games", "juegos"), ("library_of", "de"), ("library_empty", "No hay juegos en la biblioteca"), ("library_no_results", "No se encontraron juegos · limpiá la búsqueda"), ("library_internal", "Interno: {value}"), ("library_edit", "Editar juego"), ("library_name", "Nombre comercial"), ("library_search_name_cover", "Buscar nombre y portada"), ("library_change_cover", "Cambiar carátula"), ("library_automatic", "Automática"), ("library_results", "Resultados del catálogo"), ("library_save", "Guardar"), ("library_cancel", "Cancelar"), ("library_scan_searching", "Buscando archivos…"), ("library_scan_reading_file", "Leyendo: {name}"), ("library_scan_inspecting_zip", "Inspeccionando ZIP: {name}"), ("library_scan_file_counter", "Archivo {current} de {total} · {percent}%"), ("library_scan_completed", "Escaneo completo: {games} juegos · {omitted} omitidos · {errors} errores"), ("library_scan_cancel", "Cancelar"), ("library_scan_cancelled", "Escaneo cancelado; se conserva la biblioteca anterior"), ("library_scan_error", "No se pudo escanear la biblioteca: {error}"), ("library_favorite", "Favorito"), ("library_cover_invalid", "La imagen no es compatible"), ("library_cover_ready", "Portada local lista; guardá para confirmar"),
    ("status_loaded", "ROM cargada: {path}"), ("status_unable_read_rom", "No se pudo leer la ROM"), ("status_no_cartridge", "No hay cartucho disponible"), ("status_reset", "Consola {console} reiniciada"), ("status_ejected", "ROM expulsada"), ("status_saved", "Estado guardado"), ("status_loaded_state", "Estado cargado"), ("status_unable_state", "No se pudo guardar/cargar el estado"), ("status_library_unavailable", "Carpeta de biblioteca no disponible: {path}"), ("status_library_folder", "Biblioteca: {path}"), ("status_controls_saved", "Controles guardados"), ("status_invalid_key", "Tecla no válida; se conserva la asignación"), ("status_defaults_restored", "Controles predeterminados de C1 restaurados"), ("status_select_folder", "Elegí una carpeta con Otra carpeta"), ("status_cover_saved", "Ficha guardada"), ("status_downloading_cover", "Descargando portada seleccionada…"), ("status_language_reloaded", "Idiomas recargados"), ("status_language_invalid", "Idioma no disponible; se usó Español"), ("manual_slots", "RANURAS MANUALES"), ("tint_intensity_prefix", "Intensidad del tinte:"), ("theme_atomic", "Atómico"), ("theme_grape", "Uva"), ("theme_ice", "Hielo"), ("theme_ocean", "Océano"), ("theme_cherry", "Cereza"), ("theme_turquoise", "Turquesa"), ("theme_solar", "Solar"), ("theme_smoke", "Humo"), ("theme_crystal", "Cristal"), ("theme_graphite", "Grafito"), ("theme_lime", "Lima"), ("status_capture_cancelled", "Captura cancelada"), ("status_state_wrong_rom", "El Save State pertenece a otra ROM"),
    ]);
    catalog.extend(embedded_catalog(&[
        ("app_stopped", "Detenido"),
        ("footer_tagline", "FOX // 01  •  Juega donde quieras"),
        ("controls_binding_summary", "Teclas: Izquierda={left} Derecha={right} Arriba={up} Abajo={down} | A={a} B={b} Select={select} Start={start} | Guardado rápido={quick_save} Carga rápida={quick_load} | Mando {gamepad}"),
        ("control_select", "Select"),
        ("control_start", "Start"),
        ("status_rebind_prompt", "Presioná una tecla para C{console} {action} (Escape cancela)"),
        ("status_rebind_conflict", "Tecla ya asignada a C{console} {action}; elegí otra"),
        ("status_console_ejected_link", "Consola {console} expulsada; {battery}; Game Link esperando cartucho"),
        ("status_console_ejected", "Cartucho de Consola {console} expulsado; {battery}"),
        ("status_battery_written", "partida guardada: {path}"),
        ("status_no_battery", "sin partida de batería"),
        ("status_operation_error", "Error: {error}"),
        ("status_library_search_prompt", "Escribí un nombre para buscar"),
        ("status_library_search_results", "Resultados del catálogo"),
        ("status_favorite_added", "Agregado a favoritos"),
        ("status_favorite_removed", "Quitado de favoritos"),
        ("status_slot_saved", "Ranura {slot} guardada: {path}"),
        ("status_slot_save_failed", "No se pudo guardar la ranura {slot}"),
        ("status_slot_loaded", "Ranura {slot} cargada: {path}"),
        ("status_slot_loaded_link_disabled", "Ranura {slot} cargada: {path}; Game Link desconectado para proteger el estado"),
        ("status_slot_load_failed", "Ranura {slot} incompatible o corrupta"),
        ("status_slot_deleted", "Ranura {slot} borrada: {path}"),
        ("status_slot_delete_failed", "No se pudo borrar la ranura {slot}"),
        ("status_invalid_slot", "Ranura de estado inválida"),
        ("dialog_overwrite_state_title", "Sobrescribir estado"),
        ("dialog_overwrite_state_description", "La ranura ya está ocupada. ¿Sobrescribirla?"),
        ("dialog_delete_state_title", "Borrar estado"),
        ("dialog_delete_state_description", "¿Borrar la ranura seleccionada?"),
        ("dialog_import_battery_title", "Importar partida de batería"),
        ("dialog_import_battery_description", "Se respaldará la partida actual y se reemplazará la SRAM residente. ¿Continuar?"),
        ("dialog_load_state_title", "Cargar Save State externo"),
        ("dialog_load_state_description", "El estado se cargará sin modificar el archivo seleccionado. ¿Continuar?"),
        ("dialog_save_copy_title", "Guardar copia"),
        ("dialog_save_copy_description", "¿Conservar una copia en la ranura seleccionada?"),
        ("dialog_overwrite_slot_title", "Sobrescribir ranura"),
        ("dialog_overwrite_slot_description", "La ranura está ocupada. ¿Respaldar y sobrescribirla?"),
        ("status_file_unrecognized", "Archivo no reconocido: {error}"),
        ("status_sram_incompatible", "SRAM incompatible: {error}"),
        ("status_import_failed", "No se pudo importar la partida: {error}"),
        ("status_battery_imported", "Partida de batería importada: {canonical} (fuente conservada: {source})"),
        ("status_external_state_incompatible", "Save State incompatible o corrupto: {error}"),
        ("status_external_state_loaded", "Save State externo cargado: {source}"),
        ("status_external_state_imported", "Save State cargado e importado en ranura {slot}"),
        ("status_serial_clock_applied", "Reloj serial externo aplicado"),
        ("status_serial_clock_waiting", "No hay transferencia serial externa pendiente"),
        ("status_link_enabled", "Game Link activado (solo serial)"),
        ("status_link_disabled", "Game Link desactivado"),
        ("status_shell", "Carcasa: {name}"),
        ("status_console_paused", "Consola {console} {state}"),
        ("status_second_console_enabled", "Consola 2 activada"),
        ("status_second_console_disabled", "Consola 2 desactivada"),
        ("status_scale_integer", "Escala entera {scale}×"),
        ("status_scale_auto", "Escala automática entera"),
        ("status_scale_manual", "Escala manual"),
        ("diagnostics_summary", "C1 {c1} · {speed}% · frame {avg_frame}/{max_frame} ms · audio {audio_rate} Hz {queue}/{max_queue} ms · underruns {underruns} drops {drops} · C2 {c2}"),
        ("diagnostics_console_on", "ACTIVA"),
        ("diagnostics_console_hidden", "oculta"),
        ("diagnostics_console_empty", "sin cartucho"),
    ]));
    catalog
}

fn embedded_english() -> HashMap<String, String> {
    let mut catalog = embedded_catalog(&[
    ("render_engine", "Rendering engine"), ("render_opengl", "OpenGL"), ("render_software", "Software"), ("render_restart_required", "The change will apply after restart"), ("emulation_speed", "Emulation speed"), ("emulation_speed_native", "Native"), ("emulation_speed_default_boost", "Default boost"), ("emulation_speed_boost_tooltip", "Toggle emulation boost"),
    ("render_engine", "Rendering engine"), ("render_opengl", "OpenGL"), ("render_software", "Software"), ("render_restart_required", "The change will apply after restart"), ("emulation_speed", "Emulation speed"), ("emulation_speed_default_boost", "Default boost"), ("emulation_speed_boost_tooltip", "Toggle emulation boost"),
    ("app_title", "FoxMulator Next"), ("app_load_rom", "Load a ROM from Options"), ("app_no_cartridge", "No cartridge"), ("app_running", "Running"), ("app_paused", "Paused"), ("menu_settings", "Options"), ("menu_close", "Close"), ("menu_language", "LANGUAGE"), ("menu_reload_languages", "Reload languages"), ("section_game_cartridges", "GAME AND CARTRIDGES"), ("section_save_states", "SAVE STATES"), ("section_screen", "SCREEN"), ("section_appearance", "APPEARANCE"), ("section_sound", "SOUND"), ("section_game_link", "GAME LINK"), ("section_controls", "CONTROLS"), ("section_advanced", "ADVANCED"), ("quick_save", "Quick Save"), ("quick_load", "Quick Load"), ("save_state", "Save"), ("load_state", "Load"), ("delete_state", "Delete"), ("load_state_file", "Load from file..."), ("slot_empty", "Empty"), ("slot_occupied", "Occupied"), ("slot_0", "Slot 0 · Quick Save"), ("slot_1", "Slot 1"), ("slot_2", "Slot 2"), ("slot_3", "Slot 3"), ("slot_4", "Slot 4"), ("slot_5", "Slot 5"), ("slot_6", "Slot 6"), ("slot_7", "Slot 7"), ("slot_8", "Slot 8"), ("slot_9", "Slot 9"), ("slot_10", "Slot 10"), ("state_saved", "State saved to {path}"), ("state_loaded", "State loaded from {path}"), ("console_one", "Console 1"), ("console_two", "Console 2"), ("console_load", "Load"), ("console_reset", "Reset"), ("console_eject", "Eject ROM"), ("console_pause", "Pause"), ("screen_show_controls", "Show on-screen controls"), ("screen_show_second_console", "Show second console"), ("screen_show_grid", "Show grid (3× scale and above only)"), ("screen_auto_scale", "Automatic integer scale"), ("screen_scale", "Scale:"), ("appearance_change_shell", "Change shell"), ("appearance_library", "Library"), ("appearance_translucent", "Translucent finish"), ("display_tint_intensity", "Tint intensity: {value}%"), ("appearance_themes", "ORIGINAL THEMES"), ("appearance_custom_color", "CUSTOM COLOR (HEX)"), ("appearance_shell", "Shell"), ("appearance_accents", "Accents"), ("sound_general", "General"), ("sound_resume", "Unmute"), ("sound_mute", "Mute"), ("sound_description", "Gains are applied to each console PCM before mixing, without affecting emulation."), ("link_connected", "Link Cable connected"), ("link_disconnected", "Link Cable disconnected"), ("link_manual_clock", "Manual serial clock (diagnostic)"), ("controls_keys", "Keys"), ("controls_quick_save", "Quick Save"), ("controls_quick_load", "Quick Load"), ("controls_instructions", "Choose a control and press the new key. Escape cancels; Backspace and Enter can also be assigned."), ("controls_restore", "Restore Console 1 default controls"), ("advanced_diagnostics", "Performance diagnostics"), ("library_title", "LIBRARY · choose C1 or C2"), ("library_search", "Search games..."), ("library_scan", "Scan"), ("library_other_folder", "Other folder"), ("library_games", "games"), ("library_of", "of"), ("library_empty", "No games in library"), ("library_no_results", "No games found · clear the search"), ("library_internal", "Internal: {value}"), ("library_edit", "Edit game"), ("library_name", "Commercial name"), ("library_search_name_cover", "Search name and cover"), ("library_change_cover", "Change cover"), ("library_automatic", "Automatic"), ("library_results", "Catalog results"), ("library_save", "Save"), ("library_cancel", "Cancel"), ("library_scan_searching", "Searching files…"), ("library_scan_reading_file", "Reading: {name}"), ("library_scan_inspecting_zip", "Inspecting ZIP: {name}"), ("library_scan_file_counter", "File {current} of {total} · {percent}%"), ("library_scan_completed", "Scan complete: {games} games · {omitted} omitted · {errors} errors"), ("library_scan_cancel", "Cancel"), ("library_scan_cancelled", "Scan cancelled; previous library preserved"), ("library_scan_error", "Library scan failed: {error}"), ("library_favorite", "Favorite"), ("library_cover_invalid", "The image is not compatible"), ("library_cover_ready", "Local cover ready; save to confirm"), ("status_loaded", "ROM loaded: {path}"), ("status_unable_read_rom", "Unable to read ROM"), ("status_no_cartridge", "No cartridge available"), ("status_reset", "Console {console} reset"), ("status_ejected", "ROM ejected"), ("status_saved", "State saved"), ("status_loaded_state", "State loaded"), ("status_unable_state", "Unable to save/load state"), ("status_library_unavailable", "Library folder unavailable: {path}"), ("status_library_folder", "Library: {path}"), ("status_controls_saved", "Controls saved"), ("status_invalid_key", "Invalid key; assignment preserved"), ("status_defaults_restored", "Console 1 default controls restored"), ("status_select_folder", "Choose a folder with Other folder"), ("status_cover_saved", "Game record saved"), ("status_downloading_cover", "Downloading selected cover…"), ("status_language_reloaded", "Languages reloaded"), ("status_language_invalid", "Language unavailable; Spanish was selected"), ("manual_slots", "MANUAL SLOTS"), ("tint_intensity_prefix", "Tint intensity:"), ("theme_atomic", "Atomic"), ("theme_grape", "Grape"), ("theme_ice", "Ice"), ("theme_ocean", "Ocean"), ("theme_cherry", "Cherry"), ("theme_turquoise", "Turquoise"), ("theme_solar", "Solar"), ("theme_smoke", "Smoke"), ("theme_crystal", "Crystal"), ("theme_graphite", "Graphite"), ("theme_lime", "Lime"), ("status_capture_cancelled", "Capture cancelled"), ("status_state_wrong_rom", "The Save State belongs to another ROM"),
    ]);
    catalog.extend(embedded_catalog(&[
        ("app_stopped", "Stopped"),
        ("footer_tagline", "FOX // 01  •  Play wherever you want"),
        ("controls_binding_summary", "Keys: Left={left} Right={right} Up={up} Down={down} | A={a} B={b} Select={select} Start={start} | Quick Save={quick_save} Quick Load={quick_load} | Gamepad {gamepad}"),
        ("control_select", "Select"),
        ("control_start", "Start"),
        ("status_rebind_prompt", "Press a key for C{console} {action} (Escape cancels)"),
        ("status_rebind_conflict", "Key already assigned to C{console} {action}; choose another"),
        ("status_console_ejected_link", "Console {console} ejected; {battery}; Game Link waiting for cartridge"),
        ("status_console_ejected", "Console {console} cartridge ejected; {battery}"),
        ("status_battery_written", "battery save written: {path}"),
        ("status_no_battery", "no battery save"),
        ("status_operation_error", "Error: {error}"),
        ("status_library_search_prompt", "Type a name to search"),
        ("status_library_search_results", "Catalog results"),
        ("status_favorite_added", "Added to favorites"),
        ("status_favorite_removed", "Removed from favorites"),
        ("status_slot_saved", "Slot {slot} saved: {path}"),
        ("status_slot_save_failed", "Unable to save slot {slot}"),
        ("status_slot_loaded", "Slot {slot} loaded: {path}"),
        ("status_slot_loaded_link_disabled", "Slot {slot} loaded: {path}; Game Link disconnected to protect the state"),
        ("status_slot_load_failed", "Slot {slot} is incompatible or corrupt"),
        ("status_slot_deleted", "Slot {slot} deleted: {path}"),
        ("status_slot_delete_failed", "Unable to delete slot {slot}"),
        ("status_invalid_slot", "Invalid save-state slot"),
        ("dialog_overwrite_state_title", "Overwrite state"),
        ("dialog_overwrite_state_description", "The slot is already occupied. Overwrite it?"),
        ("dialog_delete_state_title", "Delete state"),
        ("dialog_delete_state_description", "Delete the selected slot?"),
        ("dialog_import_battery_title", "Import battery save"),
        ("dialog_import_battery_description", "The current save will be backed up and resident SRAM replaced. Continue?"),
        ("dialog_load_state_title", "Load external Save State"),
        ("dialog_load_state_description", "The state will load without modifying the selected file. Continue?"),
        ("dialog_save_copy_title", "Save copy"),
        ("dialog_save_copy_description", "Keep a copy in the selected slot?"),
        ("dialog_overwrite_slot_title", "Overwrite slot"),
        ("dialog_overwrite_slot_description", "The slot is occupied. Back up and overwrite it?"),
        ("status_file_unrecognized", "Unrecognized file: {error}"),
        ("status_sram_incompatible", "Incompatible SRAM: {error}"),
        ("status_import_failed", "Unable to import save: {error}"),
        ("status_battery_imported", "Battery save imported: {canonical} (source preserved: {source})"),
        ("status_external_state_incompatible", "External Save State is incompatible or corrupt: {error}"),
        ("status_external_state_loaded", "External Save State loaded: {source}"),
        ("status_external_state_imported", "Save State loaded and imported into slot {slot}"),
        ("status_serial_clock_applied", "External serial clock applied"),
        ("status_serial_clock_waiting", "No external serial transfer waiting"),
        ("status_link_enabled", "Game Link enabled (serial only)"),
        ("status_link_disabled", "Game Link disabled"),
        ("status_shell", "Shell: {name}"),
        ("status_console_paused", "Console {console} {state}"),
        ("status_second_console_enabled", "Console 2 enabled"),
        ("status_second_console_disabled", "Console 2 disabled"),
        ("status_scale_integer", "Integer scale {scale}×"),
        ("status_scale_auto", "Automatic integer scale"),
        ("status_scale_manual", "Manual scale"),
        ("diagnostics_summary", "C1 {c1} · {speed}% · frame {avg_frame}/{max_frame} ms · audio {audio_rate} Hz {queue}/{max_queue} ms · underruns {underruns} drops {drops} · C2 {c2}"),
        ("diagnostics_console_on", "ON"),
        ("diagnostics_console_hidden", "hidden"),
        ("diagnostics_console_empty", "no cartridge"),
    ]));
    catalog
}

fn embedded_catalog(entries: &[(&str, &str)]) -> HashMap<String, String> {
    entries
        .iter()
        .map(|(key, value)| ((*key).into(), (*value).into()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "foxmulator-i18n-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn embedded_languages_work_without_external_files() {
        let dir = temp_dir();
        let mut manager = LanguageManager::from_directory(dir.clone());
        assert_eq!(manager.text("menu_settings"), "Opciones");
        assert_eq!(manager.text("slot_0"), "Ranura 0 · Guardado rápido");
        for slot in 1..=10 {
            assert_eq!(
                manager.text(&format!("slot_{slot}")),
                format!("Ranura {slot}")
            );
        }
        assert_eq!(
            manager.format("library_internal", &[("value", "PM_CRYSTAL")]),
            "Interno: PM_CRYSTAL"
        );
        assert!(manager
            .languages()
            .iter()
            .any(|x| x.name == "English (United States)"));
        manager.select_id(ENGLISH_ID);
        assert_eq!(manager.text("slot_0"), "Slot 0 · Quick Save");
        for slot in 1..=10 {
            assert_eq!(
                manager.text(&format!("slot_{slot}")),
                format!("Slot {slot}")
            );
        }
        assert_eq!(
            manager.format("library_internal", &[("value", "PM_CRYSTAL")]),
            "Internal: PM_CRYSTAL"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn embedded_catalogs_have_exactly_the_same_complete_key_set() {
        let expected = KEYS
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(
            expected.len(),
            KEYS.len(),
            "duplicate required translation key"
        );
        let spanish = embedded_spanish();
        let english = embedded_english();
        assert_eq!(spanish.len(), KEYS.len());
        assert_eq!(english.len(), KEYS.len());
        assert_eq!(
            spanish
                .keys()
                .map(String::as_str)
                .collect::<std::collections::HashSet<_>>(),
            expected
        );
        assert_eq!(
            english
                .keys()
                .map(String::as_str)
                .collect::<std::collections::HashSet<_>>(),
            expected
        );
        for key in KEYS {
            assert!(valid_key(key), "translation key is not snake_case: {key}");
            assert_eq!(
                parameter_names(spanish.get(*key).unwrap()),
                parameter_names(english.get(*key).unwrap()),
                "parameter mismatch for {key}"
            );
        }
    }

    #[test]
    fn confirmed_untranslated_ui_literals_are_not_hardcoded_in_slint() {
        let ui = include_str!("Ui.slint");
        let rust = include_str!("main.rs");
        for stale in [
            "FOX Mulator · Opciones",
            "C1 · Cargar",
            "C1 · Reiniciar",
            "C1 · Expulsar ROM",
            "C1 · Guardar",
            "C1 · Pausa",
            "Ranura 0 · Quick Save",
            "RANURAS MANUALES",
        ] {
            assert!(!ui.contains(stale), "stale UI literal remains: {stale}");
        }
        assert!(!ui.contains("translations["));
        assert!(!rust.contains("translations["));
        assert!(ui.contains("root.slot_1"));
        assert!(ui.contains("root.display_tint_intensity"));
        assert!(ui.contains("root.library-internals[idx]"));
        for slot in 0..=10 {
            assert!(KEYS.contains(&format!("slot_{slot}").as_str()));
        }
        assert!(KEYS.contains(&"display_tint_intensity"));
    }

    #[test]
    fn repeated_language_changes_keep_catalog_access_non_reentrant() {
        let dir = temp_dir();
        let manager = std::rc::Rc::new(std::cell::RefCell::new(LanguageManager::from_directory(
            dir.clone(),
        )));
        for id in [ENGLISH_ID, SPANISH_ID, ENGLISH_ID, SPANISH_ID] {
            {
                let mut current = manager.borrow_mut();
                assert!(current.select_id(id));
            }
            let current = manager.borrow();
            assert!(!current.text("menu_settings").is_empty());
        }
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn dynamic_statuses_follow_language_changes_and_replace_parameters() {
        let dir = temp_dir();
        let mut manager = LanguageManager::from_directory(dir.clone());
        let params = [("slot", "10"), ("path", "/tmp/state.state")];
        assert_eq!(
            manager.format("status_slot_saved", &params),
            "Ranura 10 guardada: /tmp/state.state"
        );
        assert!(manager.select_id(ENGLISH_ID));
        assert_eq!(
            manager.format("status_slot_saved", &params),
            "Slot 10 saved: /tmp/state.state"
        );
        let diagnostics = manager.format(
            "diagnostics_summary",
            &[
                ("c1", "ACTIVA"),
                ("speed", "99.9"),
                ("avg_frame", "1.20"),
                ("max_frame", "2.40"),
                ("audio_rate", "44100"),
                ("queue", "2.0"),
                ("max_queue", "3.0"),
                ("underruns", "0"),
                ("drops", "1"),
                ("c2", "sin cartucho"),
            ],
        );
        assert!(diagnostics.contains("99.9%"));
        assert!(diagnostics.contains("1.20/2.40 ms"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn template_is_created_and_never_overwrites_user_content() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).unwrap();
        write_template_if_missing(&dir).unwrap();
        let path = dir.join("EN-US.xml");
        let original = fs::read_to_string(&path).unwrap();
        fs::write(&path, "custom").unwrap();
        write_template_if_missing(&dir).unwrap();
        assert_eq!(fs::read_to_string(path).unwrap(), "custom");
        assert!(original.contains("LanguageID"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn external_custom_and_regional_catalogs_validate_and_fallback() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("wea-CL.xml"), r#"<Language><LanguageID>wea-CL</LanguageID><LanguageName>Español weón de Chile</LanguageName><String id="menu_settings">Configuración po</String></Language>"#).unwrap();
        fs::write(dir.join("en-GB.xml"), r#"<Language><LanguageID>en-GB</LanguageID><LanguageName>English (United Kingdom)</LanguageName><String id="menu_settings">Settings</String></Language>"#).unwrap();
        let mut manager = LanguageManager::from_directory(dir.clone());
        assert!(manager
            .languages()
            .iter()
            .any(|x| x.name == "Español weón de Chile"));
        let index = manager
            .languages()
            .iter()
            .position(|x| x.id == "EN-GB")
            .unwrap();
        manager.select_index(index as i32);
        assert_eq!(manager.text("menu_settings"), "Settings");
        assert_eq!(manager.text("save_state"), "Save");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn english_template_is_ignored_as_an_external_override() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("EN-US.xml"), r#"<Language><LanguageID>EN-US</LanguageID><LanguageName>Edited</LanguageName><String id="menu_settings">Wrong</String></Language>"#).unwrap();
        let manager = LanguageManager::from_directory(dir.clone());
        assert_eq!(manager.text("menu_settings"), "Opciones");
        let mut manager = manager;
        manager.select_id("EN-US");
        assert_eq!(manager.text("menu_settings"), "Options");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn malformed_and_duplicate_catalogs_are_ignored() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("bad.xml"), "<Language>").unwrap();
        fs::write(dir.join("dup.xml"), r#"<Language><LanguageID>dup</LanguageID><LanguageName>Dup</LanguageName><String id="x">1</String><String id="x">2</String></Language>"#).unwrap();
        let manager = LanguageManager::from_directory(dir.clone());
        assert!(!manager
            .languages()
            .iter()
            .any(|x| x.id == "BAD" || x.id == "DUP"));
        assert!(!manager.diagnostics().is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn parameters_are_replaced_without_interpreting_catalog_content() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("x.xml"), r#"<Language><LanguageID>x</LanguageID><LanguageName>X</LanguageName><String id="state.slot">Custom {number}</String></Language>"#).unwrap();
        let mut manager = LanguageManager::from_directory(dir.clone());
        manager.select_index(
            manager
                .languages()
                .iter()
                .position(|x| x.id == "X")
                .unwrap() as i32,
        );
        assert_eq!(manager.format("slot_3", &[("number", "3")]), "Custom 3");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn xml_order_does_not_change_semantic_resolution() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).unwrap();
        let first = r#"<Language><LanguageID>x</LanguageID><LanguageName>X</LanguageName><String id="slot_1">ONE</String><String id="display_tint_intensity">TINT {value}%</String><String id="slot_2">TWO</String></Language>"#;
        let second = r#"<Language><LanguageID>x</LanguageID><LanguageName>X</LanguageName><String id="slot_2">TWO</String><String id="display_tint_intensity">TINT {value}%</String><String id="slot_1">ONE</String></Language>"#;
        fs::write(dir.join("x.xml"), first).unwrap();
        let mut manager = LanguageManager::from_directory(dir.clone());
        manager.select_id("X");
        let before = (
            manager.text("slot_1"),
            manager.text("slot_2"),
            manager.format("display_tint_intensity", &[("value", "60")]),
        );
        fs::write(dir.join("x.xml"), second).unwrap();
        manager.reload();
        manager.select_id("X");
        assert_eq!(
            before,
            (
                manager.text("slot_1"),
                manager.text("slot_2"),
                manager.format("display_tint_intensity", &[("value", "60")])
            )
        );
        let _ = fs::remove_dir_all(dir);
    }
}
