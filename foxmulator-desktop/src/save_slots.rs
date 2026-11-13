//! Persistent battery saves and the eleven per-ROM save-state slots.
//!
//! `.sav` is cartridge battery data (including the MBC3 RTC envelope when the
//! core needs it). `.state` is a complete emulator snapshot. They deliberately
//! have separate names, validators, and migration paths.

#![cfg_attr(not(feature = "gui"), allow(dead_code))]

use foxmulator_core::System;
use serde::{Deserialize, Serialize};
use std::io::{Error, ErrorKind, Result};
use std::path::{Path, PathBuf};

pub const SLOT_COUNT: usize = 11;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SlotMetadata {
    pub slot: usize,
    pub rom_identity: String,
    pub rom_title: String,
    pub saved_at: String,
    pub size: u64,
    pub format: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BatteryMigration {
    None,
    Migrated { source: PathBuf, backup: PathBuf },
    Conflict { files: Vec<PathBuf> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalSaveKind {
    Battery,
    State,
}

/// Serialize battery writes across FoxMulator processes. A second process gets
/// an explicit error instead of silently replacing the first one's SRAM/RTC.
pub fn save_battery(mmu: &mut foxmulator_core::mmu::Mmu, path: &Path) -> Result<()> {
    if !mmu.has_battery() {
        return Ok(());
    }
    let lock = path.with_extension("sav.lock");
    let _guard = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&lock)
        .map_err(|error| {
            Error::new(
                ErrorKind::WouldBlock,
                format!("battery save is locked: {error}"),
            )
        })?;
    let result = mmu.save_ram(path);
    let _ = std::fs::remove_file(lock);
    result
}

fn stem(path: &Path) -> String {
    sanitize_filename_component(
        path.file_stem()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty())
            .unwrap_or("game"),
    )
}

/// Convert a ROM-derived component into a portable Windows filename component.
/// This only affects physical filenames; the logical ROM identity is unchanged.
pub fn sanitize_filename_component(input: &str) -> String {
    let mut output = input
        .chars()
        .map(|character| match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            other if other.is_control() => '_',
            other => other,
        })
        .collect::<String>();
    while matches!(output.chars().last(), Some(' ' | '.')) {
        output.pop();
    }
    if output.is_empty() {
        output.push_str("game");
    }
    let reserved = matches!(
        output.to_ascii_uppercase().as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    );
    if reserved {
        output.insert(0, '_');
    }
    output
}

pub fn canonical_battery_path(rom_path: &Path, identity: &str) -> PathBuf {
    rom_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!(
            "{}.{}.sav",
            stem(rom_path),
            sanitize_filename_component(identity)
        ))
}

pub fn legacy_battery_path(rom_path: &Path, identity: &str) -> PathBuf {
    rom_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(format!("{}.{}.sav", stem(rom_path), identity))
}

pub fn slot_path_from_battery(battery_path: &Path, slot: usize) -> Result<PathBuf> {
    if slot >= SLOT_COUNT {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "invalid save-state slot",
        ));
    }
    Ok(battery_path.with_extension(format!("slot{slot}.state")))
}

pub fn identity_from_battery(battery_path: &Path) -> Option<String> {
    let mut identity = battery_path
        .file_stem()?
        .to_str()?
        .rsplit('.')
        .next()
        .map(str::to_owned)?;
    if identity.starts_with("instance") {
        identity = battery_path
            .file_stem()?
            .to_str()?
            .rsplit('.')
            .nth(1)?
            .to_owned();
    }
    if let Some((system, checksum)) = identity.split_once('_') {
        if (system == "cgb" || system == "dmg") && checksum.len() == 8 {
            return Some(format!("{system}:{checksum}"));
        }
    }
    Some(identity)
}

pub fn resolved_battery_path(rom_path: &Path, identity: &str) -> PathBuf {
    let canonical = canonical_battery_path(rom_path, identity);
    if canonical.exists() {
        canonical
    } else {
        legacy_battery_path(rom_path, identity)
    }
}

pub fn resolved_slot_path(battery_path: &Path, slot: usize) -> Result<PathBuf> {
    let canonical = slot_path_from_battery(battery_path, slot)?;
    if canonical.exists() {
        return Ok(canonical);
    }
    let legacy_battery = battery_path
        .file_stem()
        .and_then(|value| value.to_str())
        .map(|stem| {
            if let Some((base, identity)) = stem.rsplit_once('.') {
                let logical = identity.replace('_', ":");
                battery_path
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join(format!("{base}.{logical}.slot{slot}.state"))
            } else {
                canonical.clone()
            }
        })
        .unwrap_or(canonical.clone());
    if legacy_battery.exists() {
        Ok(legacy_battery)
    } else {
        Ok(canonical)
    }
}

