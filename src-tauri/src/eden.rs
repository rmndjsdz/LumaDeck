use crate::settings::DatabaseState;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

const EDEN_PROVIDER_ID: &str = "eden";
const EDEN_INSTALLATION_KEY: &str = "emulator.eden.installation";
const EDEN_TITLE_ID_PATTERN_PREFIX: &str = "010";
const SUPPORTED_EXTENSIONS: [&str; 2] = ["nsp", "xci"];
const EDEN_PROFILE_RECORD_SIZE: usize = 200;
const EDEN_PROFILE_HEADER_SIZE: usize = 16;
const EDEN_PROFILE_MAX_COUNT: usize = 8;
const EDEN_PLAYTIME_RECORD_SIZE: usize = 16;
const EDEN_IDENTITY_CORRELATION: &str = "eden-identity";

#[derive(Debug, Error)]
pub enum EdenError {
    #[error("EDEN executable does not exist")]
    ExecutableMissing,
    #[error("selected executable is not a valid Eden PE executable")]
    InvalidExecutable,
    #[error("EDEN configuration is corrupt: {0}")]
    CorruptConfiguration(String),
    #[error("EDEN installation has not been configured")]
    NotConfigured,
    #[error("EDEN installation could not be saved")]
    Persistence(String),
    #[error("EDEN game file is unavailable")]
    GameMissing,
    #[error("EDEN game file is outside the configured library roots")]
    GameOutsideLibrary,
    #[error("EDEN game file type is not supported")]
    UnsupportedGame,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EdenLibraryRoot {
    pub path: String,
    pub deep_scan: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EdenInstallation {
    pub executable_path: String,
    pub data_path: Option<String>,
    pub config_path: Option<String>,
    pub portable: bool,
    pub library_roots: Vec<EdenLibraryRoot>,
    #[serde(default)]
    pub manual_library_roots: Vec<EdenLibraryRoot>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EdenRootStatus {
    pub path: String,
    pub deep_scan: bool,
    pub available: bool,
    pub game_count: usize,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EdenStatus {
    pub provider_id: String,
    pub status: String,
    pub executable_path: Option<String>,
    pub data_path: Option<String>,
    pub config_path: Option<String>,
    pub portable: bool,
    pub configuration_found: bool,
    pub library_roots: Vec<EdenRootStatus>,
    pub profiles: Vec<EdenProfile>,
    pub games_detected: usize,
    pub duplicate_games: usize,
    pub playtime_synced: usize,
    pub playtime_unavailable: usize,
    pub playtime_file_found: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EdenExecutableInspection {
    pub executable_path: String,
    pub valid: bool,
    pub data_path: Option<String>,
    pub config_path: Option<String>,
    pub portable: bool,
    pub configuration_found: bool,
    pub library_roots: Vec<EdenLibraryRoot>,
    pub profiles: Vec<EdenProfile>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EdenProfile {
    pub id: String,
    pub name: String,
    pub avatar_data_url: Option<String>,
    pub is_current: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdenLaunchTarget {
    pub executable_path: PathBuf,
    pub game_path: PathBuf,
}

#[derive(Debug, Clone)]
struct EdenGame {
    identity: String,
    path: String,
    display_name: String,
    title_id: Option<String>,
    playtime_minutes: i64,
    playtime_seconds: Option<i64>,
    last_played_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct EdenPlaytimeEntry {
    title_id: String,
    total_seconds: u64,
}

#[derive(Debug, Default, Clone)]
struct EdenPlaytimeReadResult {
    entries: HashMap<String, EdenPlaytimeEntry>,
    last_played_at: HashMap<String, String>,
    file_found: bool,
    parse_error: Option<String>,
}

#[derive(Debug, Clone)]
struct Discovery {
    inspection: EdenExecutableInspection,
    roots: Vec<EdenRootStatus>,
    games: Vec<EdenGame>,
    duplicate_games: usize,
    playtime: EdenPlaytimeReadResult,
}

#[derive(Debug, Default)]
struct ParsedConfig {
    roots: Vec<EdenLibraryRoot>,
    current_profile_index: Option<usize>,
}

pub fn inspect_executable(executable_path: &str) -> Result<EdenExecutableInspection, EdenError> {
    let path = validate_executable(executable_path)?;
    discover_paths(&path)
}

pub fn get_status(state: &DatabaseState) -> Result<EdenStatus, EdenError> {
    let Some(installation) = read_installation(state)? else {
        return Ok(EdenStatus {
            provider_id: EDEN_PROVIDER_ID.to_string(),
            status: "not-configured".to_string(),
            executable_path: None,
            data_path: None,
            config_path: None,
            portable: false,
            configuration_found: false,
            library_roots: Vec::new(),
            profiles: Vec::new(),
            games_detected: 0,
            duplicate_games: 0,
            playtime_synced: 0,
            playtime_unavailable: 0,
            playtime_file_found: false,
            warnings: Vec::new(),
        });
    };
    status_from_installation(&installation, state)
}

pub fn reconcile_existing_identities(state: &DatabaseState) -> Result<usize, EdenError> {
    let Some(installation) = read_installation(state)? else {
        return Ok(0);
    };
    let installation_id = eden_installation_id(&installation);
    let connection = state
        .connection
        .lock()
        .map_err(|_| EdenError::Persistence("database lock poisoned".to_string()))?;
    let transaction = connection
        .unchecked_transaction()
        .map_err(|error| EdenError::Persistence(error.to_string()))?;
    let mut events = Vec::new();
    reconcile_existing_eden_title_ids(&transaction, &installation_id, &mut events)?;
    transaction
        .commit()
        .map_err(|error| EdenError::Persistence(error.to_string()))?;
    for (checkpoint, details) in events {
        state.log(EDEN_IDENTITY_CORRELATION, checkpoint, &details);
    }
    Ok(1)
}

pub fn connect(
    state: &DatabaseState,
    executable_path: &str,
    manual_library_roots: &[String],
) -> Result<EdenStatus, EdenError> {
    let path = validate_executable(executable_path)?;
    let mut inspection = discover_paths(&path)?;
    let manual_roots = normalize_roots(manual_library_roots.iter().map(|path| EdenLibraryRoot {
        path: path.clone(),
        deep_scan: true,
    }));
    if inspection.library_roots.is_empty() && !manual_roots.is_empty() {
        inspection.library_roots = manual_roots.clone();
        inspection.warnings.push(
            "No se encontraron game directories en la configuración; se usa el fallback manual."
                .to_string(),
        );
    }
    let installation = EdenInstallation {
        executable_path: path.to_string_lossy().into_owned(),
        data_path: inspection.data_path.clone(),
        config_path: inspection.config_path.clone(),
        portable: inspection.portable,
        library_roots: inspection.library_roots.clone(),
        manual_library_roots: manual_roots,
    };
    write_installation(state, &installation)?;
    let discovery = discover_installation(&installation)?;
    persist_games(
        state,
        &installation,
        &discovery.games,
        &discovery.roots,
        &discovery.playtime,
        "connect",
    )?;
    status_from_discovery(&discovery)
}

pub fn rescan(state: &DatabaseState) -> Result<EdenStatus, EdenError> {
    let Some(mut installation) = read_installation(state)? else {
        return Err(EdenError::NotConfigured);
    };
    let executable = validate_executable(&installation.executable_path)?;
    let inspection = discover_paths(&executable)?;
    if !inspection.configuration_found && installation.manual_library_roots.is_empty() {
        return status_from_discovery(&Discovery {
            inspection,
            roots: Vec::new(),
            games: Vec::new(),
            duplicate_games: 0,
            playtime: EdenPlaytimeReadResult::default(),
        });
    }
    if inspection.configuration_found {
        installation.data_path = inspection.data_path.clone();
        installation.config_path = inspection.config_path.clone();
        installation.portable = inspection.portable;
        installation.library_roots = inspection.library_roots.clone();
    } else {
        installation.library_roots = installation.manual_library_roots.clone();
    }
    write_installation(state, &installation)?;
    let discovery = discover_installation(&installation)?;
    persist_games(
        state,
        &installation,
        &discovery.games,
        &discovery.roots,
        &discovery.playtime,
        "rescan",
    )?;
    status_from_discovery(&discovery)
}

pub fn sync_playtime_after_session(state: &DatabaseState) -> Result<EdenStatus, EdenError> {
    let Some(installation) = read_installation(state)? else {
        return Err(EdenError::NotConfigured);
    };
    let discovery = discover_installation(&installation)?;
    persist_games(
        state,
        &installation,
        &discovery.games,
        &discovery.roots,
        &discovery.playtime,
        "session_end",
    )?;
    status_from_discovery(&discovery)
}

pub fn external_playtime_seconds(
    state: &DatabaseState,
    game_id: &str,
) -> Result<Option<i64>, EdenError> {
    let connection = state
        .connection
        .lock()
        .map_err(|_| EdenError::Persistence("database lock poisoned".to_string()))?;
    connection
        .query_row(
            "SELECT total_seconds
             FROM external_playtime_snapshots
             WHERE provider = 'eden' AND game_id = ?1
             ORDER BY observed_at DESC
             LIMIT 1",
            params![game_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| EdenError::Persistence(error.to_string()))
}

pub fn installation_id(state: &DatabaseState) -> Result<Option<String>, EdenError> {
    read_installation(state)
        .map(|installation| installation.map(|value| eden_installation_id(&value)))
}

pub fn disconnect(state: &DatabaseState) -> Result<EdenStatus, EdenError> {
    let connection = state
        .connection
        .lock()
        .map_err(|_| EdenError::Persistence("database lock poisoned".to_string()))?;
    connection
        .execute(
            "DELETE FROM app_settings WHERE key = ?1",
            params![EDEN_INSTALLATION_KEY],
        )
        .map_err(|error| EdenError::Persistence(error.to_string()))?;
    get_status(state)
}

pub fn resolve_launch_target(
    state: &DatabaseState,
    game_path: &str,
) -> Result<EdenLaunchTarget, EdenError> {
    let Some(installation) = read_installation(state)? else {
        return Err(EdenError::NotConfigured);
    };
    let executable_path = validate_executable(&installation.executable_path)?;
    let game_path = PathBuf::from(game_path.trim());
    if !game_path.is_file() {
        return Err(EdenError::GameMissing);
    }
    if !supported_extension(&game_path) {
        return Err(EdenError::UnsupportedGame);
    }
    let roots = installation
        .library_roots
        .iter()
        .chain(installation.manual_library_roots.iter())
        .filter_map(|root| {
            let path = PathBuf::from(&root.path);
            path.is_dir().then(|| normalize_path_key(&path))
        })
        .collect::<Vec<_>>();
    let game_key = normalize_path_key(&game_path);
    if !roots.iter().any(|root| is_within_root(&game_key, root)) {
        return Err(EdenError::GameOutsideLibrary);
    }
    Ok(EdenLaunchTarget {
        executable_path,
        game_path,
    })
}

fn validate_executable(executable_path: &str) -> Result<PathBuf, EdenError> {
    let path = PathBuf::from(executable_path.trim());
    if !path.is_file() {
        return Err(EdenError::ExecutableMissing);
    }
    let is_eden_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("eden.exe"));
    if !is_eden_name {
        return Err(EdenError::InvalidExecutable);
    }
    let bytes = fs::read(&path).map_err(|_| EdenError::InvalidExecutable)?;
    if bytes.len() < 2 || &bytes[..2] != b"MZ" {
        return Err(EdenError::InvalidExecutable);
    }
    Ok(path)
}

fn discover_paths(executable: &Path) -> Result<EdenExecutableInspection, EdenError> {
    let executable_dir = executable.parent().ok_or(EdenError::InvalidExecutable)?;
    let portable_root = executable_dir.join("user");
    let portable_config = portable_root.join("config").join("qt-config.ini");
    let app_data = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .map(|path| path.join("eden"));
    let app_data_config = app_data
        .as_ref()
        .map(|path| path.join("config").join("qt-config.ini"));

    let (data_path, config_path, portable) = if portable_config.is_file() || portable_root.is_dir()
    {
        (Some(portable_root), portable_config, true)
    } else if let (Some(data_path), Some(config_path)) = (app_data, app_data_config) {
        (Some(data_path), config_path, false)
    } else {
        (None, PathBuf::new(), false)
    };
    let configuration_found = config_path.is_file();
    let mut warnings = Vec::new();
    let (roots, current_profile_index) = if configuration_found {
        match parse_config(&config_path) {
            Ok(parsed) => (parsed.roots, parsed.current_profile_index),
            Err(error) => {
                warnings.push(error.to_string());
                (Vec::new(), None)
            }
        }
    } else {
        warnings
            .push("No se encontró qt-config.ini en la instalación normal ni portable.".to_string());
        (Vec::new(), None)
    };
    let profiles = read_profiles(data_path.as_deref(), current_profile_index);
    Ok(EdenExecutableInspection {
        executable_path: executable.to_string_lossy().into_owned(),
        valid: true,
        data_path: data_path.map(|value| value.to_string_lossy().into_owned()),
        config_path: configuration_found.then(|| config_path.to_string_lossy().into_owned()),
        portable,
        configuration_found,
        library_roots: roots,
        profiles,
        warnings,
    })
}

fn parse_config(config_path: &Path) -> Result<ParsedConfig, EdenError> {
    let content = fs::read_to_string(config_path)
        .map_err(|error| EdenError::CorruptConfiguration(error.to_string()))?;
    let mut entries: BTreeMap<usize, EdenLibraryRoot> = BTreeMap::new();
    let mut in_paths = false;
    let mut current_profile_index = None;
    let mut legacy_root: Option<String> = None;
    let mut legacy_deep_scan = false;
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            in_paths = line[1..line.len() - 1].eq_ignore_ascii_case("Paths");
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = unquote(value.trim());
        let parts: Vec<&str> = key.split('\\').collect();
        if parts.len() == 1 && parts[0].eq_ignore_ascii_case("current_user") {
            current_profile_index = value.parse::<usize>().ok();
            continue;
        }
        // QSettings writes the same data in two forms depending on the Eden
        // build: inside a `[Paths]` section, or flattened as
        // `Paths\\gamedirs\\N\\path`. Support both forms.
        let prefix = if parts
            .first()
            .is_some_and(|part| part.eq_ignore_ascii_case("Paths"))
        {
            1
        } else if in_paths {
            0
        } else {
            continue;
        };
        if parts.len() >= prefix + 3 && parts[prefix].eq_ignore_ascii_case("gamedirs") {
            let Ok(index) = parts[prefix + 1].parse::<usize>() else {
                continue;
            };
            let entry = entries.entry(index).or_insert_with(|| EdenLibraryRoot {
                path: String::new(),
                deep_scan: false,
            });
            if parts[prefix + 2].eq_ignore_ascii_case("path") {
                entry.path = normalize_path_text(&value);
            } else if parts[prefix + 2].eq_ignore_ascii_case("deep_scan") {
                entry.deep_scan = parse_bool(&value);
            }
        } else if parts.len() == prefix + 1 && parts[prefix].eq_ignore_ascii_case("gameListRootDir")
        {
            legacy_root = Some(normalize_path_text(&value));
        } else if parts.len() == prefix + 1
            && parts[prefix].eq_ignore_ascii_case("gameListDeepScan")
        {
            legacy_deep_scan = parse_bool(&value);
        }
    }
    let mut roots: Vec<EdenLibraryRoot> = entries
        .into_values()
        .filter(|entry| !entry.path.is_empty())
        .collect();
    if roots.is_empty() {
        if let Some(path) = legacy_root.filter(|value| value != ".") {
            roots.push(EdenLibraryRoot {
                path,
                deep_scan: legacy_deep_scan,
            });
        }
    }
    Ok(ParsedConfig {
        roots: normalize_roots(roots),
        current_profile_index,
    })
}

fn read_profiles(
    data_path: Option<&Path>,
    current_profile_index: Option<usize>,
) -> Vec<EdenProfile> {
    let Some(data_path) = data_path else {
        return Vec::new();
    };
    let profiles_dir = data_path
        .join("nand")
        .join("system")
        .join("save")
        .join("8000000000000010")
        .join("su")
        .join("avators");
    let profile_file = profiles_dir.join("profiles.dat");
    let Ok(bytes) = fs::read(profile_file) else {
        return Vec::new();
    };
    let mut profiles = Vec::new();
    for index in 0..EDEN_PROFILE_MAX_COUNT {
        let offset = EDEN_PROFILE_HEADER_SIZE + index * EDEN_PROFILE_RECORD_SIZE;
        let Some(record) = bytes.get(offset..offset + EDEN_PROFILE_RECORD_SIZE) else {
            break;
        };
        let Some(id_bytes) = record.get(..16) else {
            continue;
        };
        if id_bytes.iter().all(|byte| *byte == 0) {
            continue;
        }
        let id = format_uuid(id_bytes);
        let name = record
            .get(40..72)
            .map(|value| {
                String::from_utf8_lossy(value)
                    .trim_matches('\0')
                    .trim()
                    .to_string()
            })
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "Perfil Eden".to_string());
        let avatar_data_url = read_profile_avatar(&profiles_dir, &id);
        profiles.push(EdenProfile {
            id,
            name,
            avatar_data_url,
            is_current: current_profile_index == Some(index),
        });
    }
    profiles
}

fn read_profile_avatar(profiles_dir: &Path, id: &str) -> Option<String> {
    let bytes = fs::read(profiles_dir.join(format!("{id}.jpg"))).ok()?;
    let mime = if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        "image/jpeg"
    } else if bytes.starts_with(b"\x89PNG") {
        "image/png"
    } else {
        return None;
    };
    Some(format!("data:{mime};base64,{}", BASE64.encode(bytes)))
}

fn format_uuid(bytes: &[u8]) -> String {
    let hex = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

fn discover_installation(installation: &EdenInstallation) -> Result<Discovery, EdenError> {
    let executable = validate_executable(&installation.executable_path)?;
    let mut inspection = discover_paths(&executable)?;
    if !inspection.configuration_found {
        inspection.library_roots = installation.library_roots.clone();
        inspection.warnings.push(
            "Discovery de configuración no disponible; se conservan los roots guardados."
                .to_string(),
        );
    }
    let roots = normalize_roots(inspection.library_roots.iter().cloned());
    let title_map = build_title_map(inspection.data_path.as_deref());
    let stats = read_statistics(inspection.data_path.as_deref());
    let mut reports = Vec::new();
    let mut games_by_identity: BTreeMap<String, EdenGame> = BTreeMap::new();
    let mut duplicate_games = 0;
    for root in &roots {
        let scan = scan_root(root);
        let mut game_count = 0;
        if scan.available {
            for path in scan.files {
                let display_name = path
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .unwrap_or("Unknown game")
                    .replace(['_', '.'], " ");
                let title_id = title_id_from_path(&path)
                    .or_else(|| title_map.get(&normalize_name(&display_name)).cloned());
                let identity = title_id
                    .clone()
                    .map(|id| format!("title:{id}"))
                    .unwrap_or_else(|| format!("path:{}", normalize_path_key(&path)));
                let game = EdenGame {
                    identity: identity.clone(),
                    path: path.to_string_lossy().into_owned(),
                    display_name,
                    playtime_minutes: title_id
                        .as_deref()
                        .and_then(|id| {
                            stats
                                .entries
                                .get(id)
                                .map(|value| value.total_seconds.saturating_add(30) / 60)
                        })
                        .unwrap_or(0)
                        .min(i64::MAX as u64) as i64,
                    playtime_seconds: title_id.as_deref().and_then(|id| {
                        stats
                            .entries
                            .get(id)
                            .and_then(|value| i64::try_from(value.total_seconds).ok())
                    }),
                    last_played_at: title_id
                        .as_deref()
                        .and_then(|id| stats.last_played_at.get(id).cloned()),
                    title_id,
                };
                match games_by_identity.entry(identity) {
                    std::collections::btree_map::Entry::Vacant(entry) => {
                        entry.insert(game);
                        game_count += 1;
                    }
                    std::collections::btree_map::Entry::Occupied(mut entry) => {
                        duplicate_games += 1;
                        let current_path = normalize_path_key(Path::new(&entry.get().path));
                        let candidate_path = normalize_path_key(Path::new(&game.path));
                        if candidate_path < current_path {
                            entry.insert(game);
                        }
                    }
                }
            }
        }
        reports.push(EdenRootStatus {
            path: root.path.clone(),
            deep_scan: root.deep_scan,
            available: scan.available,
            game_count,
            error: scan.error,
        });
    }
    reports.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(Discovery {
        inspection,
        roots: reports,
        games: games_by_identity.into_values().collect(),
        duplicate_games,
        playtime: stats,
    })
}

struct RootScan {
    available: bool,
    files: Vec<PathBuf>,
    error: Option<String>,
}

fn scan_root(root: &EdenLibraryRoot) -> RootScan {
    let root_path = PathBuf::from(&root.path);
    if !root_path.is_dir() {
        return RootScan {
            available: false,
            files: Vec::new(),
            error: Some("Carpeta inaccesible o desconectada.".to_string()),
        };
    }
    let mut files = Vec::new();
    let mut visited = HashSet::new();
    let result = scan_directory(&root_path, root.deep_scan, &mut visited, &mut files);
    if let Err(error) = result {
        return RootScan {
            available: false,
            files: Vec::new(),
            error: Some(error),
        };
    }
    files.sort_by(|left, right| normalize_path_key(left).cmp(&normalize_path_key(right)));
    RootScan {
        available: true,
        files,
        error: None,
    }
}

fn scan_directory(
    path: &Path,
    recursive: bool,
    visited: &mut HashSet<String>,
    files: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let key = normalize_path_key(path);
    if !visited.insert(key) {
        return Ok(());
    }
    let entries = fs::read_dir(path).map_err(|error| error.to_string())?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let entry_path = entry.path();
        let file_type = entry.file_type().map_err(|error| error.to_string())?;
        if file_type.is_file() && supported_extension(&entry_path) {
            files.push(entry_path);
        } else if recursive && file_type.is_dir() {
            scan_directory(&entry_path, true, visited, files)?;
        }
    }
    Ok(())
}

#[derive(Debug, Default)]
struct PlaytimeSyncReport {
    matched: usize,
    updated: usize,
    unchanged: usize,
    decreased: usize,
    unavailable: usize,
}

fn persist_games(
    state: &DatabaseState,
    installation: &EdenInstallation,
    games: &[EdenGame],
    roots: &[EdenRootStatus],
    playtime: &EdenPlaytimeReadResult,
    source: &str,
) -> Result<(), EdenError> {
    let now = unix_timestamp();
    state.log(
        "eden-playtime",
        "EDEN_PLAYTIME_SYNC_STARTED",
        &format!("emulator=eden source={source}"),
    );
    if playtime.file_found {
        state.log(
            "eden-playtime",
            "EDEN_PLAYTIME_FILE_FOUND",
            "emulator=eden format=playtime.bin:v1:le:u64,u64",
        );
    }
    if playtime.parse_error.is_none() && playtime.file_found {
        state.log(
            "eden-playtime",
            "EDEN_PLAYTIME_PARSED",
            &format!("emulator=eden entries={}", playtime.entries.len()),
        );
    }
    let connection = state
        .connection
        .lock()
        .map_err(|_| EdenError::Persistence("database lock poisoned".to_string()))?;
    let transaction = connection
        .unchecked_transaction()
        .map_err(|error| EdenError::Persistence(error.to_string()))?;
    let installation_id = eden_installation_id(installation);
    let mut playtime_report = PlaytimeSyncReport::default();
    transaction.execute("INSERT OR IGNORE INTO providers(id, display_name, enabled, created_at, updated_at) VALUES ('eden', 'Eden', 1, ?1, ?1)", params![now]).map_err(|error| EdenError::Persistence(error.to_string()))?;
    let mut found_paths = HashSet::new();
    let mut identity_events = Vec::new();
    reconcile_existing_eden_title_ids(&transaction, &installation_id, &mut identity_events)?;
    for game in games {
        found_paths.insert(normalize_path_key(Path::new(&game.path)));
        let game_id = persisted_game_id(&installation_id, game);
        let previous_canonical_path = load_eden_games(&transaction, &installation_id)?
            .into_iter()
            .find(|candidate| candidate.id == game_id)
            .and_then(|candidate| candidate.game_path);
        let had_existing_title_id = match game.title_id.as_deref() {
            Some(title_id) => has_eden_title_id(&transaction, &installation_id, title_id)?,
            None => false,
        };
        let had_existing_path = has_eden_path(&transaction, &installation_id, &game.path)?;
        transaction.execute(
            "INSERT INTO games(id, title, sort_title, provider, platform, source, emulator_id, emulator_installation_id, game_path, title_id, playtime_minutes, last_played_at, installed, created_at, updated_at) VALUES (?1, ?2, ?3, 'Eden', 'Nintendo Switch', 'emulator', 'eden', ?4, ?5, ?6, ?7, ?8, 1, ?9, ?9) ON CONFLICT(id) DO UPDATE SET title = excluded.title, sort_title = excluded.sort_title, emulator_installation_id = excluded.emulator_installation_id, game_path = excluded.game_path, title_id = excluded.title_id, playtime_minutes = MAX(games.playtime_minutes, excluded.playtime_minutes), last_played_at = CASE WHEN excluded.last_played_at IS NULL THEN games.last_played_at WHEN games.last_played_at IS NULL THEN excluded.last_played_at WHEN CAST(excluded.last_played_at AS INTEGER) > CAST(games.last_played_at AS INTEGER) THEN excluded.last_played_at ELSE games.last_played_at END, installed = 1, missing_since = NULL, updated_at = excluded.updated_at",
            params![game_id, game.display_name, game.display_name.to_lowercase(), installation_id, game.path, game.title_id, game.playtime_minutes, game.last_played_at, now],
        ).map_err(|error| EdenError::Persistence(error.to_string()))?;
        if let (Some(title_id), Some(previous_path)) =
            (game.title_id.as_deref(), previous_canonical_path.as_deref())
        {
            if normalize_path_key(Path::new(previous_path))
                != normalize_path_key(Path::new(&game.path))
            {
                record_identity_event(
                    &mut identity_events,
                    "eden_game_path_updated",
                    &game_id,
                    Some(title_id),
                    &installation_id,
                );
            }
        }
        let merged_count = if let Some(title_id) = game.title_id.as_deref() {
            reconcile_title_id_records(
                &transaction,
                &installation_id,
                title_id,
                &game.path,
                &game.display_name,
                &game_id,
                &mut identity_events,
            )?
        } else {
            reconcile_provisional_path_records(
                &transaction,
                &installation_id,
                &game.path,
                &game_id,
                &mut identity_events,
            )?
        };
        if let Some(title_id) = game.title_id.as_deref() {
            if !had_existing_title_id {
                record_identity_event(
                    &mut identity_events,
                    "eden_title_id_discovered",
                    &game_id,
                    Some(title_id),
                    &installation_id,
                );
            }
        } else if !had_existing_path && merged_count == 0 {
            record_identity_event(
                &mut identity_events,
                "eden_identity_provisional_created",
                &game_id,
                None,
                &installation_id,
            );
        }
        let external_id = game.identity.clone();
        transaction
            .execute(
                "DELETE FROM game_provider_links
                 WHERE game_id = ?1 AND provider_id = 'eden' AND external_id <> ?2",
                params![game_id, external_id],
            )
            .map_err(|error| EdenError::Persistence(error.to_string()))?;
        transaction.execute(
            "INSERT INTO game_provider_links(game_id, provider_id, external_id, is_owned, last_synced_at) VALUES (?1, 'eden', ?2, 1, ?3) ON CONFLICT(provider_id, external_id) DO UPDATE SET game_id = excluded.game_id, last_synced_at = excluded.last_synced_at",
            params![game_id, external_id, now],
        ).map_err(|error| EdenError::Persistence(error.to_string()))?;
        let Some(title_id) = game.title_id.as_deref() else {
            playtime_report.unavailable += 1;
            continue;
        };
        let Some(total_seconds) = game.playtime_seconds else {
            playtime_report.unavailable += 1;
            continue;
        };
        playtime_report.matched += 1;
        let snapshot_previous: Option<i64> = transaction
            .query_row(
                "SELECT total_seconds FROM external_playtime_snapshots
                 WHERE provider = 'eden' AND emulator_installation_id = ?1 AND title_id = ?2",
                params![installation_id, title_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| EdenError::Persistence(error.to_string()))?;
        let fallback_previous: i64 = transaction
            .query_row(
                "SELECT COALESCE(playtime_minutes, 0) * 60 FROM games WHERE id = ?1",
                params![game_id],
                |row| row.get(0),
            )
            .map_err(|error| EdenError::Persistence(error.to_string()))?;
        let previous = snapshot_previous.unwrap_or(0).max(fallback_previous).max(0);
        match previous {
            value if total_seconds < value => playtime_report.decreased += 1,
            value if total_seconds == value => playtime_report.unchanged += 1,
            _ => playtime_report.updated += 1,
        }
        if total_seconds >= previous {
            transaction
                .execute(
                    "INSERT INTO external_playtime_snapshots(
                        provider, emulator_installation_id, title_id, game_id,
                        total_seconds, observed_at, format
                     ) VALUES ('eden', ?1, ?2, ?3, ?4, ?5, 'playtime.bin:v1:le:u64,u64')
                     ON CONFLICT(provider, emulator_installation_id, title_id) DO UPDATE SET
                        game_id = excluded.game_id,
                        total_seconds = excluded.total_seconds,
                        observed_at = excluded.observed_at,
                        format = excluded.format
                     WHERE excluded.total_seconds >= external_playtime_snapshots.total_seconds",
                    params![installation_id, title_id, game_id, total_seconds, now],
                )
                .map_err(|error| EdenError::Persistence(error.to_string()))?;
        }
    }
    for root in roots.iter().filter(|root| root.available) {
        let root_key = normalize_path_key(Path::new(&root.path));
        let mut statement = transaction.prepare("SELECT id, game_path FROM games WHERE source = 'emulator' AND emulator_id = 'eden' AND game_path IS NOT NULL").map_err(|error| EdenError::Persistence(error.to_string()))?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|error| EdenError::Persistence(error.to_string()))?;
        for row in rows {
            let (id, path) = row.map_err(|error| EdenError::Persistence(error.to_string()))?;
            let path_key = normalize_path_key(Path::new(&path));
            if is_within_root(&path_key, &root_key) && !found_paths.contains(&path_key) {
                transaction.execute("UPDATE games SET installed = 0, missing_since = COALESCE(missing_since, ?1), updated_at = ?1 WHERE id = ?2", params![now, id]).map_err(|error| EdenError::Persistence(error.to_string()))?;
            }
        }
    }
    transaction
        .commit()
        .map_err(|error| EdenError::Persistence(error.to_string()))?;
    for (checkpoint, details) in identity_events {
        state.log(EDEN_IDENTITY_CORRELATION, checkpoint, &details);
    }
    if let Some(error) = playtime.parse_error.as_deref() {
        state.log(
            "eden-playtime",
            "EDEN_PLAYTIME_PARSE_FAILED",
            &format!("emulator=eden error={error}"),
        );
    }
    state.log(
        "eden-playtime",
        "EDEN_PLAYTIME_SYNC_COMPLETED",
        &format!(
            "emulator=eden matched={} updated={} unchanged={} decreased={} unavailable={}",
            playtime_report.matched,
            playtime_report.updated,
            playtime_report.unchanged,
            playtime_report.decreased,
            playtime_report.unavailable
        ),
    );
    state.log(
        "eden-playtime",
        "EDEN_PLAYTIME_MATCHED",
        &format!("emulator=eden count={}", playtime_report.matched),
    );
    state.log(
        "eden-playtime",
        "EDEN_PLAYTIME_UPDATED",
        &format!("emulator=eden count={}", playtime_report.updated),
    );
    state.log(
        "eden-playtime",
        "EDEN_PLAYTIME_UNCHANGED",
        &format!("emulator=eden count={}", playtime_report.unchanged),
    );
    state.log(
        "eden-playtime",
        "EDEN_PLAYTIME_UNAVAILABLE",
        &format!("emulator=eden count={}", playtime_report.unavailable),
    );
    if playtime_report.decreased > 0 {
        state.log(
            "eden-playtime",
            "EDEN_PLAYTIME_DECREASED",
            &format!(
                "emulator=eden count={} action=preserved_previous",
                playtime_report.decreased
            ),
        );
    }
    Ok(())
}

type IdentityEvents = Vec<(&'static str, String)>;

#[derive(Debug, Clone)]
struct EdenDbGame {
    id: String,
    title: String,
    game_path: Option<String>,
    title_id: Option<String>,
    installed: bool,
}

fn persisted_game_id(installation_id: &str, game: &EdenGame) -> String {
    let identity = match game.title_id.as_deref() {
        Some(title_id) => format!("eden:{installation_id}:title:{title_id}"),
        None => format!(
            "eden:{installation_id}:path:{}",
            normalize_path_key(Path::new(&game.path))
        ),
    };
    game_id(&identity)
}

fn canonical_title_game_id(installation_id: &str, title_id: &str) -> String {
    game_id(&format!("eden:{installation_id}:title:{title_id}"))
}

fn has_eden_title_id(
    transaction: &Transaction<'_>,
    installation_id: &str,
    title_id: &str,
) -> Result<bool, EdenError> {
    transaction
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM games
                WHERE source = 'emulator' AND emulator_id = 'eden'
                  AND (emulator_installation_id = ?1 OR emulator_installation_id IS NULL)
                  AND title_id = ?2
            )",
            params![installation_id, title_id],
            |row| row.get(0),
        )
        .map_err(|error| EdenError::Persistence(error.to_string()))
}

fn has_eden_path(
    transaction: &Transaction<'_>,
    installation_id: &str,
    path: &str,
) -> Result<bool, EdenError> {
    let games = load_eden_games(transaction, installation_id)?;
    let path_key = normalize_path_key(Path::new(path));
    Ok(games.iter().any(|game| {
        game.game_path
            .as_deref()
            .is_some_and(|value| normalize_path_key(Path::new(value)) == path_key)
    }))
}

fn load_eden_games(
    transaction: &Transaction<'_>,
    installation_id: &str,
) -> Result<Vec<EdenDbGame>, EdenError> {
    let mut statement = transaction
        .prepare(
            "SELECT id, title, game_path, title_id, installed
             FROM games
             WHERE source = 'emulator' AND emulator_id = 'eden'
               AND (?1 = '' OR emulator_installation_id = ?1 OR emulator_installation_id IS NULL)",
        )
        .map_err(|error| EdenError::Persistence(error.to_string()))?;
    let result = statement
        .query_map(params![installation_id], |row| {
            Ok(EdenDbGame {
                id: row.get(0)?,
                title: row.get(1)?,
                game_path: row.get(2)?,
                title_id: row.get(3)?,
                installed: row.get::<_, i64>(4)? != 0,
            })
        })
        .map_err(|error| EdenError::Persistence(error.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| EdenError::Persistence(error.to_string()));
    result
}

fn reconcile_existing_eden_title_ids(
    transaction: &Transaction<'_>,
    installation_id: &str,
    events: &mut IdentityEvents,
) -> Result<(), EdenError> {
    let mut statement = transaction
        .prepare(
            "SELECT DISTINCT title_id
             FROM games
             WHERE source = 'emulator' AND emulator_id = 'eden'
               AND title_id IS NOT NULL AND title_id <> ''
               AND (emulator_installation_id = ?1 OR emulator_installation_id IS NULL)",
        )
        .map_err(|error| EdenError::Persistence(error.to_string()))?;
    let title_ids = statement
        .query_map(params![installation_id], |row| row.get::<_, String>(0))
        .map_err(|error| EdenError::Persistence(error.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| EdenError::Persistence(error.to_string()))?;

    for title_id in title_ids {
        let canonical_id = canonical_title_game_id(installation_id, &title_id);
        let exists: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM games WHERE id = ?1)",
                params![canonical_id],
                |row| row.get(0),
            )
            .map_err(|error| EdenError::Persistence(error.to_string()))?;
        if !exists {
            let source_id = load_eden_games(transaction, installation_id)?
                .into_iter()
                .filter(|game| game.title_id.as_deref() == Some(title_id.as_str()))
                .map(|game| game.id)
                .min();
            if let Some(source_id) = source_id {
                clone_game_row(transaction, &source_id, &canonical_id, installation_id)?;
            }
        }
        reconcile_title_id_records(
            transaction,
            installation_id,
            &title_id,
            "",
            "",
            &canonical_id,
            events,
        )?;
    }
    Ok(())
}

fn reconcile_title_id_records(
    transaction: &Transaction<'_>,
    installation_id: &str,
    title_id: &str,
    current_path: &str,
    current_title: &str,
    canonical_id: &str,
    events: &mut IdentityEvents,
) -> Result<usize, EdenError> {
    let games = load_eden_games(transaction, installation_id)?;
    let current_path_key =
        (!current_path.is_empty()).then(|| normalize_path_key(Path::new(current_path)));
    let mut candidates = games
        .iter()
        .filter(|game| game.id != canonical_id)
        .filter(|game| {
            game.title_id.as_deref() == Some(title_id)
                || (game.title_id.is_none()
                    && current_path_key.as_deref().is_some_and(|path_key| {
                        game.game_path
                            .as_deref()
                            .is_some_and(|path| normalize_path_key(Path::new(path)) == *path_key)
                    }))
        })
        .map(|game| game.id.clone())
        .collect::<Vec<_>>();

    if candidates.is_empty() && !current_title.is_empty() {
        let normalized_title = normalize_name(current_title);
        let provisional_matches = games
            .iter()
            .filter(|game| {
                !game.installed
                    && game.title_id.is_none()
                    && normalize_name(&game.title) == normalized_title
            })
            .map(|game| game.id.clone())
            .collect::<Vec<_>>();
        if provisional_matches.len() == 1 {
            candidates = provisional_matches;
        }
    }

    candidates.sort();
    candidates.dedup();
    if candidates.is_empty() {
        return Ok(0);
    }

    record_identity_event(
        events,
        "eden_duplicate_title_id_detected",
        canonical_id,
        Some(title_id),
        installation_id,
    );

    record_identity_event(
        events,
        "eden_identity_reconciliation_started",
        canonical_id,
        Some(title_id),
        installation_id,
    );
    let mut merged = 0;
    for source_id in candidates {
        if !current_path.is_empty() {
            let previous_path: Option<String> = transaction
                .query_row(
                    "SELECT game_path FROM games WHERE id = ?1",
                    params![source_id],
                    |row| row.get(0),
                )
                .optional()
                .map_err(|error| EdenError::Persistence(error.to_string()))?;
            if previous_path.as_deref().is_some_and(|path| {
                normalize_path_key(Path::new(path)) != normalize_path_key(Path::new(current_path))
            }) {
                record_identity_event(
                    events,
                    "eden_game_path_updated",
                    canonical_id,
                    Some(title_id),
                    installation_id,
                );
            }
        }
        merge_eden_game_records(transaction, &source_id, canonical_id, installation_id)?;
        merged += 1;
    }
    record_identity_event(
        events,
        "eden_identity_reconciliation_completed",
        canonical_id,
        Some(title_id),
        installation_id,
    );
    Ok(merged)
}

fn reconcile_provisional_path_records(
    transaction: &Transaction<'_>,
    installation_id: &str,
    current_path: &str,
    canonical_id: &str,
    events: &mut IdentityEvents,
) -> Result<usize, EdenError> {
    let path_key = normalize_path_key(Path::new(current_path));
    let candidates = load_eden_games(transaction, installation_id)?
        .into_iter()
        .filter(|game| {
            game.id != canonical_id
                && game.title_id.is_none()
                && game
                    .game_path
                    .as_deref()
                    .is_some_and(|path| normalize_path_key(Path::new(path)) == path_key)
        })
        .map(|game| game.id)
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Ok(0);
    }
    record_identity_event(
        events,
        "eden_identity_reconciliation_started",
        canonical_id,
        None,
        installation_id,
    );
    for source_id in &candidates {
        merge_eden_game_records(transaction, source_id, canonical_id, installation_id)?;
    }
    record_identity_event(
        events,
        "eden_identity_reconciliation_completed",
        canonical_id,
        None,
        installation_id,
    );
    Ok(candidates.len())
}

fn clone_game_row(
    transaction: &Transaction<'_>,
    source_id: &str,
    canonical_id: &str,
    installation_id: &str,
) -> Result<(), EdenError> {
    transaction
        .execute(
            "INSERT INTO games(
                id, title, sort_title, provider, platform, favorite, installed, progress, status,
                created_at, updated_at, hidden, source, emulator_id, emulator_installation_id,
                game_path, title_id, playtime_minutes, last_played_at, missing_since
             )
             SELECT ?1, title, sort_title, provider, platform, favorite, installed, progress,
                    status, created_at, updated_at, hidden, source, emulator_id, ?3,
                    game_path, title_id, playtime_minutes, last_played_at, missing_since
             FROM games WHERE id = ?2",
            params![canonical_id, source_id, installation_id],
        )
        .map_err(|error| EdenError::Persistence(error.to_string()))?;
    Ok(())
}

fn merge_eden_game_records(
    transaction: &Transaction<'_>,
    source_id: &str,
    canonical_id: &str,
    installation_id: &str,
) -> Result<(), EdenError> {
    if source_id == canonical_id {
        return Ok(());
    }
    transaction
        .execute(
            "UPDATE games
             SET favorite = MAX(favorite, (SELECT favorite FROM games WHERE id = ?1)),
                 hidden = MAX(hidden, (SELECT hidden FROM games WHERE id = ?1)),
                 progress = MAX(progress, (SELECT progress FROM games WHERE id = ?1)),
                 status = CASE
                    WHEN status = 'completed' THEN status
                    WHEN (SELECT status FROM games WHERE id = ?1) = 'completed' THEN 'completed'
                    WHEN status = 'not-started'
                         AND (SELECT status FROM games WHERE id = ?1) = 'playing'
                        THEN 'playing'
                    ELSE status
                 END,
                 playtime_minutes = MAX(playtime_minutes, (SELECT playtime_minutes FROM games WHERE id = ?1)),
                 last_played_at = CASE
                    WHEN last_played_at IS NULL THEN (SELECT last_played_at FROM games WHERE id = ?1)
                    WHEN (SELECT last_played_at FROM games WHERE id = ?1) IS NULL THEN last_played_at
                    WHEN CAST((SELECT last_played_at FROM games WHERE id = ?1) AS INTEGER) > CAST(last_played_at AS INTEGER)
                        THEN (SELECT last_played_at FROM games WHERE id = ?1)
                    ELSE last_played_at
                 END,
                 emulator_installation_id = ?2
             WHERE id = ?3",
            params![source_id, installation_id, canonical_id],
        )
        .map_err(|error| EdenError::Persistence(error.to_string()))?;

    merge_singleton_game_table(transaction, "game_details", source_id, canonical_id)?;
    merge_singleton_game_table(transaction, "hltb_game_times", source_id, canonical_id)?;
    merge_singleton_game_table(transaction, "hltb_match_overrides", source_id, canonical_id)?;
    merge_singleton_game_table(
        transaction,
        "game_display_profiles",
        source_id,
        canonical_id,
    )?;
    merge_singleton_game_table(
        transaction,
        "game_frame_generation_profiles",
        source_id,
        canonical_id,
    )?;
    merge_singleton_game_table(transaction, "game_reviews_cache", source_id, canonical_id)?;
    merge_singleton_game_table(
        transaction,
        "game_review_consensus",
        source_id,
        canonical_id,
    )?;
    merge_singleton_game_table(
        transaction,
        "steam_achievement_sync_state",
        source_id,
        canonical_id,
    )?;
    merge_artwork_selections(transaction, source_id, canonical_id)?;
    merge_external_playtime(transaction, source_id, canonical_id)?;
    merge_other_game_references(transaction, source_id, canonical_id)?;

    transaction
        .execute("DELETE FROM games WHERE id = ?1", params![source_id])
        .map_err(|error| EdenError::Persistence(error.to_string()))?;
    Ok(())
}

fn merge_singleton_game_table(
    transaction: &Transaction<'_>,
    table: &str,
    source_id: &str,
    canonical_id: &str,
) -> Result<(), EdenError> {
    if !table_exists(transaction, table)? {
        return Ok(());
    }
    let quoted_table = quote_identifier(table);
    let destination_exists: bool = transaction
        .query_row(
            &format!("SELECT EXISTS(SELECT 1 FROM {quoted_table} WHERE game_id = ?1)"),
            params![canonical_id],
            |row| row.get(0),
        )
        .map_err(|error| EdenError::Persistence(error.to_string()))?;
    if !destination_exists {
        transaction
            .execute(
                &format!("UPDATE {quoted_table} SET game_id = ?1 WHERE game_id = ?2"),
                params![canonical_id, source_id],
            )
            .map_err(|error| EdenError::Persistence(error.to_string()))?;
        return Ok(());
    }
    let columns = table_columns(transaction, table)?;
    let assignments = columns
        .iter()
        .filter(|column| column.as_str() != "game_id")
        .map(|column| {
            let quoted = quote_identifier(column);
            format!(
                "{quoted} = COALESCE(NULLIF({quoted}, ''), (SELECT NULLIF({quoted}, '') FROM {quoted_table} WHERE game_id = ?2))"
            )
        })
        .collect::<Vec<_>>();
    if !assignments.is_empty() {
        transaction
            .execute(
                &format!(
                    "UPDATE {quoted_table} SET {} WHERE game_id = ?1",
                    assignments.join(", ")
                ),
                params![canonical_id, source_id],
            )
            .map_err(|error| EdenError::Persistence(error.to_string()))?;
    }
    transaction
        .execute(
            &format!("DELETE FROM {quoted_table} WHERE game_id = ?1"),
            params![source_id],
        )
        .map_err(|error| EdenError::Persistence(error.to_string()))?;
    Ok(())
}

fn merge_artwork_selections(
    transaction: &Transaction<'_>,
    source_id: &str,
    canonical_id: &str,
) -> Result<(), EdenError> {
    if !table_exists(transaction, "game_artwork_selections")? {
        return Ok(());
    }
    transaction
        .execute(
            "INSERT INTO game_artwork_selections(
                game_id, slot, artwork_asset_id, selection_source, selected_at, updated_at
             )
             SELECT ?1, slot, artwork_asset_id, selection_source, selected_at, updated_at
             FROM game_artwork_selections WHERE game_id = ?2
             ON CONFLICT(game_id, slot) DO UPDATE SET
                artwork_asset_id = excluded.artwork_asset_id,
                selection_source = excluded.selection_source,
                selected_at = excluded.selected_at,
                updated_at = excluded.updated_at
             WHERE excluded.updated_at > game_artwork_selections.updated_at",
            params![canonical_id, source_id],
        )
        .map_err(|error| EdenError::Persistence(error.to_string()))?;
    transaction
        .execute(
            "DELETE FROM game_artwork_selections WHERE game_id = ?1",
            params![source_id],
        )
        .map_err(|error| EdenError::Persistence(error.to_string()))?;
    Ok(())
}

