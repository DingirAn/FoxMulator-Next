//! Shared ROM origins for direct files and ZIP archives.

#![cfg_attr(not(feature = "gui"), allow(dead_code))]

use std::fs::File;
use std::io::{Error, ErrorKind, Read, Result};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use zip::ZipArchive;

pub const MAX_ARCHIVE_BYTES: u64 = 128 * 1024 * 1024;
pub const MAX_ROM_BYTES: usize = 32 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum RomSource {
    File(PathBuf),
    Zip { archive: PathBuf, entry: String },
}

/// Notifications emitted by the background library scanner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScanProgress {
    Searching,
    FilesDiscovered {
        total: usize,
    },
    Reading {
        current: usize,
        total: usize,
        name: String,
    },
    InspectingZip {
        current: usize,
        total: usize,
        name: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanReport {
    pub sources: Vec<RomSource>,
    pub processed: usize,
    pub omitted: usize,
    pub errors: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScanOutcome {
    Completed(ScanReport),
    Cancelled,
    Failed(String),
}

impl RomSource {
    pub fn from_path(path: PathBuf) -> Result<Self> {
        if !path.is_file() {
            return Err(Error::new(
                ErrorKind::NotFound,
                format!("ROM no existe o no es un archivo: {}", path.display()),
            ));
        }
        if is_zip_path(&path) {
            let source = discover_zip(&path)?;
            source.ok_or_else(|| {
                Error::new(
                    ErrorKind::InvalidData,
                    format!(
                        "El ZIP no contiene una ROM GB/GB Color válida: {}",
                        path.display()
                    ),
                )
            })
        } else {
            let extension = path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or_default();
            if !extension.eq_ignore_ascii_case("gb") && !extension.eq_ignore_ascii_case("gbc") {
                return Err(Error::new(
                    ErrorKind::InvalidInput,
                    "extensión de ROM no compatible",
                ));
            }
            if !valid_direct_rom(&path) {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "ROM GB/GB Color inválida",
                ));
            }
            Ok(Self::File(path))
        }
    }

    pub fn id(&self) -> String {
        match self {
            Self::File(path) => path.to_string_lossy().into_owned(),
            Self::Zip { archive, entry } => {
                format!("zip://{}#{}", archive.to_string_lossy(), entry)
            }
        }
    }

    pub fn physical_path(&self) -> &Path {
        match self {
            Self::File(path) => path,
            Self::Zip { archive, .. } => archive,
        }
    }

    pub fn display_name(&self) -> String {
        match self {
            Self::File(path) => path.display().to_string(),
            Self::Zip { archive, entry } => format!("{} [{}]", archive.display(), entry),
        }
    }

    pub fn load(&self) -> Result<Vec<u8>> {
        match self {
            Self::File(path) => {
                let metadata = std::fs::metadata(path)?;
                if metadata.len() > MAX_ROM_BYTES as u64 {
                    return Err(Error::new(ErrorKind::InvalidData, "ROM demasiado grande"));
                }
                std::fs::read(path)
            }
            Self::Zip { archive, entry } => read_zip_entry(archive, entry),
        }
    }

    pub fn from_id(id: &str) -> Self {
        let Some(value) = id.strip_prefix("zip://") else {
            return Self::File(PathBuf::from(id));
        };
        let Some((archive, entry)) = value.rsplit_once('#') else {
            return Self::File(PathBuf::from(id));
        };
        Self::Zip {
            archive: PathBuf::from(archive),
            entry: entry.to_owned(),
        }
    }
}

pub fn is_zip_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("zip"))
}

#[allow(dead_code)]
pub fn discover(root: &Path) -> Vec<RomSource> {
    let mut sources = std::fs::read_dir(root)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            let extension = path.extension()?.to_str()?;
            if extension.eq_ignore_ascii_case("gb") || extension.eq_ignore_ascii_case("gbc") {
                return valid_direct_rom(&path).then_some(RomSource::File(path));
            }
            if is_zip_path(&path) {
                return discover_zip(&path).ok().flatten();
            }
            None
        })
        .collect::<Vec<_>>();
    sources.sort_by_key(RomSource::id);
    sources
}