fn metadata_path(state: &Path) -> PathBuf {
    state.with_extension("json")
}

fn atomic_copy(source: &Path, destination: &Path) -> Result<()> {
    let temp = destination.with_extension("migration.tmp");
    std::fs::copy(source, &temp)?;
    std::fs::rename(temp, destination)
}

pub fn identify_external_save(path: &Path) -> Result<ExternalSaveKind> {
    let data = std::fs::read(path)?;
    if data.starts_with(b"FOXSTATE1\n") {
        return Ok(ExternalSaveKind::State);
    }
    if data.starts_with(b"FOXRTC1\0") || !data.is_empty() {
        return Ok(ExternalSaveKind::Battery);
    }
    Err(Error::new(
        ErrorKind::InvalidData,
        "empty or unknown save file",
    ))
}

/// Validate an external battery file and return a candidate MMU. The caller
/// can install this candidate instead of allowing the old resident SRAM to be
/// written back over the imported data.
pub fn validate_external_battery(
    mmu: &foxmulator_core::mmu::Mmu,
    source: &Path,
) -> Result<foxmulator_core::mmu::Mmu> {
    if !mmu.has_battery() {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "ROM has no battery-backed SRAM",
        ));
    }
    let data = std::fs::read(source)?;
    let mut candidate = mmu.clone();
    if !data_is_rtc(&data) && data.len() != candidate.export_ram().len() {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "SRAM size does not match the cartridge",
        ));
    }
    if !candidate.load_ram(source)? {
        return Err(Error::new(ErrorKind::InvalidData, "battery save is empty"));
    }
    Ok(candidate)
}

/// Back up the current canonical battery and atomically install the selected
/// external file. The source is never modified.
pub fn install_external_battery(source: &Path, canonical: &Path) -> Result<Option<PathBuf>> {
    let backup = if canonical.is_file() {
        let backup = canonical.with_extension("sav.before-import.bak");
        if !backup.exists() {
            atomic_copy(canonical, &backup)?;
        }
        Some(backup)
    } else {
        None
    };
    atomic_copy(source, canonical)?;
    Ok(backup)
}

/// Migrate only validated battery files. Existing source files are never
/// removed. A conflict is returned when distinct legacy saves exist.
pub fn migrate_battery(
    mmu: &mut foxmulator_core::mmu::Mmu,
    rom_path: &Path,
    canonical: &Path,
) -> Result<BatteryMigration> {
    if !mmu.has_battery() {
        return Ok(BatteryMigration::None);
    }
    if canonical.is_file() {
        return Ok(BatteryMigration::None);
    }
    let stem = stem(rom_path);
    let candidates = [
        rom_path.with_extension("sav"),
        rom_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!("{stem}.console1.sav")),
        rom_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!("{stem}.console2.sav")),
    ];
    let mut valid = Vec::new();
    for candidate in candidates {
        if !candidate.is_file() {
            continue;
        }
        let bytes = std::fs::read(&candidate)?;
        let mut probe = mmu.clone();
        let raw_size = probe.export_ram().len();
        let valid_shape = data_is_rtc(&bytes) || bytes.len() == raw_size;
        if valid_shape && probe.load_ram(&candidate).unwrap_or(false) {
            valid.push((candidate, bytes));
        }
    }
    valid.dedup_by(|left, right| left.1 == right.1);
    if valid.len() > 1 {
        return Ok(BatteryMigration::Conflict {
            files: valid.into_iter().map(|item| item.0).collect(),
        });
    }
    let Some((source, _)) = valid.into_iter().next() else {
        return Ok(BatteryMigration::None);
    };
    let backup = source.with_extension("sav.legacy.bak");
    if !backup.exists() {
        atomic_copy(&source, &backup)?;
    }
    atomic_copy(&source, canonical)?;
    Ok(BatteryMigration::Migrated { source, backup })
}

fn data_is_rtc(data: &[u8]) -> bool {
    data.starts_with(b"FOXRTC1\0")
}