fn merge_external_playtime(
    transaction: &Transaction<'_>,
    source_id: &str,
    canonical_id: &str,
) -> Result<(), EdenError> {
    if !table_exists(transaction, "external_playtime_snapshots")? {
        return Ok(());
    }
    transaction
        .execute(
            "INSERT INTO external_playtime_snapshots(
                provider, emulator_installation_id, title_id, game_id,
                total_seconds, observed_at, format
             )
             SELECT provider, emulator_installation_id, title_id, ?1,
                    total_seconds, observed_at, format
             FROM external_playtime_snapshots WHERE game_id = ?2
             ON CONFLICT(provider, emulator_installation_id, title_id) DO UPDATE SET
                game_id = excluded.game_id,
                total_seconds = MAX(external_playtime_snapshots.total_seconds, excluded.total_seconds),
                observed_at = CASE WHEN excluded.observed_at > external_playtime_snapshots.observed_at
                                   THEN excluded.observed_at ELSE external_playtime_snapshots.observed_at END,
                format = CASE WHEN excluded.total_seconds >= external_playtime_snapshots.total_seconds
                              THEN excluded.format ELSE external_playtime_snapshots.format END",
            params![canonical_id, source_id],
        )
        .map_err(|error| EdenError::Persistence(error.to_string()))?;
    transaction
        .execute(
            "DELETE FROM external_playtime_snapshots WHERE game_id = ?1",
            params![source_id],
        )
        .map_err(|error| EdenError::Persistence(error.to_string()))?;
    Ok(())
}