/// Scan one physical directory using the same direct/ZIP discovery rules as
/// `discover`, while allowing a UI worker to report progress and cancel safely.
/// Results are returned only after the complete scan, so callers can apply them
/// transactionally and retain the previous library on cancellation or failure.
pub fn scan_with_progress<F>(root: &Path, cancel: &AtomicBool, mut notify: F) -> ScanOutcome
where
    F: FnMut(ScanProgress),
{
    notify(ScanProgress::Searching);
    let mut files = Vec::new();
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) => return ScanOutcome::Failed(error.to_string()),
    };
    for entry in entries {
        if cancel.load(Ordering::Relaxed) {
            return ScanOutcome::Cancelled;
        }
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let path = entry.path();
        if path.is_file() && is_scan_candidate(&path) {
            files.push(path);
        }
    }
    files.sort_by_key(|path| path.to_string_lossy().to_ascii_lowercase());
    let total = files.len();
    notify(ScanProgress::FilesDiscovered { total });

    let mut sources = Vec::new();
    let mut omitted = 0;
    let mut errors = 0;
    let mut last_notification = Instant::now() - Duration::from_millis(100);
    for (offset, path) in files.into_iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return ScanOutcome::Cancelled;
        }
        let current = offset + 1;
        let name = path
            .file_name()
            .map(|value| value.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let is_zip = is_zip_path(&path);
        if last_notification.elapsed() >= Duration::from_millis(75) || current == 1 {
            notify(if is_zip {
                ScanProgress::InspectingZip {
                    current,
                    total,
                    name: name.clone(),
                }
            } else {
                ScanProgress::Reading {
                    current,
                    total,
                    name: name.clone(),
                }
            });
            last_notification = Instant::now();
        }
        match RomSource::from_path(path) {
            Ok(source) => sources.push(source),
            Err(error) => {
                omitted += 1;
                if is_zip_path(Path::new(&name)) || error.kind() != ErrorKind::InvalidData {
                    errors += 1;
                }
            }
        }
    }
    sources.sort_by_key(RomSource::id);
    ScanOutcome::Completed(ScanReport {
        sources,
        processed: total,
        omitted,
        errors,
    })
}

fn is_scan_candidate(path: &Path) -> bool {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    extension.eq_ignore_ascii_case("gb")
        || extension.eq_ignore_ascii_case("gbc")
        || extension.eq_ignore_ascii_case("zip")
}

fn valid_direct_rom(path: &Path) -> bool {
    std::fs::metadata(path)
        .map(|metadata| metadata.len() <= MAX_ROM_BYTES as u64)
        .unwrap_or(false)
        && std::fs::read(path)
            .map(|bytes| valid_rom_bytes(&bytes))
            .unwrap_or(false)
}

fn discover_zip(path: &Path) -> Result<Option<RomSource>> {
    let metadata = std::fs::metadata(path)?;
    if metadata.len() > MAX_ARCHIVE_BYTES {
        return Err(Error::new(ErrorKind::InvalidData, "ZIP demasiado grande"));
    }
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file).map_err(zip_error)?;
    let mut candidates = Vec::new();
    for index in 0..archive.len() {
        let entry = archive.by_index(index).map_err(zip_error)?;
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().replace('\\', "/");
        let Some(extension) = Path::new(&name)
            .extension()
            .and_then(|value| value.to_str())
        else {
            continue;
        };
        if !extension.eq_ignore_ascii_case("gb") && !extension.eq_ignore_ascii_case("gbc") {
            continue;
        }
        if entry.size() > MAX_ROM_BYTES as u64 {
            continue;
        }
        candidates.push(name);
    }
    candidates.sort_by_key(|name| name.to_ascii_lowercase());
    for entry in candidates {
        let source = RomSource::Zip {
            archive: path.to_owned(),
            entry,
        };
        if source
            .load()
            .map(|bytes| valid_rom_bytes(&bytes))
            .unwrap_or(false)
        {
            return Ok(Some(source));
        }
    }
    Ok(None)
}