/// Import an unambiguous legacy quick-state into slot 0 without deleting the
/// original. Invalid or cross-ROM files remain untouched for explicit recovery.
pub fn migrate_legacy_state(
    system: &System,
    battery_path: &Path,
    identity: &str,
    title: &str,
) -> Result<bool> {
    let canonical = slot_path_from_battery(battery_path, 0)?;
    if canonical.exists() {
        return Ok(false);
    }
    let legacy_stem = battery_path
        .file_stem()
        .and_then(|value| value.to_str())
        .and_then(|value| value.rsplit_once('.').map(|(stem, _)| stem.to_owned()))
        .unwrap_or_else(|| "game".into());
    let parent = battery_path.parent().unwrap_or_else(|| Path::new("."));
    let candidates = [
        parent.join(format!("{legacy_stem}.state")),
        parent.join(format!("{legacy_stem}.right.state")),
        parent.join(format!("{legacy_stem}.console1.state")),
        parent.join(format!("{legacy_stem}.console2.state")),
    ];
    let mut valid = Vec::new();
    for candidate in candidates {
        if !candidate.is_file() {
            continue;
        }
        let mut probe = system.clone();
        if probe.load_state(&candidate).is_ok() && probe.rom_checksum() == system.rom_checksum() {
            valid.push(candidate);
        }
    }
    if valid.len() != 1 {
        return Ok(false);
    }
    let source = &valid[0];
    atomic_copy(source, &canonical)?;
    let metadata = SlotMetadata {
        slot: 0,
        rom_identity: identity.to_owned(),
        rom_title: title.to_owned(),
        saved_at: chrono_like_now(),
        size: std::fs::metadata(&canonical)?.len(),
        format: "FOXSTATE1 (migrated)".into(),
    };
    let meta = metadata_path(&canonical);
    let temp = meta.with_extension("json.tmp");
    std::fs::write(
        &temp,
        serde_json::to_vec_pretty(&metadata)
            .map_err(|error| Error::new(ErrorKind::InvalidData, error.to_string()))?,
    )?;
    std::fs::rename(temp, meta)?;
    Ok(true)
}

pub fn save_slot_at(
    system: &System,
    battery_path: &Path,
    identity: &str,
    title: &str,
    slot: usize,
) -> Result<SlotMetadata> {
    let path = slot_path_from_battery(battery_path, slot)?;
    system.save_state(&path)?;
    let metadata = SlotMetadata {
        slot,
        rom_identity: identity.to_owned(),
        rom_title: title.to_owned(),
        saved_at: chrono_like_now(),
        size: std::fs::metadata(&path)?.len(),
        format: "FOXSTATE1".into(),
    };
    let meta = metadata_path(&path);
    let temp = meta.with_extension("json.tmp");
    std::fs::write(
        &temp,
        serde_json::to_vec_pretty(&metadata)
            .map_err(|error| Error::new(ErrorKind::InvalidData, error.to_string()))?,
    )?;
    std::fs::rename(temp, meta)?;
    Ok(metadata)
}

pub fn load_slot_at(
    system: &mut System,
    battery_path: &Path,
    identity: &str,
    slot: usize,
) -> Result<SlotMetadata> {
    let path = resolved_slot_path(battery_path, slot)?;
    let metadata: SlotMetadata = serde_json::from_slice(&std::fs::read(metadata_path(&path))?)
        .map_err(|error| Error::new(ErrorKind::InvalidData, error.to_string()))?;
    if metadata.rom_identity != identity || metadata.slot != slot {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "save-state ROM identity mismatch",
        ));
    }
    let mut restored = system.clone();
    restored.load_state(&path)?;
    if restored.rom_checksum() != system.rom_checksum() {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "save-state ROM mismatch",
        ));
    }
    *system = restored;
    Ok(metadata)
}

pub fn delete_slot_at(battery_path: &Path, slot: usize) -> Result<()> {
    let path = resolved_slot_path(battery_path, slot)?;
    if path.exists() {
        std::fs::remove_file(&path)?;
    }
    let meta = metadata_path(&path);
    if meta.exists() {
        std::fs::remove_file(meta)?;
    }
    Ok(())
}

pub fn inspect_slot_at(battery_path: &Path, identity: &str, slot: usize) -> Option<SlotMetadata> {
    let path = resolved_slot_path(battery_path, slot).ok()?;
    let metadata: SlotMetadata =
        serde_json::from_slice(&std::fs::read(metadata_path(&path)).ok()?).ok()?;
    (metadata.slot == slot && metadata.rom_identity == identity && path.is_file())
        .then_some(metadata)
}