fn merge_other_game_references(
    transaction: &Transaction<'_>,
    source_id: &str,
    canonical_id: &str,
) -> Result<(), EdenError> {
    let mut tables = transaction
        .prepare(
            "SELECT name FROM sqlite_master
             WHERE type = 'table' AND name NOT LIKE 'sqlite_%' AND name <> 'games'",
        )
        .map_err(|error| EdenError::Persistence(error.to_string()))?;
    let table_names = tables
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| EdenError::Persistence(error.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| EdenError::Persistence(error.to_string()))?;
    for table in table_names {
        if matches!(
            table.as_str(),
            "game_details"
                | "hltb_game_times"
                | "hltb_match_overrides"
                | "game_display_profiles"
                | "game_frame_generation_profiles"
                | "game_reviews_cache"
                | "game_review_consensus"
                | "steam_achievement_sync_state"
                | "game_artwork_selections"
                | "external_playtime_snapshots"
        ) || !table_columns(transaction, &table)?
            .iter()
            .any(|column| column == "game_id")
        {
            continue;
        }
        let quoted_table = quote_identifier(&table);
        transaction
            .execute(
                &format!("UPDATE OR IGNORE {quoted_table} SET game_id = ?1 WHERE game_id = ?2"),
                params![canonical_id, source_id],
            )
            .map_err(|error| EdenError::Persistence(error.to_string()))?;
        transaction
            .execute(
                &format!("DELETE FROM {quoted_table} WHERE game_id = ?1"),
                params![source_id],
            )
            .map_err(|error| EdenError::Persistence(error.to_string()))?;
    }
    Ok(())
}