fn read_zip_entry(archive_path: &Path, wanted: &str) -> Result<Vec<u8>> {
    let metadata = std::fs::metadata(archive_path)?;
    if metadata.len() > MAX_ARCHIVE_BYTES {
        return Err(Error::new(ErrorKind::InvalidData, "ZIP demasiado grande"));
    }
    let file = File::open(archive_path)?;
    let mut archive = ZipArchive::new(file).map_err(zip_error)?;
    let mut entry = archive.by_name(wanted).map_err(zip_error)?;
    if entry.is_dir() {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "La entrada ZIP está cifrada o no es un archivo",
        ));
    }
    if entry.size() > MAX_ROM_BYTES as u64 {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "ROM ZIP demasiado grande",
        ));
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut bytes)?;
    if !valid_rom_bytes(&bytes) {
        return Err(Error::new(ErrorKind::InvalidData, "ROM ZIP inválida"));
    }
    Ok(bytes)
}

fn valid_rom_bytes(bytes: &[u8]) -> bool {
    const NINTENDO_LOGO: [u8; 48] = [
        0xCE, 0xED, 0x66, 0x66, 0xCC, 0x0D, 0x00, 0x0B, 0x03, 0x73, 0x00, 0x83, 0x00, 0x0C, 0x00,
        0x0D, 0x00, 0x08, 0x11, 0x1F, 0x88, 0x89, 0x00, 0x0E, 0xDC, 0xCC, 0x6E, 0xE6, 0xDD, 0xDD,
        0xD9, 0x99, 0xBB, 0xBB, 0x67, 0x63, 0x6E, 0x0E, 0xEC, 0xCC, 0xDD, 0xDC, 0x99, 0x9F, 0xBB,
        0xB9, 0x33, 0x3E,
    ];
    bytes.len() >= 0x150 && bytes.get(0x104..0x134) == Some(NINTENDO_LOGO.as_slice())
}