fn chrono_like_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let days = seconds / 86_400;
    let day_seconds = seconds % 86_400;
    let z = days as i64 + 719_468;
    let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    let year = year + if month <= 2 { 1 } else { 0 };
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC",
        day_seconds / 3_600,
        day_seconds / 60 % 60,
        day_seconds % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_exactly_eleven_slots_and_identity_is_stable() {
        assert_eq!(SLOT_COUNT, 11);
        let path = Path::new("sample.1234ABCD.sav");
        assert_eq!(identity_from_battery(path).as_deref(), Some("1234ABCD"));
        assert_eq!(
            slot_path_from_battery(path, 0).expect("valid slot"),
            PathBuf::from("sample.1234ABCD.slot0.state")
        );
    }

    #[test]
    fn generated_components_are_windows_safe_without_changing_logical_identity() {
        assert_eq!(sanitize_filename_component("cgb:6BDF3C3F"), "cgb_6BDF3C3F");
        assert_eq!(
            sanitize_filename_component("bad<>:\"/\\|?*"),
            "bad_________"
        );
        assert_eq!(sanitize_filename_component("CON"), "_CON");
        assert_eq!(sanitize_filename_component("name. "), "name");
        let path = canonical_battery_path(Path::new("sample.gb"), "cgb:6BDF3C3F");
        assert_eq!(path, PathBuf::from("sample.cgb_6BDF3C3F.sav"));
    }

    #[test]
    fn legacy_colon_state_is_a_read_fallback() {
        let root = std::env::temp_dir().join(format!("foxmulator-legacy-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&root);
        let canonical = root.join("game.cgb_6BDF3C3F.sav");
        let legacy = root.join("game.cgb:6BDF3C3F.slot0.state");
        std::fs::write(&legacy, b"legacy").expect("legacy state");
        assert_eq!(
            resolved_slot_path(&canonical, 0).expect("valid slot"),
            legacy
        );
        assert!(canonical
            .file_name()
            .unwrap()
            .to_string_lossy()
            .contains('_'));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn saves_loads_and_deletes_a_real_slot_without_partial_restore() {
        let root = std::env::temp_dir().join(format!("foxmulator-slots-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&root);
        let battery = root.join("game.ABCD1234.sav");
        let rom = vec![0u8; 0x8000];
        let system = System::from_rom(rom.clone());
        let saved = save_slot_at(&system, &battery, "ABCD1234", "Game", 0).expect("save slot");
        assert_eq!(saved.slot, 0);
        let mut loaded = System::from_rom(rom);
        load_slot_at(&mut loaded, &battery, "ABCD1234", 0).expect("load slot");
        assert_eq!(loaded.rom_checksum(), system.rom_checksum());
        std::fs::write(
            slot_path_from_battery(&battery, 1).expect("valid slot"),
            b"broken",
        )
        .expect("corrupt file");
        assert!(load_slot_at(&mut loaded, &battery, "ABCD1234", 1).is_err());
        delete_slot_at(&battery, 0).expect("delete slot");
        assert!(inspect_slot_at(&battery, "ABCD1234", 0).is_none());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn invalid_slots_are_rejected_without_constructing_paths() {
        let battery = Path::new("game.ABCD1234.sav");
        assert!(slot_path_from_battery(battery, 10).is_ok());
        assert!(
            slot_path_from_battery(battery, 11)
                .expect_err("slot 11 must be rejected")
                .kind()
                == ErrorKind::InvalidInput
        );
        assert_eq!(
            slot_path_from_battery(battery, usize::MAX)
                .expect_err("large slot must be rejected")
                .kind(),
            ErrorKind::InvalidInput
        );
        assert!(resolved_slot_path(battery, 11).is_err());
        assert!(delete_slot_at(battery, 11).is_err());
        assert!(inspect_slot_at(battery, "ABCD1234", 11).is_none());
    }

    #[test]
    fn external_battery_is_validated_backed_up_and_source_is_preserved() {
        let root = std::env::temp_dir().join(format!("foxmulator-import-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&root);
        let mut rom = vec![0u8; 0x8000];
        rom[0x147] = 0x03; // MBC1 + RAM + battery
        rom[0x149] = 0x02; // 8 KiB RAM
        let system = System::from_rom(rom);
        let source = root.join("juego.console1.sav");
        let canonical = root.join("juego.ABCD1234.sav");
        let source_data = vec![0x5a; system.mmu.export_ram().len()];
        std::fs::write(&source, &source_data).expect("external SRAM");
        let candidate = validate_external_battery(&system.mmu, &source).expect("valid SRAM");
        assert_eq!(candidate.export_ram(), source_data);
        std::fs::write(&canonical, vec![0x11; source_data.len()]).expect("current SRAM");
        let backup = install_external_battery(&source, &canonical)
            .expect("install SRAM")
            .expect("backup created");
        assert_eq!(std::fs::read(&source).expect("source remains"), source_data);
        assert_eq!(std::fs::read(&canonical).expect("canonical"), source_data);
        assert_eq!(
            std::fs::read(backup).expect("backup"),
            vec![0x11; source_data.len()]
        );
        let _ = std::fs::remove_dir_all(root);
    }
}