fn table_exists(transaction: &Transaction<'_>, table: &str) -> Result<bool, EdenError> {
    transaction
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            params![table],
            |row| row.get(0),
        )
        .map_err(|error| EdenError::Persistence(error.to_string()))
}

fn table_columns(transaction: &Transaction<'_>, table: &str) -> Result<Vec<String>, EdenError> {
    let mut statement = transaction
        .prepare(&format!("PRAGMA table_info({})", quote_identifier(table)))
        .map_err(|error| EdenError::Persistence(error.to_string()))?;
    let result = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|error| EdenError::Persistence(error.to_string()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| EdenError::Persistence(error.to_string()));
    result
}

fn quote_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn record_identity_event(
    events: &mut IdentityEvents,
    checkpoint: &'static str,
    game_id: &str,
    title_id: Option<&str>,
    installation_id: &str,
) {
    events.push((
        checkpoint,
        format!(
            "gameId={game_id} titleId={} emulatorInstallationId={installation_id}",
            title_id.unwrap_or("none")
        ),
    ));
}

fn read_installation(state: &DatabaseState) -> Result<Option<EdenInstallation>, EdenError> {
    let connection = state
        .connection
        .lock()
        .map_err(|_| EdenError::Persistence("database lock poisoned".to_string()))?;
    let value: Option<String> = connection
        .query_row(
            "SELECT value_json FROM app_settings WHERE key = ?1",
            params![EDEN_INSTALLATION_KEY],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| EdenError::Persistence(error.to_string()))?;
    value
        .map(|json| {
            serde_json::from_str(&json).map_err(|error| EdenError::Persistence(error.to_string()))
        })
        .transpose()
}