fn zip_error(error: zip::result::ZipError) -> Error {
    Error::new(ErrorKind::InvalidData, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::sync::atomic::AtomicBool;
    fn test_directory(name: &str) -> PathBuf {
        let directory =
            std::env::temp_dir().join(format!("foxmulator-zip-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        directory
    }

    fn test_rom(fill: u8) -> Vec<u8> {
        let mut rom = vec![fill; 0x150];
        rom[0x104..0x134].copy_from_slice(&[
            0xCE, 0xED, 0x66, 0x66, 0xCC, 0x0D, 0x00, 0x0B, 0x03, 0x73, 0x00, 0x83, 0x00, 0x0C,
            0x00, 0x0D, 0x00, 0x08, 0x11, 0x1F, 0x88, 0x89, 0x00, 0x0E, 0xDC, 0xCC, 0x6E, 0xE6,
            0xDD, 0xDD, 0xD9, 0x99, 0xBB, 0xBB, 0x67, 0x63, 0x6E, 0x0E, 0xEC, 0xCC, 0xDD, 0xDC,
            0x99, 0x9F, 0xBB, 0xB9, 0x33, 0x3E,
        ]);
        rom
    }

    #[test]
    fn zip_rom_is_selected_from_nested_directory() {
        let directory = test_directory("nested");
        let path = directory.join("game.ZIP");
        let file = File::create(&path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("nested/game.GBC", zip::write::FileOptions::default())
            .unwrap();
        writer.write_all(&test_rom(0)).unwrap();
        writer.finish().unwrap();
        let source = RomSource::from_path(path.clone()).unwrap();
        assert_eq!(source.physical_path(), path);
        assert!(source.id().contains("nested/game.GBC"));
        assert_eq!(source.load().unwrap().len(), 0x150);
    }

    #[test]
    fn zip_selection_is_deterministic_and_ignores_non_roms() {
        let directory = test_directory("multi");
        let path = directory.join("multi.zip");
        let file = File::create(&path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("z.gb", zip::write::FileOptions::default())
            .unwrap();
        writer.write_all(&test_rom(1)).unwrap();
        writer
            .start_file("a.gbc", zip::write::FileOptions::default())
            .unwrap();
        writer.write_all(&test_rom(2)).unwrap();
        writer
            .start_file("readme.txt", zip::write::FileOptions::default())
            .unwrap();
        writer.write_all(b"not a rom").unwrap();
        writer.finish().unwrap();
        let source = RomSource::from_path(path).unwrap();
        assert!(source.id().contains("a.gbc"));
    }

    #[test]
    fn discovery_mixes_direct_and_zip_roms_and_ignores_zip_without_rom() {
        let directory = test_directory("discovery");
        let direct_path = directory.join("direct.GB");
        std::fs::write(&direct_path, test_rom(3)).unwrap();
        let zip_path = directory.join("archive.zip");
        let file = File::create(&zip_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("folder/inside.gbc", zip::write::FileOptions::default())
            .unwrap();
        writer.write_all(&test_rom(4)).unwrap();
        writer.finish().unwrap();
        let empty_zip_path = directory.join("empty.ZIP");
        let file = File::create(empty_zip_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        writer
            .start_file("notes.txt", zip::write::FileOptions::default())
            .unwrap();
        writer.write_all(b"not a cartridge").unwrap();
        writer.finish().unwrap();

        let sources = discover(&directory);
        assert_eq!(sources.len(), 2);
        assert!(sources
            .iter()
            .any(|source| matches!(source, RomSource::File(_))));
        assert!(sources
            .iter()
            .any(|source| matches!(source, RomSource::Zip { .. })));
    }

    #[test]
    fn malformed_zip_is_reported_without_panic() {
        let directory = test_directory("malformed");
        let path = directory.join("broken.zip");
        std::fs::write(&path, b"not a zip archive").unwrap();
        assert!(RomSource::from_path(path).is_err());
    }

    #[test]
    fn zip_size_limit_rejects_oversized_archives() {
        assert_eq!(MAX_ROM_BYTES, 32 * 1024 * 1024);
        assert_eq!(MAX_ARCHIVE_BYTES, 128 * 1024 * 1024);
    }

    #[test]
    fn progress_scan_reports_physical_files_and_keeps_zip_policy() {
        let directory = test_directory("progress");
        std::fs::write(directory.join("game.gb"), test_rom(5)).unwrap();
        std::fs::write(directory.join("broken.zip"), b"not a zip archive").unwrap();
        let mut events = Vec::new();
        let outcome = scan_with_progress(&directory, &AtomicBool::new(false), |event| {
            events.push(event);
        });
        let ScanOutcome::Completed(report) = outcome else {
            panic!("scan should complete");
        };
        assert_eq!(report.processed, 2);
        assert_eq!(report.sources.len(), 1);
        assert!(events
            .iter()
            .any(|event| matches!(event, ScanProgress::Searching)));
        assert!(events
            .iter()
            .any(|event| matches!(event, ScanProgress::FilesDiscovered { total: 2 })));
        assert!(events
            .iter()
            .any(|event| matches!(event, ScanProgress::InspectingZip { .. })));
    }

    #[test]
    fn progress_scan_cancels_without_returning_partial_results() {
        let directory = test_directory("cancel");
        for index in 0..8 {
            std::fs::write(directory.join(format!("game-{index}.gb")), test_rom(index)).unwrap();
        }
        let cancel = AtomicBool::new(false);
        let outcome = scan_with_progress(&directory, &cancel, |event| {
            if matches!(event, ScanProgress::Reading { .. }) {
                cancel.store(true, Ordering::Release);
            }
        });
        assert_eq!(outcome, ScanOutcome::Cancelled);
    }
}