fn write_installation(
    state: &DatabaseState,
    installation: &EdenInstallation,
) -> Result<(), EdenError> {
    let json = serde_json::to_string(installation)
        .map_err(|error| EdenError::Persistence(error.to_string()))?;
    let connection = state
        .connection
        .lock()
        .map_err(|_| EdenError::Persistence("database lock poisoned".to_string()))?;
    connection.execute("INSERT INTO app_settings(key, value_json, schema_version, updated_at) VALUES (?1, ?2, 1, ?3) ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at", params![EDEN_INSTALLATION_KEY, json, unix_timestamp()]).map_err(|error| EdenError::Persistence(error.to_string()))?;
    Ok(())
}

fn status_from_installation(
    installation: &EdenInstallation,
    state: &DatabaseState,
) -> Result<EdenStatus, EdenError> {
    let discovery = discover_installation(installation)?;
    let _ = state;
    status_from_discovery(&discovery)
}

fn status_from_discovery(discovery: &Discovery) -> Result<EdenStatus, EdenError> {
    let playtime_synced = discovery
        .games
        .iter()
        .filter(|game| game.title_id.is_some() && game.playtime_seconds.is_some())
        .count();
    let playtime_unavailable = discovery.games.len().saturating_sub(playtime_synced);
    let mut warnings = discovery.inspection.warnings.clone();
    if let Some(error) = discovery.playtime.parse_error.as_deref() {
        warnings.push(format!("No se pudo leer el playtime de Eden: {error}"));
    }
    Ok(EdenStatus {
        provider_id: EDEN_PROVIDER_ID.to_string(),
        status: if discovery.inspection.configuration_found
            || !discovery.inspection.library_roots.is_empty()
        {
            "ready".to_string()
        } else {
            "configuration-missing".to_string()
        },
        executable_path: Some(discovery.inspection.executable_path.clone()),
        data_path: discovery.inspection.data_path.clone(),
        config_path: discovery.inspection.config_path.clone(),
        portable: discovery.inspection.portable,
        configuration_found: discovery.inspection.configuration_found,
        library_roots: discovery.roots.clone(),
        profiles: discovery.inspection.profiles.clone(),
        games_detected: discovery.games.len(),
        duplicate_games: discovery.duplicate_games,
        playtime_synced,
        playtime_unavailable,
        playtime_file_found: discovery.playtime.file_found,
        warnings,
    })
}

fn build_title_map(data_path: Option<&str>) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let Some(data_path) = data_path else {
        return map;
    };
    for path in [
        PathBuf::from(data_path).join("log").join("eden_log.txt"),
        PathBuf::from(data_path)
            .join("log")
            .join("eden_log.txt.old.txt"),
    ] {
        let Ok(content) = fs::read_to_string(path) else {
            continue;
        };
        for line in content.lines() {
            let title_id = line.split_whitespace().find_map(title_id_from_text);
            let Some(title_id) = title_id else {
                continue;
            };
            if let Some((_, name)) = line.split_once("Loading ") {
                let clean = name.split('(').next().unwrap_or(name).trim();
                map.insert(normalize_name(clean), title_id.clone());
            } else if let Some((_, name)) = line.split_once("Booting game:") {
                let clean = name
                    .split('|')
                    .nth(1)
                    .unwrap_or(name)
                    .split('(')
                    .next()
                    .unwrap_or(name)
                    .trim();
                map.insert(normalize_name(clean), title_id.clone());
            }
        }
    }
    map
}

fn parse_playtime_bin(bytes: &[u8]) -> Result<Vec<EdenPlaytimeEntry>, String> {
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    if bytes.len() % EDEN_PLAYTIME_RECORD_SIZE != 0 {
        return Err(format!(
            "playtime.bin length {} is not aligned to {}-byte records",
            bytes.len(),
            EDEN_PLAYTIME_RECORD_SIZE
        ));
    }
    let mut entries = HashMap::<String, EdenPlaytimeEntry>::new();
    for record in bytes.chunks_exact(EDEN_PLAYTIME_RECORD_SIZE) {
        let title_id = format!(
            "{:016X}",
            u64::from_le_bytes(
                record[0..8]
                    .try_into()
                    .map_err(|_| "invalid title id".to_string())?
            )
        );
        let total_seconds = u64::from_le_bytes(
            record[8..16]
                .try_into()
                .map_err(|_| "invalid seconds".to_string())?,
        );
        if !is_title_id(&title_id) {
            continue;
        }
        let entry = EdenPlaytimeEntry {
            title_id: title_id.clone(),
            total_seconds,
        };
        entries
            .entry(title_id)
            .and_modify(|current| {
                if entry.total_seconds > current.total_seconds {
                    *current = entry.clone();
                }
            })
            .or_insert(entry);
    }
    Ok(entries.into_values().collect())
}

fn read_statistics(data_path: Option<&str>) -> EdenPlaytimeReadResult {
    let mut result = EdenPlaytimeReadResult::default();
    let Some(data_path) = data_path else {
        return result;
    };
    let root = PathBuf::from(data_path);
    let playtime_path = root.join("play_time").join("playtime.bin");
    match fs::read(&playtime_path) {
        Ok(bytes) => {
            result.file_found = true;
            match parse_playtime_bin(&bytes) {
                Ok(entries) => {
                    result.entries = entries
                        .into_iter()
                        .map(|entry| (entry.title_id.clone(), entry))
                        .collect();
                }
                Err(error) => result.parse_error = Some(error),
            }
        }
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            result.file_found = true;
            result.parse_error = Some(error.to_string());
        }
        Err(_) => {}
    }
    if let Ok(content) = fs::read_to_string(root.join("cache").join("launched.json")) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(entries) = value.as_object() {
                for (raw_id, entry) in entries {
                    let title_id = raw_id.to_uppercase();
                    if !is_title_id(&title_id) {
                        continue;
                    }
                    let raw_timestamp = entry
                        .get("timestamp")
                        .and_then(|value| value.as_i64())
                        .or_else(|| {
                            entry
                                .get("timestamp")
                                .and_then(|value| value.as_str())
                                .and_then(|value| value.parse().ok())
                        });
                    if let Some(timestamp) = raw_timestamp {
                        let seconds = if timestamp > 2_000_000_000_000 {
                            timestamp / 1000
                        } else {
                            timestamp
                        };
                        result.last_played_at.insert(title_id, seconds.to_string());
                    }
                }
            }
        }
    }
    result
}

fn title_id_from_path(path: &Path) -> Option<String> {
    title_id_from_text(&path.to_string_lossy())
}

fn title_id_from_text(text: &str) -> Option<String> {
    let upper = text.to_uppercase();
    let bytes = upper.as_bytes();
    for start in 0..bytes.len().saturating_sub(15) {
        let candidate = &bytes[start..start + 16];
        if let Ok(candidate) = std::str::from_utf8(candidate) {
            if is_title_id(candidate) {
                return Some(candidate.to_string());
            }
        }
    }
    None
}

fn is_title_id(value: &str) -> bool {
    value.len() == 16
        && value.starts_with(EDEN_TITLE_ID_PATTERN_PREFIX)
        && value.chars().all(|value| value.is_ascii_hexdigit())
}

fn supported_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| {
            SUPPORTED_EXTENSIONS
                .iter()
                .any(|extension| value.eq_ignore_ascii_case(extension))
        })
}

fn normalize_roots<I>(roots: I) -> Vec<EdenLibraryRoot>
where
    I: IntoIterator<Item = EdenLibraryRoot>,
{
    let mut seen = BTreeSet::new();
    roots
        .into_iter()
        .filter_map(|mut root| {
            root.path = normalize_path_text(&root.path);
            if root.path.is_empty()
                || is_virtual_eden_root(&root.path)
                || !seen.insert(normalize_path_key(Path::new(&root.path)))
            {
                None
            } else {
                Some(root)
            }
        })
        .collect()
}

fn normalize_path_text(value: &str) -> String {
    value
        .trim()
        .trim_matches('"')
        .replace('/', "\\")
        .replace("\\\\", "\\")
}

fn is_virtual_eden_root(path: &str) -> bool {
    matches!(
        path.trim_matches('\\').to_ascii_lowercase().as_str(),
        "sdmc" | "usernand" | "sysnand" | "nand"
    )
}

fn normalize_path_key(path: &Path) -> String {
    path.to_string_lossy().replace('/', "\\").to_lowercase()
}
fn is_within_root(path: &str, root: &str) -> bool {
    path == root
        || path
            .strip_prefix(root)
            .is_some_and(|rest| rest.starts_with('\\'))
}
fn normalize_name(value: &str) -> String {
    value
        .chars()
        .map(|char| {
            if char.is_ascii_alphanumeric() {
                char.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
fn parse_bool(value: &str) -> bool {
    matches!(
        value.to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}
fn unquote(value: &str) -> String {
    value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(value)
        .to_string()
}
fn game_id(identity: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(identity.as_bytes());
    format!("eden-{}", hex_string(&hasher.finalize())[..20].to_string())
}

fn eden_installation_id(installation: &EdenInstallation) -> String {
    let mut hasher = Sha256::new();
    hasher.update(normalize_path_text(&installation.executable_path).to_ascii_lowercase());
    hasher.update("|".as_bytes());
    hasher.update(
        installation
            .data_path
            .as_deref()
            .map(normalize_path_text)
            .unwrap_or_default()
            .to_ascii_lowercase(),
    );
    format!("eden-{}", hex_string(&hasher.finalize())[..20].to_string())
}
fn hex_string(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn unix_timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_multiple_gamedirs_and_deep_scan() {
        let dir = tempfile_dir("config");
        let config = dir.join("qt-config.ini");
        fs::write(
            &config,
            include_str!("../test-fixtures/eden/normal/qt-config.ini"),
        )
        .expect("config");
        let parsed = parse_config(&config).expect("parse");
        assert_eq!(parsed.roots.len(), 2);
        assert!(parsed.roots[0].deep_scan);
        assert_eq!(parsed.roots[1].path, "E:\\Nintendo");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn parses_flattened_qsettings_paths_and_ignores_virtual_duplicates() {
        let dir = tempfile_dir("flattened-config");
        let config = dir.join("qt-config.ini");
        fs::write(
            &config,
            include_str!("../test-fixtures/eden/flattened/qt-config.ini"),
        )
        .expect("config");
        let parsed = parse_config(&config).expect("parse");
        assert_eq!(parsed.roots.len(), 1);
        assert_eq!(
            parsed.roots[0].path,
            "G:\\LaunchBox\\Games\\Nintendo Switch"
        );
        assert!(!parsed.roots[0].deep_scan);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn reads_eden_profiles_without_touching_profile_data() {
        let dir = tempfile_dir("profiles");
        let avatar_dir = dir
            .join("nand")
            .join("system")
            .join("save")
            .join("8000000000000010")
            .join("su")
            .join("avators");
        fs::create_dir_all(&avatar_dir).expect("avatar dir");
        let profile_id = [
            0xc8, 0x6b, 0xad, 0x21, 0x50, 0x72, 0x52, 0x4a, 0xc2, 0xc0, 0x6e, 0xee, 0xa1, 0x0b,
            0x68, 0xf6,
        ];
        let mut profiles = vec![0_u8; EDEN_PROFILE_HEADER_SIZE + EDEN_PROFILE_RECORD_SIZE * 8];
        profiles[EDEN_PROFILE_HEADER_SIZE..EDEN_PROFILE_HEADER_SIZE + 16]
            .copy_from_slice(&profile_id);
        profiles[EDEN_PROFILE_HEADER_SIZE + 40..EDEN_PROFILE_HEADER_SIZE + 48]
            .copy_from_slice(b"rmndjsdz");
        fs::write(
            avatar_dir.join("c86bad21-5072-524a-c2c0-6eeea10b68f6.jpg"),
            [0xff, 0xd8, 0xff],
        )
        .expect("avatar");
        fs::write(avatar_dir.join("profiles.dat"), profiles).expect("profiles");

        let parsed = read_profiles(Some(&dir), Some(0));
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].name, "rmndjsdz");
        assert_eq!(parsed[0].id, "c86bad21-5072-524a-c2c0-6eeea10b68f6");
        assert!(parsed[0].is_current);
        assert!(parsed[0]
            .avatar_data_url
            .as_deref()
            .is_some_and(|value| value.starts_with("data:image/jpeg;base64,")));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn filters_extensions_case_insensitively_and_deduplicates_title_id() {
        let dir = tempfile_dir("games");
        fs::write(dir.join("Mario 0100000000010000.NSP"), b"rom").expect("rom");
        fs::write(dir.join("Mario copy 0100000000010000.xci"), b"rom").expect("rom");
        fs::write(dir.join("readme.zip"), b"not a rom").expect("file");
        let scan = scan_root(&EdenLibraryRoot {
            path: dir.to_string_lossy().into_owned(),
            deep_scan: false,
        });
        assert_eq!(scan.files.len(), 2);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn portable_discovery_prefers_user_directory_beside_executable() {
        let dir = tempfile_dir("portable");
        let executable = dir.join("eden.exe");
        fs::write(&executable, b"MZ Eden").expect("executable");
        let config = dir.join("user").join("config").join("qt-config.ini");
        fs::create_dir_all(config.parent().expect("config parent")).expect("config dir");
        fs::write(
            &config,
            include_str!("../test-fixtures/eden/portable/user/config/qt-config.ini"),
        )
        .expect("portable config");
        let inspection = inspect_executable(&executable.to_string_lossy()).expect("inspection");
        assert!(inspection.valid);
        assert!(inspection.portable);
        assert!(inspection.configuration_found);
        assert_eq!(inspection.library_roots.len(), 1);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_wrong_filename_and_non_pe_file() {
        let dir = tempfile_dir("validation");
        let wrong_name = dir.join("yuzu.exe");
        fs::write(&wrong_name, b"MZ").expect("wrong executable");
        assert!(matches!(
            inspect_executable(&wrong_name.to_string_lossy()),
            Err(EdenError::InvalidExecutable)
        ));
        let invalid = dir.join("eden.exe");
        fs::write(&invalid, b"not a PE").expect("invalid executable");
        assert!(matches!(
            inspect_executable(&invalid.to_string_lossy()),
            Err(EdenError::InvalidExecutable)
        ));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn reports_inaccessible_root_and_title_id_fallbacks() {
        let root = PathBuf::from("Z:\\Disconnected Eden Games");
        let scan = scan_root(&EdenLibraryRoot {
            path: root.to_string_lossy().into_owned(),
            deep_scan: true,
        });
        assert!(!scan.available);
        assert!(title_id_from_text("Mario 0100000000010000.xci").is_some());
        assert!(title_id_from_text("Mario Kart Deluxe.xci").is_none());
    }

    #[test]
    fn reads_optional_statistics_without_making_them_required() {
        let dir = tempfile_dir("stats");
        let playtime = dir.join("play_time");
        let cache = dir.join("cache");
        fs::create_dir_all(&playtime).expect("playtime dir");
        fs::create_dir_all(&cache).expect("cache dir");
        let title_id = "0100000000010000";
        let numeric = u64::from_str_radix(title_id, 16).expect("title id");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&numeric.to_le_bytes());
        bytes.extend_from_slice(&120_u64.to_le_bytes());
        fs::write(playtime.join("playtime.bin"), bytes).expect("playtime");
        fs::write(
            cache.join("launched.json"),
            format!(r#"{{"{title_id}":{{"timestamp":1700000000}}}}"#),
        )
        .expect("launch cache");
        let stats = read_statistics(Some(&dir.to_string_lossy()));
        assert!(stats.file_found);
        assert_eq!(
            stats.entries.get(title_id).map(|value| value.total_seconds),
            Some(120)
        );
        assert_eq!(
            stats.last_played_at.get(title_id).map(String::as_str),
            Some("1700000000")
        );
        let empty = read_statistics(Some(&dir.join("missing").to_string_lossy()));
        assert!(empty.entries.is_empty());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn parses_playtime_bin_as_little_endian_title_id_and_seconds_records() {
        let title_id = "0100000000010000";
        let numeric = u64::from_str_radix(title_id, 16).expect("title id");
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&numeric.to_le_bytes());
        bytes.extend_from_slice(&21_113_u64.to_le_bytes());
        let entries = parse_playtime_bin(&bytes).expect("playtime records");
        assert_eq!(
            entries,
            vec![EdenPlaytimeEntry {
                title_id: title_id.to_string(),
                total_seconds: 21_113,
            }]
        );
    }

    #[test]
    fn playtime_parser_handles_multiple_ids_and_rejects_corrupt_length() {
        let mut bytes = Vec::new();
        for (title_id, seconds) in [("0100000000010000", 120_u64), ("010015100B514000", 76_u64)] {
            let numeric = u64::from_str_radix(title_id, 16).expect("title id");
            bytes.extend_from_slice(&numeric.to_le_bytes());
            bytes.extend_from_slice(&seconds.to_le_bytes());
        }
        assert_eq!(parse_playtime_bin(&bytes).expect("records").len(), 2);
        assert!(parse_playtime_bin(&bytes[..15]).is_err());
        assert!(parse_playtime_bin(&[]).expect("empty file").is_empty());
    }

    #[test]
    fn deduplicates_games_by_title_id() {
        let dir = tempfile_dir("duplicates");
        let executable = dir.join("eden.exe");
        fs::write(&executable, b"MZ Eden").expect("executable");
        let games = dir.join("games");
        fs::create_dir_all(&games).expect("games dir");
        fs::write(games.join("Mario 0100000000010000.nsp"), b"rom").expect("rom");
        fs::write(games.join("Mario copy 0100000000010000.xci"), b"rom").expect("rom");
        let config = dir.join("user").join("config").join("qt-config.ini");
        fs::create_dir_all(config.parent().expect("config parent")).expect("config dir");
        fs::write(
            &config,
            format!(
                "[Paths]\ngamedirs\\size=1\ngamedirs\\1\\path={}\ngamedirs\\1\\deep_scan=false\n",
                games.to_string_lossy()
            ),
        )
        .expect("config");
        let installation = EdenInstallation {
            executable_path: executable.to_string_lossy().into_owned(),
            data_path: None,
            config_path: None,
            portable: true,
            library_roots: Vec::new(),
            manual_library_roots: Vec::new(),
        };
        let discovery = discover_installation(&installation).expect("discovery");
        assert_eq!(discovery.games.len(), 1);
        assert_eq!(discovery.duplicate_games, 1);
        assert!(discovery.games[0]
            .path
            .ends_with("Mario 0100000000010000.nsp"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn reconciles_path_only_game_to_title_id_and_preserves_related_data() {
        let dir = tempfile_dir("identity-reconciliation");
        let executable = dir.join("eden.exe");
        fs::write(&executable, b"MZ Eden").expect("executable");
        let games = dir.join("games");
        fs::create_dir_all(&games).expect("games dir");
        let rom = games.join("Mario Odyssey.nsp");
        fs::write(&rom, b"rom").expect("rom");
        let config = dir.join("user").join("config").join("qt-config.ini");
        fs::create_dir_all(config.parent().expect("config parent")).expect("config dir");
        fs::write(
            &config,
            format!(
                "[Paths]\ngamedirs\\size=1\ngamedirs\\1\\path={}\ngamedirs\\1\\deep_scan=false\n",
                games.to_string_lossy()
            ),
        )
        .expect("config");
        let database_root = dir.join("lumadeck-data");
        let state = crate::settings::initialize(
            crate::data_directory::DataDirectoryResolver::for_app_data(&database_root),
        )
        .expect("database");
        connect(&state, &executable.to_string_lossy(), &[]).expect("path-only connect");

        let installation = EdenInstallation {
            executable_path: executable.to_string_lossy().into_owned(),
            data_path: Some(dir.join("user").to_string_lossy().into_owned()),
            config_path: Some(config.to_string_lossy().into_owned()),
            portable: true,
            library_roots: vec![EdenLibraryRoot {
                path: games.to_string_lossy().into_owned(),
                deep_scan: false,
            }],
            manual_library_roots: Vec::new(),
        };
        let installation_id = eden_installation_id(&installation);
        let provisional_id = game_id(&format!(
            "eden:{installation_id}:path:{}",
            normalize_path_key(&rom)
        ));
        let title_id = "0100000000010000";
        let session_id =
            crate::settings::start_game_session(&state, &provisional_id).expect("session");
        {
            let connection = state.connection.lock().expect("database lock");
            connection
                .execute(
                    "UPDATE games SET last_played_at = '1700000100', favorite = 1, status = 'completed' WHERE id = ?1",
                    params![provisional_id],
                )
                .expect("last played");
            connection
                .execute(
                    "INSERT INTO game_details(game_id, steam_updated_at) VALUES (?1, '1700000000')",
                    params![provisional_id],
                )
                .expect("metadata");
            connection
                .execute(
                    "INSERT INTO artwork_assets(
                        source, external_asset_id, kind, width, height, source_mime_type,
                        cached_mime_type, cache_key, cached_path, checksum, byte_size,
                        downloaded_at, created_at, updated_at
                     ) VALUES ('steamgriddb', 1, 'grid', 600, 900, 'image/png', 'image/png',
                               'identity-art', 'artwork/identity.png', 'checksum', 10,
                               '1700000000', '1700000000', '1700000000')",
                    [],
                )
                .expect("artwork asset");
            let asset_id = connection.last_insert_rowid();
            connection
                .execute(
                    "INSERT INTO game_artwork_selections(
                        game_id, slot, artwork_asset_id, selection_source,
                        selected_at, updated_at
                     ) VALUES (?1, 'grid_vertical', ?2, 'steamgriddb', '1700000000', '1700000000')",
                    params![provisional_id, asset_id],
                )
                .expect("artwork selection");
            connection
                .execute(
                    "INSERT INTO external_playtime_snapshots(
                        provider, emulator_installation_id, title_id, game_id,
                        total_seconds, observed_at, format
                     ) VALUES ('eden', ?1, ?2, ?3, 3600, '1700000200', 'test')",
                    params![installation_id, title_id, provisional_id],
                )
                .expect("external playtime");
        }

        let log = dir.join("user").join("log").join("eden_log.txt");
        fs::create_dir_all(log.parent().expect("log parent")).expect("log dir");
        fs::write(&log, "Loading Mario Odyssey (0100000000010000)\n").expect("eden log");

        let first_rescan = rescan(&state).expect("title-id rescan");
        assert_eq!(first_rescan.games_detected, 1);
        let second_rescan = rescan(&state).expect("idempotent rescan");
        assert_eq!(second_rescan.games_detected, 1);

        let canonical_id = canonical_title_game_id(&installation_id, title_id);
        let connection = state.connection.lock().expect("database lock");
        let game_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM games WHERE source = 'emulator' AND emulator_id = 'eden'",
                [],
                |row| row.get(0),
            )
            .expect("game count");
        assert_eq!(game_count, 1);
        let (stored_id, stored_title_id, stored_path, stored_last_played, favorite, status):
            (String, String, String, String, i64, String) =
            connection
                .query_row(
                    "SELECT id, title_id, game_path, last_played_at, favorite, status FROM games WHERE source = 'emulator' AND emulator_id = 'eden'",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
                )
                .expect("canonical game");
        assert_eq!(stored_id, canonical_id);
        assert_eq!(stored_title_id, title_id);
        assert_eq!(stored_path, rom.to_string_lossy());
        assert_eq!(stored_last_played, "1700000100");
        assert_eq!(favorite, 1);
        assert_eq!(status, "completed");
        let session_game_id: String = connection
            .query_row(
                "SELECT game_id FROM game_sessions WHERE id = ?1",
                params![session_id],
                |row| row.get(0),
            )
            .expect("session game");
        assert_eq!(session_game_id, canonical_id);
        let snapshot: (String, i64) = connection
            .query_row(
                "SELECT game_id, total_seconds FROM external_playtime_snapshots WHERE title_id = ?1",
                params![title_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("snapshot");
        assert_eq!(snapshot, (canonical_id.clone(), 3600));
        let metadata_exists: i64 = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM game_details WHERE game_id = ?1)",
                params![canonical_id],
                |row| row.get(0),
            )
            .expect("metadata");
        assert_eq!(metadata_exists, 1);
        let artwork_game_id: String = connection
            .query_row(
                "SELECT game_id FROM game_artwork_selections WHERE slot = 'grid_vertical'",
                [],
                |row| row.get(0),
            )
            .expect("artwork");
        assert_eq!(artwork_game_id, canonical_id);
        drop(connection);
        let local_games = crate::settings::get_local_games(&state).expect("local games");
        let canonical = local_games
            .iter()
            .find(|game| game.id == canonical_id)
            .expect("canonical local game");
        assert_eq!(canonical.playtime_minutes, 60);

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn keeps_canonical_id_when_title_id_path_changes() {
        let dir = tempfile_dir("identity-path-change");
        let executable = dir.join("eden.exe");
        fs::write(&executable, b"MZ Eden").expect("executable");
        let games = dir.join("games");
        fs::create_dir_all(&games).expect("games dir");
        let original = games.join("Mario 0100000000010000.nsp");
        fs::write(&original, b"rom").expect("rom");
        let config = dir.join("user").join("config").join("qt-config.ini");
        fs::create_dir_all(config.parent().expect("config parent")).expect("config dir");
        let write_config = |deep_scan: bool| {
            fs::write(
                &config,
                format!(
                    "[Paths]\ngamedirs\\size=1\ngamedirs\\1\\path={}\ngamedirs\\1\\deep_scan={}\n",
                    games.to_string_lossy(),
                    deep_scan
                ),
            )
            .expect("config");
        };
        write_config(false);
        let database_root = dir.join("lumadeck-data");
        let state = crate::settings::initialize(
            crate::data_directory::DataDirectoryResolver::for_app_data(&database_root),
        )
        .expect("database");
        connect(&state, &executable.to_string_lossy(), &[]).expect("connect");
        let installation = EdenInstallation {
            executable_path: executable.to_string_lossy().into_owned(),
            data_path: Some(dir.join("user").to_string_lossy().into_owned()),
            config_path: Some(config.to_string_lossy().into_owned()),
            portable: true,
            library_roots: vec![],
            manual_library_roots: Vec::new(),
        };
        let installation_id = eden_installation_id(&installation);
        let canonical_id = canonical_title_game_id(&installation_id, "0100000000010000");
        let moved_dir = games.join("moved");
        fs::create_dir_all(&moved_dir).expect("moved dir");
        let moved = moved_dir.join("Mario 0100000000010000.nsp");
        fs::rename(&original, &moved).expect("move rom");
        write_config(true);
        rescan(&state).expect("path change rescan");
        rescan(&state).expect("repeated path change rescan");
        let connection = state.connection.lock().expect("database lock");
        let row: (i64, String, String) = connection
            .query_row(
                "SELECT COUNT(*), id, game_path FROM games WHERE source = 'emulator' AND emulator_id = 'eden'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("path change row");
        assert_eq!(row.0, 1);
        assert_eq!(row.1, canonical_id);
        assert_eq!(row.2, moved.to_string_lossy());
        drop(connection);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn reconciliation_rolls_back_with_the_outer_transaction() {
        let dir = tempfile_dir("identity-rollback");
        let database_root = dir.join("lumadeck-data");
        let state = crate::settings::initialize(
            crate::data_directory::DataDirectoryResolver::for_app_data(&database_root),
        )
        .expect("database");
        let installation_id = "rollback-installation";
        let source_id = "eden-source-rollback";
        let canonical_id = "eden-canonical-rollback";
        {
            let connection = state.connection.lock().expect("database lock");
            connection
                .execute(
                    "INSERT INTO games(id, title, sort_title, provider, platform, source, emulator_id, game_path, title_id, created_at, updated_at)
                     VALUES (?1, 'Mario', 'mario', 'Eden', 'Nintendo Switch', 'emulator', 'eden', 'D:\\\\Mario.nsp', ?2, '1700000000', '1700000000')",
                    params![source_id, "0100000000010000"],
                )
                .expect("source game");
            connection
                .execute(
                    "INSERT INTO games(id, title, sort_title, provider, platform, source, emulator_id, created_at, updated_at)
                     VALUES (?1, 'Mario', 'mario', 'Eden', 'Nintendo Switch', 'emulator', 'eden', '1700000000', '1700000000')",
                    params![canonical_id],
                )
                .expect("canonical game");
            let transaction = connection.unchecked_transaction().expect("transaction");
            reconcile_title_id_records(
                &transaction,
                installation_id,
                "0100000000010000",
                "D:\\Mario.nsp",
                "Mario",
                canonical_id,
                &mut Vec::new(),
            )
            .expect("reconcile");
            assert!(transaction
                .execute(
                    "INSERT INTO game_sessions(game_id, started_at, status, source, created_at, updated_at)
                     VALUES ('rollback-failure', '1700000000', 'active', 'lumadeck', '1700000000', '1700000000')",
                    [],
                )
                .is_err());
            drop(transaction);
        }
        let connection = state.connection.lock().expect("database lock");
        let source_exists: i64 = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM games WHERE id = ?1)",
                params![source_id],
                |row| row.get(0),
            )
            .expect("source exists");
        assert_eq!(source_exists, 1);
        let canonical_has_session: i64 = connection
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM games WHERE id = ?1 AND title_id IS NULL)",
                params![canonical_id],
                |row| row.get(0),
            )
            .expect("canonical rollback");
        assert_eq!(canonical_has_session, 1);
        drop(connection);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rescan_marks_removed_games_but_preserves_disconnected_root_data() {
        let dir = tempfile_dir("rescan");
        let executable = dir.join("eden.exe");
        fs::write(&executable, b"MZ Eden").expect("executable");
        let games = dir.join("games");
        fs::create_dir_all(&games).expect("games dir");
        let rom = games.join("Mario 0100000000010000.nsp");
        fs::write(&rom, b"rom").expect("rom");
        let config = dir.join("user").join("config").join("qt-config.ini");
        fs::create_dir_all(config.parent().expect("config parent")).expect("config dir");
        fs::write(
            &config,
            format!(
                "[Paths]\ngamedirs\\size=1\ngamedirs\\1\\path={}\ngamedirs\\1\\deep_scan=false\n",
                games.to_string_lossy()
            ),
        )
        .expect("config");
        let database_root = dir.join("lumadeck-data");
        let state = crate::settings::initialize(
            crate::data_directory::DataDirectoryResolver::for_app_data(&database_root),
        )
        .expect("database");
        let initial = connect(&state, &executable.to_string_lossy(), &[]).expect("connect");
        assert_eq!(initial.games_detected, 1);
        fs::remove_file(&rom).expect("remove rom");
        let missing = rescan(&state).expect("rescan missing");
        assert!(missing.library_roots[0].available);
        let installed_after_missing: i64 = state
            .connection
            .lock()
            .expect("database lock")
            .query_row(
                "SELECT installed FROM games WHERE source = 'emulator' AND emulator_id = 'eden'",
                [],
                |row| row.get(0),
            )
            .expect("game row");
        assert_eq!(installed_after_missing, 0);
        fs::write(&rom, b"rom").expect("restore rom");
        let present = rescan(&state).expect("rescan restored");
        assert_eq!(present.games_detected, 1);
        fs::remove_dir_all(&games).expect("disconnect root");
        let disconnected = rescan(&state).expect("rescan disconnected");
        assert!(!disconnected.library_roots[0].available);
        let installed_after_disconnect: i64 = state
            .connection
            .lock()
            .expect("database lock")
            .query_row(
                "SELECT installed FROM games WHERE source = 'emulator' AND emulator_id = 'eden'",
                [],
                |row| row.get(0),
            )
            .expect("game row");
        assert_eq!(installed_after_disconnect, 1);
        let _ = fs::remove_dir_all(dir);
    }

    fn tempfile_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("lumadeck-eden-{name}-{}", unix_timestamp()));
        fs::create_dir_all(&path).expect("temp dir");
        path
    }
}
