use crate::settings::{
    DatabaseError, DatabaseState, LaunchBoxCatalogPhase, LaunchBoxCatalogProgress,
    LocalLaunchBoxDetails,
};
use futures_util::StreamExt;
use quick_xml::{escape::unescape, events::Event, Reader};
use reqwest::{
    header::{HeaderMap, HeaderValue},
    Client,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs::{self, File},
    io::{BufReader, BufWriter, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use zip::ZipArchive;

pub const LAUNCHBOX_METADATA_ZIP_URL: &str = "https://gamesdb.launchbox-app.com/Metadata.zip";
pub const LAUNCHBOX_CATALOG_SCHEMA_VERSION: i64 = 2;
pub const LAUNCHBOX_CATALOG_TTL: Duration = Duration::from_secs(30 * 24 * 60 * 60);
const MAX_SCREENSHOTS_PER_GAME: usize = 12;

fn lock_connection<'a>(
    state: &'a DatabaseState,
    operation: &str,
) -> Result<std::sync::MutexGuard<'a, Connection>, String> {
    let connection = state
        .connection
        .lock()
        .map_err(|_| "LAUNCHBOX_DATABASE_LOCK".to_string())?;
    if state.take_connection_poisoned() {
        state.log(
            "launchbox-database",
            "launchbox_database_mutex_poisoned",
            &format!("operation={operation}"),
        );
        state.log(
            "launchbox-database",
            "launchbox_database_mutex_recovered",
            &format!("operation={operation}"),
        );
    }
    Ok(connection)
}

fn catalog_runtime_snapshot(
    state: &DatabaseState,
) -> (
    LaunchBoxCatalogPhase,
    Option<String>,
    Option<String>,
    Option<LaunchBoxCatalogProgress>,
) {
    let runtime = state
        .launchbox_catalog_runtime
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let progress = runtime.progress.clone().map(|mut progress| {
        progress.elapsed_ms = now_millis().saturating_sub(progress.started_at_ms);
        progress
    });
    (
        runtime.phase,
        runtime.active_version.clone(),
        runtime.last_error.clone(),
        progress,
    )
}

fn begin_catalog_update(state: &DatabaseState) -> Result<(), String> {
    let mut runtime = state
        .launchbox_catalog_runtime
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if runtime.phase == LaunchBoxCatalogPhase::Updating {
        return Err("LAUNCHBOX_CATALOG_UPDATE_IN_PROGRESS".to_string());
    }
    runtime.phase = LaunchBoxCatalogPhase::Updating;
    runtime.last_error = None;
    runtime.progress = Some(LaunchBoxCatalogProgress::new("downloading"));
    state.log(
        "launchbox-catalog",
        "launchbox_catalog_state_changed",
        &format!(
            "state=updating active_version={}",
            runtime.active_version.as_deref().unwrap_or("<none>")
        ),
    );
    Ok(())
}

fn update_catalog_progress(
    state: &DatabaseState,
    phase: &str,
    processed_records: Option<i64>,
    downloaded_bytes: Option<i64>,
    total_bytes: Option<i64>,
) {
    let mut runtime = state
        .launchbox_catalog_runtime
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    let progress = runtime
        .progress
        .get_or_insert_with(|| LaunchBoxCatalogProgress::new(phase));
    let now = now_millis();
    progress.phase = phase.to_string();
    if let Some(processed_records) = processed_records {
        progress.processed_records = processed_records;
    }
    if let Some(downloaded_bytes) = downloaded_bytes {
        progress.downloaded_bytes = Some(downloaded_bytes);
    }
    if let Some(total_bytes) = total_bytes {
        progress.total_bytes = Some(total_bytes);
    }
    progress.elapsed_ms = now.saturating_sub(progress.started_at_ms);
    progress.last_progress_at_ms = now;
}

fn finish_catalog_update_success(state: &DatabaseState, version: Option<String>) {
    let mut runtime = state
        .launchbox_catalog_runtime
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    runtime.phase = LaunchBoxCatalogPhase::Ready;
    runtime.active_version = version;
    runtime.last_error = None;
    runtime.progress = None;
    state.log(
        "launchbox-catalog",
        "launchbox_catalog_state_changed",
        "state=ready",
    );
}

fn finish_catalog_update_error(state: &DatabaseState, error: &str) -> bool {
    let mut runtime = state
        .launchbox_catalog_runtime
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let has_previous = runtime.active_version.is_some();
    runtime.phase = LaunchBoxCatalogPhase::Error;
    runtime.last_error = Some(error.to_string());
    state.log(
        "launchbox-catalog",
        "launchbox_catalog_state_changed",
        &format!(
            "state=error active_version={} fallback={has_previous}",
            runtime.active_version.as_deref().unwrap_or("<none>")
        ),
    );
    has_previous
}

#[derive(Debug, Error)]
pub enum LaunchBoxError {
    #[error("launchbox catalog download failed")]
    Download(#[from] reqwest::Error),
    #[error("launchbox catalog archive is empty or invalid")]
    InvalidZip(#[from] zip::result::ZipError),
    #[error("launchbox catalog did not contain Metadata.xml or Metadata.json")]
    MetadataMissing,
    #[error("launchbox catalog could not be read")]
    Io(#[from] std::io::Error),
    #[error("launchbox catalog XML is malformed")]
    Xml(#[from] quick_xml::Error),
    #[error("launchbox catalog JSON is malformed")]
    Json(#[from] serde_json::Error),
    #[error("launchbox catalog validation failed: {0}")]
    Validation(String),
    #[error("launchbox catalog database operation failed")]
    Database(#[from] rusqlite::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LaunchBoxMediaReference {
    pub provider_media_id: Option<String>,
    pub media_type: String,
    pub url: String,
    pub ordinal: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LaunchBoxRecord {
    pub provider_game_id: String,
    pub canonical_title: String,
    pub normalized_title: String,
    pub alternate_titles: Vec<String>,
    pub platform: String,
    pub normalized_platform: String,
    pub description: Option<String>,
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub release_date: Option<String>,
    pub genres: Vec<String>,
    pub normalized_genres: Vec<String>,
    pub local_multiplayer: String,
    pub max_local_players: Option<i64>,
    pub community_rating_raw: Option<f64>,
    pub community_rating_scale: Option<f64>,
    pub community_rating_count: Option<i64>,
    pub community_rating_raw_text: Option<String>,
    pub media: Vec<LaunchBoxMediaReference>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchBoxCatalogStatus {
    pub available: bool,
    pub catalog_version: Option<String>,
    pub catalog_schema_version: Option<i64>,
    pub metadata_zip_url: String,
    pub downloaded_at: Option<String>,
    pub expires_at: Option<String>,
    pub record_count: i64,
    pub switch_record_count: i64,
    pub zip_size_bytes: Option<i64>,
    pub source_size_bytes: Option<i64>,
    pub import_duration_ms: Option<i64>,
    pub status: String,
    pub last_error: Option<String>,
    pub ttl_expired: bool,
    pub progress: Option<LaunchBoxCatalogProgress>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchBoxEnrichmentResult {
    pub resolved: i64,
    pub exact: i64,
    pub high: i64,
    pub ambiguous: i64,
    pub unresolved: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchBoxGameRefreshResult {
    pub status: String,
    pub metadata_resolved: bool,
    pub screenshots_resolved: i64,
    pub screenshots_cached: i64,
    pub screenshots_downloaded: i64,
    pub screenshots_failed: i64,
    pub confidence: String,
}

#[derive(Debug, Clone)]
pub struct LaunchBoxScreenshotReport {
    pub paths: Vec<String>,
    pub cached: i64,
    pub downloaded: i64,
    pub failed: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MatchConfidence {
    Exact,
    High,
    Ambiguous,
    Unresolved,
}

impl MatchConfidence {
    fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::High => "high",
            Self::Ambiguous => "ambiguous",
            Self::Unresolved => "unresolved",
        }
    }
}

#[derive(Debug, Clone)]
struct MatchCandidate {
    provider_game_id: String,
    confidence: MatchConfidence,
}

pub fn normalize_title(value: &str) -> String {
    let folded = value
        .trim()
        .chars()
        .flat_map(|character| character.to_lowercase())
        .collect::<String>();
    let mut output = String::with_capacity(folded.len());
    let mut previous_space = false;
    for character in folded.chars() {
        let normalized = match character {
            '’' | '`' | '´' => '\'',
            '–' | '—' | '−' => '-',
            _ => character,
        };
        if normalized.is_alphanumeric() {
            output.push(normalized);
            previous_space = false;
        } else if !previous_space {
            output.push(' ');
            previous_space = true;
        }
    }
    output.trim().to_string()
}

pub fn normalize_platform(value: &str) -> String {
    let normalized = normalize_title(value);
    let mappings = [
        ("nintendo switch", "nintendo_switch"),
        ("nintendo gamecube", "gamecube"),
        ("gamecube", "gamecube"),
        ("nintendo wii u", "wii_u"),
        ("wii u", "wii_u"),
        ("nintendo wii", "wii"),
        ("sony playstation 3", "ps3"),
        ("playstation 3", "ps3"),
        ("sony playstation 4", "ps4"),
        ("playstation 4", "ps4"),
        ("sony playstation 5", "ps5"),
        ("playstation 5", "ps5"),
    ];
    mappings
        .iter()
        .find_map(|(source, target)| normalized.contains(source).then_some((*target).to_string()))
        .unwrap_or_else(|| normalized.replace(' ', "_"))
}

pub fn normalize_genre(value: &str) -> Option<String> {
    let compact = normalize_title(value).replace(' ', "");
    let normalized = match compact.as_str() {
        "fighting" => "Fighting",
        "beatemup" | "beat'emup" => "BeatEmUp",
        "sports" => "Sports",
        "racing" => "Racing",
        "party" => "Party",
        "action" => "Action",
        "adventure" => "Adventure",
        "roleplaying" | "rpg" => "RPG",
        "platform" | "platformer" => "Platformer",
        "puzzle" => "Puzzle",
        "strategy" => "Strategy",
        "simulation" | "constructionsandmanagementsimulation" => "Simulation",
        _ => return None,
    };
    Some(normalized.to_string())
}

fn normalize_genre_values(value: &str) -> Vec<String> {
    if let Some(genre) = normalize_genre(value) {
        return vec![genre];
    }
    let compact = normalize_title(value).replace(' ', "");
    [
        ("fighting", "Fighting"),
        ("beatemup", "BeatEmUp"),
        ("sports", "Sports"),
        ("racing", "Racing"),
        ("party", "Party"),
        ("action", "Action"),
        ("adventure", "Adventure"),
        ("roleplaying", "RPG"),
        ("platform", "Platformer"),
        ("puzzle", "Puzzle"),
        ("strategy", "Strategy"),
        ("simulation", "Simulation"),
    ]
    .iter()
    .filter_map(|(needle, genre)| compact.contains(needle).then_some((*genre).to_string()))
    .collect()
}

fn split_values(value: &str) -> Vec<String> {
    value
        .split([',', ';', '|'])
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn normalize_record(raw: RawRecord) -> Option<LaunchBoxRecord> {
    let local_multiplayer = parse_local_multiplayer(&raw);
    let max_players = parse_local_player_count(&raw)
        .or_else(|| {
            if local_multiplayer == "true" {
                raw.max_players.as_deref().and_then(|value| {
                    value
                        .split(|character: char| !character.is_ascii_digit())
                        .find_map(|part| part.parse::<i64>().ok())
                })
            } else {
                None
            }
        })
        .filter(|value| *value > 0);
    let title = clean_text(raw.title?);
    let platform = clean_text(raw.platform?);
    let genres = raw
        .genres
        .into_iter()
        .flat_map(|value| split_values(&value))
        .collect::<Vec<_>>();
    let normalized_genres = genres
        .iter()
        .flat_map(|genre| normalize_genre_values(genre))
        .collect::<Vec<_>>();
    let (rating, scale, rating_text) = parse_rating(raw.community_rating.as_deref());
    let mut alternate_titles = raw
        .alternate_titles
        .into_iter()
        .flat_map(|value| split_values(&value))
        .filter(|value| normalize_title(value) != normalize_title(&title))
        .collect::<Vec<_>>();
    alternate_titles.sort_by_key(|value| normalize_title(value));
    alternate_titles.dedup_by(|left, right| normalize_title(left) == normalize_title(right));
    let mut media_urls = HashSet::new();
    let media = raw
        .media
        .into_iter()
        .filter_map(normalize_media)
        .filter(|media| media_urls.insert(media.url.clone()))
        .take(MAX_SCREENSHOTS_PER_GAME.saturating_mul(4))
        .collect::<Vec<_>>();
    let developer = raw.developer.and_then(|value| non_empty(clean_text(value)));
    let publisher = raw.publisher.and_then(|value| non_empty(clean_text(value)));
    Some(LaunchBoxRecord {
        provider_game_id: clean_text(raw.provider_game_id.unwrap_or_default()),
        canonical_title: title.clone(),
        normalized_title: normalize_title(&title),
        alternate_titles,
        platform: platform.clone(),
        normalized_platform: normalize_platform(&platform),
        description: raw
            .description
            .and_then(|value| non_empty(clean_text(value))),
        developer: developer.map(clean_text),
        publisher: publisher.map(clean_text),
        release_date: raw
            .release_date
            .and_then(|value| non_empty(clean_text(value))),
        genres,
        normalized_genres,
        local_multiplayer,
        max_local_players: max_players,
        community_rating_raw: rating,
        community_rating_scale: scale,
        community_rating_count: raw.rating_count.as_deref().and_then(parse_integer),
        community_rating_raw_text: rating_text,
        media,
    })
}

fn normalize_media(raw: RawMedia) -> Option<LaunchBoxMediaReference> {
    let url = normalize_media_url(&raw.url?);
    if url.is_empty() {
        return None;
    }
    Some(LaunchBoxMediaReference {
        provider_media_id: raw.id.and_then(non_empty),
        media_type: normalize_media_type(&raw.media_type),
        url,
        ordinal: raw.ordinal,
    })
}

fn normalize_media_type(value: &str) -> String {
    let normalized = normalize_title(value);
    if normalized.contains("screenshot") || normalized.contains("gameplay") {
        "screenshot".to_string()
    } else if normalized.contains("box front") || normalized == "front" {
        "box_front".to_string()
    } else if normalized.contains("box back") {
        "box_back".to_string()
    } else if normalized.contains("clear logo") || normalized == "logo" {
        "clear_logo".to_string()
    } else if normalized.contains("fanart") || normalized.contains("background") {
        "fanart".to_string()
    } else if normalized.contains("banner") {
        "banner".to_string()
    } else {
        "other".to_string()
    }
}

fn normalize_media_url(value: &str) -> String {
    let value = clean_text(value.to_string()).replace('\\', "/");
    if value.is_empty() {
        return String::new();
    }
    if value.starts_with("http://") || value.starts_with("https://") {
        return value;
    }
    if value.starts_with("//") {
        return format!("https:{value}");
    }
    format!("https://images.launchbox-app.com/{value}")
}

fn parse_local_multiplayer(raw: &RawRecord) -> String {
    if raw.description.as_deref().is_some_and(|value| {
        let normalized = normalize_title(value);
        normalized.contains("local multiplayer")
            || normalized.contains("play locally")
            || (normalized.contains("local") && normalized.contains("multiplayer"))
    }) {
        return "true".to_string();
    }
    let explicit = [raw.multiplayer.as_deref(), raw.cooperative.as_deref()]
        .into_iter()
        .flatten()
        .map(normalize_title)
        .find(|value| !value.is_empty());
    match explicit.as_deref() {
        Some("true" | "yes" | "y" | "1" | "local multiplayer" | "local coop" | "cooperative") => {
            "true".to_string()
        }
        Some("false" | "no" | "n" | "0" | "none" | "single player") => "false".to_string(),
        Some(value)
            if value.contains("local")
                || value.contains("co op")
                || value.contains("cooperative") =>
        {
            "true".to_string()
        }
        Some(_) => "unknown".to_string(),
        None => "unknown".to_string(),
    }
}

/// Returns at most `max_chars` Unicode scalar values without splitting UTF-8.
/// The limit is measured in characters, not encoded bytes.
fn truncate_unicode(value: &str, max_chars: usize) -> &str {
    value
        .char_indices()
        .nth(max_chars)
        .map(|(byte_index, _)| &value[..byte_index])
        .unwrap_or(value)
}

fn parse_local_player_count(raw: &RawRecord) -> Option<i64> {
    let description = raw.description.as_deref()?.to_ascii_lowercase();
    let local_index = description.find("local")?;
    let nearby = truncate_unicode(&description[local_index..], 120);
    nearby
        .split(|character: char| !character.is_ascii_digit())
        .find_map(|part| part.parse::<i64>().ok())
        .filter(|value| *value > 0 && *value <= 32)
}

fn parse_rating(value: Option<&str>) -> (Option<f64>, Option<f64>, Option<String>) {
    let raw = value.map(str::trim).filter(|item| !item.is_empty());
    let Some(raw) = raw else {
        return (None, None, None);
    };
    let score = raw
        .split(|character: char| !character.is_ascii_digit() && character != '.')
        .find_map(|part| part.parse::<f64>().ok())
        .filter(|value| value.is_finite() && *value >= 0.0);
    let scale = raw
        .split_once('/')
        .and_then(|(_, right)| right.split_whitespace().next())
        .and_then(|value| value.parse::<f64>().ok())
        .or_else(|| score.filter(|value| *value <= 5.0).map(|_| 5.0))
        .or_else(|| score.filter(|value| *value <= 100.0).map(|_| 100.0));
    (score, scale, Some(raw.to_string()))
}

fn parse_integer(value: &str) -> Option<i64> {
    value
        .split(|character: char| !character.is_ascii_digit())
        .find_map(|part| part.parse::<i64>().ok())
        .filter(|number| *number > 0)
}

fn clean_text(value: String) -> String {
    let unescaped = unescape(&value)
        .map(|item| item.into_owned())
        .unwrap_or(value);
    unescaped.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn non_empty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

#[derive(Default)]
struct RawRecord {
    provider_game_id: Option<String>,
    title: Option<String>,
    alternate_titles: Vec<String>,
    platform: Option<String>,
    description: Option<String>,
    developer: Option<String>,
    publisher: Option<String>,
    release_date: Option<String>,
    genres: Vec<String>,
    multiplayer: Option<String>,
    cooperative: Option<String>,
    max_players: Option<String>,
    community_rating: Option<String>,
    rating_count: Option<String>,
    media: Vec<RawMedia>,
}

#[derive(Default)]
struct RawMedia {
    id: Option<String>,
    media_type: String,
    url: Option<String>,
    ordinal: i64,
}

pub fn parse_metadata_xml<
    R: std::io::BufRead,
    F: FnMut(LaunchBoxRecord) -> Result<(), LaunchBoxError>,
>(
    source: R,
    on_record: F,
) -> Result<usize, LaunchBoxError> {
    parse_metadata_xml_with_media(source, on_record, |_provider_game_id, _media| Ok(()))
}

fn parse_metadata_xml_with_media<
    R: std::io::BufRead,
    F: FnMut(LaunchBoxRecord) -> Result<(), LaunchBoxError>,
    M: FnMut(String, LaunchBoxMediaReference) -> Result<(), LaunchBoxError>,
>(
    source: R,
    mut on_record: F,
    mut on_media: M,
) -> Result<usize, LaunchBoxError> {
    let mut reader = Reader::from_reader(source);
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut count = 0;
    let mut global_media_ordinal = 0;
    loop {
        match reader.read_event_into(&mut buffer)? {
            Event::Start(event) if event.local_name().as_ref().eq_ignore_ascii_case(b"Game") => {
                if let Some(record) = parse_game_element(&mut reader)? {
                    on_record(record)?;
                    count += 1;
                }
            }
            Event::Start(event) if is_global_media_element(event.local_name().as_ref()) => {
                let ordinal = global_media_ordinal;
                global_media_ordinal += 1;
                if let Some((provider_game_id, media)) =
                    parse_global_media_element(&mut reader, event.local_name().as_ref(), ordinal)?
                {
                    on_media(provider_game_id, media)?;
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    Ok(count)
}

fn is_global_media_element(name: &[u8]) -> bool {
    name.eq_ignore_ascii_case(b"GameImage")
        || name.eq_ignore_ascii_case(b"Image")
        || name.eq_ignore_ascii_case(b"Media")
}

fn parse_global_media_element<R: std::io::BufRead>(
    reader: &mut Reader<R>,
    element_name: &[u8],
    ordinal: i64,
) -> Result<Option<(String, LaunchBoxMediaReference)>, LaunchBoxError> {
    let mut buffer = Vec::new();
    let mut stack: Vec<Vec<u8>> = Vec::new();
    let mut text_stack: Vec<String> = Vec::new();
    let mut provider_game_id = None;
    let mut media_type = String::new();
    let mut url = None;
    loop {
        match reader.read_event_into(&mut buffer)? {
            Event::Start(event) => {
                stack.push(event.local_name().as_ref().to_ascii_lowercase());
                text_stack.push(String::new());
            }
            Event::Text(event) => {
                if let Some(text) = text_stack.last_mut() {
                    text.push_str(&clean_text(
                        String::from_utf8_lossy(event.as_ref()).into_owned(),
                    ));
                }
            }
            Event::CData(event) => {
                if let Some(text) = text_stack.last_mut() {
                    text.push_str(&clean_text(
                        String::from_utf8_lossy(event.as_ref()).into_owned(),
                    ));
                }
            }
            Event::End(event) => {
                let name = event.local_name().as_ref().to_ascii_lowercase();
                let text = text_stack.pop().unwrap_or_default();
                stack.pop();
                if name.eq_ignore_ascii_case(element_name) {
                    break;
                }
                match name.as_slice() {
                    b"databaseid" | b"databasegameid" | b"gameid" => {
                        provider_game_id = non_empty(text)
                    }
                    b"type" | b"imagetype" | b"category" => media_type = text,
                    b"url" | b"imageurl" | b"path" | b"filename" | b"file" => url = non_empty(text),
                    _ => {}
                }
            }
            Event::Eof => {
                return Err(LaunchBoxError::Validation(
                    "unexpected end of global LaunchBox media element".to_string(),
                ))
            }
            _ => {}
        }
        buffer.clear();
    }
    let Some(provider_game_id) = provider_game_id else {
        return Ok(None);
    };
    let Some(media) = normalize_media(RawMedia {
        id: None,
        media_type,
        url,
        ordinal,
    }) else {
        return Ok(None);
    };
    Ok(Some((provider_game_id, media)))
}

fn parse_game_element<R: std::io::BufRead>(
    reader: &mut Reader<R>,
) -> Result<Option<LaunchBoxRecord>, LaunchBoxError> {
    let mut record = RawRecord::default();
    let mut stack: Vec<Vec<u8>> = Vec::new();
    let mut text_stack: Vec<String> = Vec::new();
    let mut active_image: Option<RawMedia> = None;
    let mut buffer = Vec::new();
    let mut ordinal = 0;
    loop {
        match reader.read_event_into(&mut buffer)? {
            Event::Start(event) => {
                let name = event.local_name().as_ref().to_ascii_lowercase();
                if matches!(name.as_slice(), b"image" | b"gameimage" | b"media") {
                    active_image = Some(RawMedia {
                        ordinal,
                        ..RawMedia::default()
                    });
                    ordinal += 1;
                }
                stack.push(name);
                text_stack.push(String::new());
            }
            Event::Text(event) => {
                if let Some(text) = text_stack.last_mut() {
                    text.push_str(&clean_text(
                        String::from_utf8_lossy(event.as_ref()).into_owned(),
                    ));
                }
            }
            Event::CData(event) => {
                if let Some(text) = text_stack.last_mut() {
                    text.push_str(&clean_text(
                        String::from_utf8_lossy(event.as_ref()).into_owned(),
                    ));
                }
            }
            Event::End(event) => {
                let name = event.local_name().as_ref().to_ascii_lowercase();
                let text = text_stack.pop().unwrap_or_default();
                stack.pop();
                if name == b"game" {
                    break;
                }
                if let Some(image) = active_image.as_mut() {
                    match name.as_slice() {
                        b"id" | b"databaseid" | b"databasegameid" | b"gameid" => {
                            image.id = non_empty(text)
                        }
                        b"type" | b"imagetype" | b"category" => image.media_type = text,
                        b"url" | b"imageurl" | b"path" | b"filename" | b"file" => {
                            image.url = non_empty(text)
                        }
                        b"image" | b"gameimage" | b"media" => {
                            record.media.push(std::mem::take(image));
                            active_image = None;
                        }
                        _ => {}
                    }
                } else {
                    match name.as_slice() {
                        b"id" | b"databaseid" => record.provider_game_id = non_empty(text),
                        b"title" | b"name" => record.title = non_empty(text),
                        b"alternatename" | b"alternatetitle" | b"sorttitle" => {
                            record.alternate_titles.push(text)
                        }
                        b"platform" => record.platform = non_empty(text),
                        b"overview" | b"description" => record.description = non_empty(text),
                        b"developer" | b"developers" => record.developer = non_empty(text),
                        b"publisher" | b"publishers" => record.publisher = non_empty(text),
                        b"releasedate" | b"date" => record.release_date = non_empty(text),
                        b"genre" | b"genres" => record.genres.push(text),
                        b"multiplayer" | b"localmultiplayer" => {
                            record.multiplayer = non_empty(text)
                        }
                        b"cooperative" | b"coop" | b"localcoop" => {
                            record.cooperative = non_empty(text)
                        }
                        b"maxplayers" | b"players" | b"playercount" => {
                            record.max_players = non_empty(text)
                        }
                        b"communitystarrating"
                        | b"communityrating"
                        | b"communityscore"
                        | b"rating" => record.community_rating = non_empty(text),
                        b"communitystarratingtotalvotes"
                        | b"communityratingtotalvotes"
                        | b"ratingvotes"
                        | b"ratingcount" => record.rating_count = non_empty(text),
                        _ => {}
                    }
                }
            }
            Event::Eof => {
                return Err(LaunchBoxError::Validation(
                    "unexpected end of Metadata.xml".to_string(),
                ))
            }
            _ => {}
        }
        buffer.clear();
    }
    Ok(normalize_record(record))
}

pub fn parse_metadata_json<F: FnMut(LaunchBoxRecord) -> Result<(), LaunchBoxError>>(
    source: &str,
    mut on_record: F,
) -> Result<usize, LaunchBoxError> {
    let value: serde_json::Value = serde_json::from_str(source)?;
    let records = value
        .as_array()
        .cloned()
        .or_else(|| {
            value
                .get("games")
                .and_then(serde_json::Value::as_array)
                .cloned()
        })
        .or_else(|| {
            value
                .get("Games")
                .and_then(serde_json::Value::as_array)
                .cloned()
        })
        .ok_or_else(|| {
            LaunchBoxError::Validation("Metadata.json does not contain games".to_string())
        })?;
    let mut count = 0;
    for value in records {
        let record = normalize_json_record(&value);
        if let Some(record) = record {
            on_record(record)?;
            count += 1;
        }
    }
    Ok(count)
}

fn json_string(value: &serde_json::Value, names: &[&str]) -> Option<String> {
    names.iter().find_map(|name| {
        value
            .get(*name)
            .and_then(|item| item.as_str())
            .map(ToOwned::to_owned)
    })
}

fn normalize_json_record(value: &serde_json::Value) -> Option<LaunchBoxRecord> {
    let mut raw = RawRecord {
        provider_game_id: json_string(value, &["id", "ID", "databaseId", "DatabaseID"]),
        title: json_string(value, &["name", "Name", "title", "Title"]),
        platform: json_string(value, &["platform", "Platform"]),
        description: json_string(
            value,
            &["description", "Description", "overview", "Overview"],
        ),
        developer: json_string(
            value,
            &["developer", "Developer", "developers", "Developers"],
        ),
        publisher: json_string(
            value,
            &["publisher", "Publisher", "publishers", "Publishers"],
        ),
        release_date: json_string(value, &["releaseDate", "ReleaseDate", "Release Date"]),
        multiplayer: json_string(value, &["multiplayer", "Multiplayer", "localMultiplayer"]),
        cooperative: json_string(value, &["cooperative", "Cooperative", "localCoop"]),
        max_players: json_string(value, &["maxPlayers", "MaxPlayers", "playerCount"]),
        community_rating: json_string(
            value,
            &[
                "communityStarRating",
                "CommunityStarRating",
                "communityRating",
                "CommunityRating",
                "rating",
            ],
        ),
        rating_count: json_string(
            value,
            &[
                "communityStarRatingTotalVotes",
                "CommunityStarRatingTotalVotes",
                "ratingCount",
                "ratingVotes",
            ],
        ),
        ..RawRecord::default()
    };
    raw.alternate_titles = json_string(
        value,
        &["alternateTitles", "AlternateTitles", "alternateTitle"],
    )
    .into_iter()
    .collect();
    raw.genres = json_string(value, &["genres", "Genres", "genre", "Genre"])
        .into_iter()
        .collect();
    if let Some(images) = value
        .get("images")
        .or_else(|| value.get("Images"))
        .and_then(serde_json::Value::as_array)
    {
        raw.media = images
            .iter()
            .enumerate()
            .filter_map(|(ordinal, image)| {
                Some(RawMedia {
                    id: json_string(image, &["id", "ID", "databaseId"]),
                    media_type: json_string(image, &["type", "Type", "imageType", "category"])
                        .unwrap_or_default(),
                    url: json_string(image, &["url", "URL", "path", "fileName", "source"]),
                    ordinal: ordinal as i64,
                })
            })
            .collect();
    }
    normalize_record(raw)
}

fn active_catalog_version(connection: &Connection) -> Result<Option<String>, LaunchBoxError> {
    connection
        .query_row(
            "SELECT catalog_version FROM launchbox_catalog_state WHERE id = 1 AND status = 'ready'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(LaunchBoxError::Database)
}

pub(crate) fn get_game_metadata(
    connection: &Connection,
    game_id: &str,
    title_id: Option<&str>,
    title: &str,
    platform: &str,
) -> Result<Option<LocalLaunchBoxDetails>, DatabaseError> {
    let version = active_catalog_version(connection).map_err(|_| rusqlite::Error::InvalidQuery)?;
    let Some(version) = version else {
        return Ok(None);
    };
    let provider_id = title_id.and_then(|native_id| {
        connection
            .query_row(
                "SELECT m.provider_game_id
         FROM external_identity_mappings m
         JOIN launchbox_games g
           ON g.provider_game_id = m.provider_game_id
          AND g.catalog_version = ?3
         WHERE m.platform = ?1 AND m.native_id = ?2 AND m.provider = 'launchbox'
           AND m.confidence IN ('exact', 'high')",
                params![normalize_platform(platform), native_id, version],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .ok()
            .flatten()
    });
    let selected_provider_id = provider_id.clone().or_else(|| connection.query_row("SELECT provider_game_id FROM launchbox_games WHERE normalized_platform = ?1 AND normalized_title = ?2 AND catalog_version = ?3", params![normalize_platform(platform), normalize_title(title), version], |row| row.get::<_, String>(0)).optional().ok().flatten());
    let record = if let Some(provider_id) = selected_provider_id.as_deref() {
        connection.query_row("SELECT canonical_title, description, developer, publisher, release_date, normalized_genres_json, local_multiplayer, max_local_players, community_rating_raw, community_rating_scale, community_rating_count FROM launchbox_games WHERE provider_game_id = ?1 AND catalog_version = ?2", params![provider_id, version], read_local_details).optional()?
    } else {
        let normalized_title = normalize_title(title);
        connection.query_row("SELECT canonical_title, description, developer, publisher, release_date, normalized_genres_json, local_multiplayer, max_local_players, community_rating_raw, community_rating_scale, community_rating_count FROM launchbox_games WHERE normalized_platform = ?1 AND normalized_title = ?2 AND catalog_version = ?3", params![normalize_platform(platform), normalized_title, version], read_local_details).optional()?
    };
    let Some(mut record) = record else {
        return Ok(None);
    };
    if let Some(provider_id) = selected_provider_id {
        let mut statement = connection.prepare("SELECT c.local_path FROM launchbox_media_refs r JOIN launchbox_screenshot_cache c ON c.media_url = r.media_url AND c.game_id = ?1 WHERE r.provider_game_id = ?2 AND r.catalog_version = ?3 AND r.media_type = 'screenshot' AND c.status = 'cached' AND c.local_path IS NOT NULL ORDER BY r.ordinal LIMIT 12")?;
        record.screenshots = statement
            .query_map(params![game_id, provider_id, version], |row| {
                row.get::<_, String>(0)
            })?
            .collect::<Result<Vec<_>, _>>()?;
    }
    Ok(Some(record))
}

fn read_local_details(row: &rusqlite::Row<'_>) -> rusqlite::Result<LocalLaunchBoxDetails> {
    let genres_json: String = row.get(5)?;
    Ok(LocalLaunchBoxDetails {
        canonical_title: row.get(0)?,
        description: row.get(1)?,
        developer: row.get(2)?,
        publisher: row.get(3)?,
        release_date: row.get(4)?,
        normalized_genres: serde_json::from_str(&genres_json).unwrap_or_default(),
        local_multiplayer: row.get(6)?,
        max_local_players: row.get(7)?,
        community_rating_raw: row.get(8)?,
        community_rating_scale: row.get(9)?,
        community_rating_count: row.get(10)?,
        screenshots: Vec::new(),
    })
}

fn get_catalog_paths(state: &DatabaseState) -> (PathBuf, PathBuf, PathBuf) {
    let root = state.data_directory.cache_directory().join("launchbox");
    (
        root.clone(),
        root.join("launchbox-metadata.zip.download"),
        root.join("catalog-staging"),
    )
}

fn apply_runtime_state(
    mut status: LaunchBoxCatalogStatus,
    state: &DatabaseState,
) -> LaunchBoxCatalogStatus {
    let (phase, active_version, runtime_error, progress) = catalog_runtime_snapshot(state);
    status.progress = progress;
    if status.catalog_version.is_none() {
        status.catalog_version = active_version.clone();
    }
    let has_active_catalog = active_version.is_some() || status.catalog_version.is_some();
    match phase {
        LaunchBoxCatalogPhase::Updating => {
            status.available = has_active_catalog;
            status.status = "updating".to_string();
            status.last_error = None;
        }
        LaunchBoxCatalogPhase::Error => {
            status.available = has_active_catalog;
            status.status = if has_active_catalog {
                "ready".to_string()
            } else {
                "error".to_string()
            };
            status.last_error = runtime_error.or(status.last_error);
        }
        LaunchBoxCatalogPhase::Ready => {
            status.available = has_active_catalog;
            status.status = "ready".to_string();
        }
        LaunchBoxCatalogPhase::NotDownloaded => {}
    }
    status
}

pub fn get_status(state: &DatabaseState) -> Result<LaunchBoxCatalogStatus, DatabaseError> {
    let connection = state
        .connection
        .lock()
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    if state.take_connection_poisoned() {
        state.log(
            "launchbox-database",
            "launchbox_database_mutex_poisoned",
            "operation=get_status",
        );
        state.log(
            "launchbox-database",
            "launchbox_database_mutex_recovered",
            "operation=get_status",
        );
    }
    let row = connection.query_row("SELECT catalog_version, catalog_schema_version, metadata_zip_url, downloaded_at, record_count, switch_record_count, zip_size_bytes, source_size_bytes, import_duration_ms, status, last_error FROM launchbox_catalog_state WHERE id = 1", [], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?, row.get::<_, String>(2)?, row.get::<_, Option<String>>(3)?, row.get::<_, i64>(4)?, row.get::<_, i64>(5)?, row.get::<_, Option<i64>>(6)?, row.get::<_, Option<i64>>(7)?, row.get::<_, Option<i64>>(8)?, row.get::<_, String>(9)?, row.get::<_, Option<String>>(10)?))).optional()?;
    let Some(row) = row else {
        return Ok(apply_runtime_state(
            LaunchBoxCatalogStatus {
                available: false,
                catalog_version: None,
                catalog_schema_version: None,
                metadata_zip_url: LAUNCHBOX_METADATA_ZIP_URL.to_string(),
                downloaded_at: None,
                expires_at: None,
                record_count: 0,
                switch_record_count: 0,
                zip_size_bytes: None,
                source_size_bytes: None,
                import_duration_ms: None,
                status: "missing".to_string(),
                last_error: None,
                ttl_expired: true,
                progress: None,
            },
            state,
        ));
    };
    let ttl_expired = row
        .3
        .as_deref()
        .and_then(parse_timestamp)
        .map(|timestamp| {
            now_seconds().saturating_sub(timestamp) as u64 > LAUNCHBOX_CATALOG_TTL.as_secs()
        })
        .unwrap_or(true);
    let expires_at = row
        .3
        .as_deref()
        .and_then(parse_timestamp)
        .map(|timestamp| (timestamp + LAUNCHBOX_CATALOG_TTL.as_secs() as i64).to_string());
    Ok(apply_runtime_state(
        LaunchBoxCatalogStatus {
            available: row.9 == "ready",
            catalog_version: Some(row.0),
            catalog_schema_version: Some(row.1),
            metadata_zip_url: row.2,
            downloaded_at: row.3,
            expires_at,
            record_count: row.4,
            switch_record_count: row.5,
            zip_size_bytes: row.6,
            source_size_bytes: row.7,
            import_duration_ms: row.8,
            status: row.9,
            last_error: row.10,
            ttl_expired,
            progress: None,
        },
        state,
    ))
}

pub async fn refresh_catalog(
    state: &DatabaseState,
    force: bool,
) -> Result<LaunchBoxCatalogStatus, String> {
    match refresh_catalog_inner(state, force).await {
        Ok(status) if status.status == "updating" => Ok(status),
        Ok(status) => {
            finish_catalog_update_success(state, status.catalog_version.clone());
            get_status(state).map_err(|error| error.to_string())
        }
        Err(error) => {
            if error == "LAUNCHBOX_CATALOG_UPDATE_IN_PROGRESS" {
                return Err(error);
            }
            let fallback = finish_catalog_update_error(state, &error);
            state.log(
                "launchbox-catalog",
                "launchbox_catalog_update_failed",
                &format!("error={error} fallback={fallback}"),
            );
            if fallback {
                Err(format!("LAUNCHBOX_UPDATE_FAILED_WITH_FALLBACK: {error}"))
            } else {
                Err(error)
            }
        }
    }
}

async fn refresh_catalog_inner(
    state: &DatabaseState,
    force: bool,
) -> Result<LaunchBoxCatalogStatus, String> {
    let status = get_status(state).map_err(|error| error.to_string())?;
    let schema_outdated = status.catalog_schema_version != Some(LAUNCHBOX_CATALOG_SCHEMA_VERSION);
    if !force && status.available && !status.ttl_expired && !schema_outdated {
        state.log(
            "launchbox-catalog",
            "launchbox_catalog_cache_valid",
            &format!("record_count={}", status.record_count),
        );
        return Ok(status);
    }
    if schema_outdated {
        state.log(
            "launchbox-catalog",
            "launchbox_catalog_schema_refresh_required",
            &format!(
                "stored_schema={} expected_schema={}",
                status
                    .catalog_schema_version
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "<none>".to_string()),
                LAUNCHBOX_CATALOG_SCHEMA_VERSION
            ),
        );
    }
    begin_catalog_update(state)?;
    state.log(
        "launchbox-catalog",
        "launchbox_catalog_update_started",
        &format!("force={force}"),
    );
    let started = Instant::now();
    let (cache_root, zip_path, staging_root) = get_catalog_paths(state);
    fs::create_dir_all(&cache_root).map_err(|error| error.to_string())?;
    let client = Client::builder()
        .user_agent("LumaDeck/LaunchBoxCatalogV1")
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .get(LAUNCHBOX_METADATA_ZIP_URL)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "LAUNCHBOX_CATALOG_HTTP_{}",
            response.status().as_u16()
        ));
    }
    let total_download_bytes = response.content_length().map(|value| value as i64);
    update_catalog_progress(state, "downloading", None, Some(0), total_download_bytes);
    state.log(
        "launchbox-catalog",
        "launchbox_catalog_download_started",
        LAUNCHBOX_METADATA_ZIP_URL,
    );
    let temporary = zip_path;
    let file = File::create(&temporary).map_err(|error| error.to_string())?;
    let mut writer = BufWriter::new(file);
    let mut stream = response.bytes_stream();
    let mut downloaded_bytes = 0_i64;
    let mut reported_bytes = 0_i64;
    let mut last_progress_report = Instant::now();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| error.to_string())?;
        downloaded_bytes = downloaded_bytes.saturating_add(chunk.len() as i64);
        writer
            .write_all(&chunk)
            .map_err(|error| error.to_string())?;
        if downloaded_bytes.saturating_sub(reported_bytes) >= 256 * 1024
            || last_progress_report.elapsed() >= Duration::from_millis(250)
        {
            update_catalog_progress(
                state,
                "downloading",
                None,
                Some(downloaded_bytes),
                total_download_bytes,
            );
            reported_bytes = downloaded_bytes;
            last_progress_report = Instant::now();
        }
    }
    writer.flush().map_err(|error| error.to_string())?;
    let zip_size = fs::metadata(&temporary)
        .map_err(|error| error.to_string())?
        .len();
    update_catalog_progress(
        state,
        "downloading",
        None,
        Some(zip_size as i64),
        total_download_bytes,
    );
    if zip_size < 1024 {
        let _ = fs::remove_file(&temporary);
        return Err("LAUNCHBOX_CATALOG_TOO_SMALL".to_string());
    }
    state.log(
        "launchbox-catalog",
        "launchbox_catalog_download_completed",
        &format!("bytes={zip_size}"),
    );
    state.log(
        "launchbox-catalog",
        "launchbox_catalog_extract_started",
        "native_rust_zip=true",
    );
    update_catalog_progress(state, "extracting", None, None, None);
    let staging = staging_root.join(format!("{}-{}", now_seconds(), std::process::id()));
    fs::create_dir_all(&staging).map_err(|error| error.to_string())?;
    let metadata_path = match extract_metadata_zip(&temporary, &staging) {
        Ok(path) => path,
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            let _ = fs::remove_dir_all(&staging);
            return Err(error.to_string());
        }
    };
    let source_size = match fs::metadata(&metadata_path) {
        Ok(metadata) => metadata.len(),
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            let _ = fs::remove_dir_all(&staging);
            return Err(error.to_string());
        }
    };
    state.log(
        "launchbox-catalog",
        "launchbox_catalog_parse_started",
        &format!("source_bytes={source_size}"),
    );
    update_catalog_progress(state, "importing", Some(0), None, None);
    let import = import_catalog_file(
        state,
        &metadata_path,
        zip_size as i64,
        source_size as i64,
        started.elapsed().as_millis() as i64,
    )
    .map_err(|error| error.to_string());
    let _ = fs::remove_file(&temporary);
    let _ = fs::remove_dir_all(&staging);
    match import {
        Ok(status) => {
            state.log(
                "launchbox-catalog",
                "launchbox_catalog_swap_completed",
                &format!("record_count={}", status.record_count),
            );
            Ok(status)
        }
        Err(error) => Err(error),
    }
}

pub async fn download_screenshots(
    state: &DatabaseState,
    game_id: &str,
) -> Result<Vec<String>, String> {
    Ok(download_screenshots_with_report(state, game_id)
        .await?
        .paths)
}

pub async fn download_screenshots_with_report(
    state: &DatabaseState,
    game_id: &str,
) -> Result<LaunchBoxScreenshotReport, String> {
    state.log(
        "launchbox-media",
        "launchbox_media_resolution_started",
        &format!("game_id={game_id}"),
    );
    let (catalog_phase, active_version, _, _) = catalog_runtime_snapshot(state);
    if catalog_phase == LaunchBoxCatalogPhase::Updating && active_version.is_none() {
        state.log(
            "launchbox-catalog",
            "launchbox_catalog_read_deferred",
            &format!("game_id={game_id} reason=first_import_screenshots"),
        );
        return Err("LAUNCHBOX_CATALOG_NOT_READY".to_string());
    }
    if catalog_phase == LaunchBoxCatalogPhase::Updating && active_version.is_some() {
        state.log(
            "launchbox-catalog",
            "launchbox_catalog_previous_version_used",
            &format!("game_id={game_id} operation=screenshots"),
        );
    }
    let (version, provider_id, references) = {
        let connection = lock_connection(state, "download_screenshots")?;
        let version = active_catalog_version(&connection)
            .map_err(|_| "LAUNCHBOX_CATALOG_UNAVAILABLE".to_string())?
            .ok_or_else(|| "LAUNCHBOX_CATALOG_UNAVAILABLE".to_string())?;
        let game = connection
            .query_row(
                "SELECT title, platform, title_id FROM games WHERE id = ?1 AND source = 'emulator'",
                params![game_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|_| "GAME_NOT_FOUND".to_string())?
            .ok_or_else(|| "GAME_NOT_FOUND".to_string())?;
        let provider_id = game.2.as_deref().and_then(|title_id| connection.query_row("SELECT m.provider_game_id FROM external_identity_mappings m JOIN launchbox_games g ON g.provider_game_id = m.provider_game_id AND g.catalog_version = ?3 WHERE m.platform = ?1 AND m.native_id = ?2 AND m.provider = 'launchbox' AND m.confidence IN ('exact', 'high')", params![normalize_platform(&game.1), title_id, version], |row| row.get::<_, String>(0)).optional().ok().flatten()).or_else(|| connection.query_row("SELECT provider_game_id FROM launchbox_games WHERE catalog_version = ?1 AND normalized_platform = ?2 AND normalized_title = ?3", params![version, normalize_platform(&game.1), normalize_title(&game.0)], |row| row.get::<_, String>(0)).optional().ok().flatten()).ok_or_else(|| "LAUNCHBOX_GAME_UNRESOLVED".to_string())?;
        let mut statement = connection.prepare("SELECT r.provider_media_id, r.media_url, c.local_path FROM launchbox_media_refs r LEFT JOIN launchbox_screenshot_cache c ON c.game_id = ?1 AND c.media_url = r.media_url WHERE r.provider_game_id = ?2 AND r.catalog_version = ?3 AND r.media_type = 'screenshot' ORDER BY r.ordinal LIMIT 12").map_err(|_| "LAUNCHBOX_MEDIA_QUERY_FAILED".to_string())?;
        let references = statement
            .query_map(params![game_id, provider_id, version], |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })
            .map_err(|_| "LAUNCHBOX_MEDIA_QUERY_FAILED".to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| "LAUNCHBOX_MEDIA_QUERY_FAILED".to_string())?;
        state.log(
            "launchbox-media",
            "launchbox_media_candidates_found",
            &format!(
                "game_id={game_id} title_id={} launchbox_id={} media_type=screenshot candidate_count={}",
                game.2.as_deref().unwrap_or("<none>"),
                provider_id,
                references.len()
            ),
        );
        (version, provider_id, references)
    };
    let _ = version;
    if references.is_empty() {
        state.log(
            "launchbox-media",
            "launchbox_media_resolution_completed",
            &format!(
                "game_id={game_id} launchbox_id={provider_id} candidate_count=0 downloaded_count=0 cached_count=0 failed_count=0"
            ),
        );
        return Ok(LaunchBoxScreenshotReport {
            paths: Vec::new(),
            cached: 0,
            downloaded: 0,
            failed: 0,
        });
    }
    let client = Client::builder()
        .user_agent("LumaDeck/LaunchBoxMediaV1")
        .default_headers({
            let mut headers = HeaderMap::new();
            headers.insert(
                reqwest::header::ACCEPT,
                HeaderValue::from_static("image/avif,image/webp,image/png,image/jpeg,*/*;q=0.8"),
            );
            headers
        })
        .build()
        .map_err(|error| error.to_string())?;
    let screenshot_root = state
        .data_directory
        .cache_directory()
        .join("launchbox")
        .join("screenshots");
    fs::create_dir_all(&screenshot_root).map_err(|error| error.to_string())?;
    let mut cached_paths = Vec::new();
    let mut cached = 0_i64;
    let mut downloaded = 0_i64;
    let mut failed = 0_i64;
    let requested = references.len() as i64;
    for (provider_media_id, url, local_path) in references {
        if let Some(local_path) = local_path.filter(|path| is_valid_cached_image(state, path)) {
            state.log(
                "launchbox-media",
                "launchbox_media_cache_hit",
                &format!(
                    "game_id={game_id} launchbox_id={provider_id} media_type=screenshot source_url={url} destination_path={local_path} status=200 content_type=cached bytes_received=0"
                ),
            );
            cached += 1;
            cached_paths.push(local_path);
            continue;
        }
        if !is_allowed_media_url(&url) {
            state.log(
                "launchbox-media",
                "launchbox_media_download_failed",
                &format!(
                    "game_id={game_id} launchbox_id={provider_id} media_type=screenshot source_url={url} destination_path=<none> status=<none> content_type=<none> bytes_received=0 reason=host_not_allowed"
                ),
            );
            failed += 1;
            continue;
        }
        state.log(
            "launchbox-media",
            "launchbox_media_download_started",
            &format!(
                "game_id={game_id} launchbox_id={provider_id} media_type=screenshot source_url={url}"
            ),
        );
        let response = match client.get(&url).send().await {
            Ok(response) => {
                let status = response.status();
                let content_type = response
                    .headers()
                    .get(reqwest::header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("<none>")
                    .to_string();
                if status.is_success() {
                    if !content_type.is_empty()
                        && content_type != "<none>"
                        && !content_type.to_ascii_lowercase().starts_with("image/")
                    {
                        state.log(
                            "launchbox-media",
                            "launchbox_media_download_failed",
                            &format!(
                                "game_id={game_id} launchbox_id={provider_id} media_type=screenshot source_url={url} destination_path=<none> status={status} content_type={content_type} bytes_received=0 reason=content_type_not_image"
                            ),
                        );
                        failed += 1;
                        continue;
                    }
                    response
                } else {
                    state.log(
                        "launchbox-media",
                        "launchbox_media_download_failed",
                        &format!(
                            "game_id={game_id} launchbox_id={provider_id} media_type=screenshot source_url={url} destination_path=<none> status={status} content_type={content_type} bytes_received=0 reason=http_status"
                        ),
                    );
                    failed += 1;
                    continue;
                }
            }
            Err(error) => {
                state.log(
                    "launchbox-media",
                    "launchbox_media_download_failed",
                    &format!(
                        "game_id={game_id} launchbox_id={provider_id} media_type=screenshot source_url={url} destination_path=<none> status=<none> content_type=<none> bytes_received=0 reason=request error={error}"
                    ),
                );
                failed += 1;
                continue;
            }
        };
        let bytes = match response.bytes().await {
            Ok(bytes) if bytes.len() <= 20 * 1024 * 1024 => bytes,
            Ok(bytes) => {
                state.log(
                    "launchbox-media",
                    "launchbox_media_download_failed",
                    &format!(
                        "game_id={game_id} launchbox_id={provider_id} media_type=screenshot source_url={url} destination_path=<none> status=200 content_type=image bytes_received={} reason=too_large",
                        bytes.len()
                    ),
                );
                failed += 1;
                continue;
            }
            Err(error) => {
                state.log(
                    "launchbox-media",
                    "launchbox_media_download_failed",
                    &format!(
                        "game_id={game_id} launchbox_id={provider_id} media_type=screenshot source_url={url} destination_path=<none> status=200 content_type=image bytes_received=0 reason=read error={error}"
                    ),
                );
                failed += 1;
                continue;
            }
        };
        if image::guess_format(&bytes).is_err() {
            state.log(
                "launchbox-media",
                "launchbox_media_download_failed",
                &format!(
                    "game_id={game_id} launchbox_id={provider_id} media_type=screenshot source_url={url} destination_path=<none> status=200 content_type=image bytes_received={} reason=invalid_image",
                    bytes.len()
                ),
            );
            failed += 1;
            continue;
        }
        let checksum = sha256_bytes(&bytes);
        let extension = image::guess_format(&bytes)
            .ok()
            .map(|format| match format {
                image::ImageFormat::Jpeg => "jpg",
                image::ImageFormat::Png => "png",
                image::ImageFormat::WebP => "webp",
                _ => "img",
            })
            .unwrap_or("img");
        let relative_path = PathBuf::from("cache")
            .join("launchbox")
            .join("screenshots")
            .join(format!("{checksum}.{extension}"));
        let absolute_path = state.data_directory.root().join(&relative_path);
        let temporary_path = absolute_path.with_extension(format!("{extension}.download"));
        if absolute_path.exists() && is_valid_image_file(&absolute_path) {
            let local_path = relative_path.to_string_lossy().replace('\\', "/");
            state.log(
                "launchbox-media",
                "launchbox_media_cache_hit",
                &format!(
                    "game_id={game_id} launchbox_id={provider_id} media_type=screenshot source_url={url} destination_path={local_path} status=200 content_type=image bytes_received=0"
                ),
            );
            cached += 1;
            cached_paths.push(local_path);
            continue;
        }
        fs::write(&temporary_path, &bytes).map_err(|error| error.to_string())?;
        fs::rename(&temporary_path, &absolute_path).map_err(|error| error.to_string())?;
        let connection = lock_connection(state, "cache_screenshot")?;
        connection.execute("INSERT INTO launchbox_screenshot_cache(game_id, provider_media_id, media_url, local_path, fetched_at, content_hash, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'cached') ON CONFLICT(game_id, media_url) DO UPDATE SET provider_media_id=excluded.provider_media_id, local_path=excluded.local_path, fetched_at=excluded.fetched_at, content_hash=excluded.content_hash, status='cached'", params![game_id, provider_media_id, url, relative_path.to_string_lossy().replace('\\', "/"), now_seconds().to_string(), checksum]).map_err(|error| error.to_string())?;
        drop(connection);
        state.log(
            "launchbox-media",
            "launchbox_media_download_completed",
            &format!(
                "game_id={game_id} launchbox_id={provider_id} media_type=screenshot source_url={url} destination_path={} status=200 content_type=image bytes_received={}",
                relative_path.to_string_lossy().replace('\\', "/"),
                bytes.len()
            ),
        );
        downloaded += 1;
        cached_paths.push(relative_path.to_string_lossy().replace('\\', "/"));
    }
    state.log(
        "launchbox-media",
        "launchbox_media_resolution_completed",
        &format!(
            "game_id={game_id} launchbox_id={provider_id} candidate_count={requested} downloaded_count={downloaded} cached_count={cached} failed_count={failed}"
        ),
    );
    Ok(LaunchBoxScreenshotReport {
        paths: cached_paths,
        cached,
        downloaded,
        failed,
    })
}

fn is_allowed_media_url(url: &str) -> bool {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|url| {
            url.host_str().map(|host| {
                host == "images.launchbox-app.com" || host.ends_with(".launchbox-app.com")
            })
        })
        .unwrap_or(false)
}

fn is_valid_cached_image(state: &DatabaseState, relative_path: &str) -> bool {
    is_valid_image_file(&state.data_directory.root().join(relative_path))
}

fn is_valid_image_file(path: &Path) -> bool {
    fs::read(path)
        .ok()
        .filter(|bytes| !bytes.is_empty())
        .is_some_and(|bytes| image::guess_format(&bytes).is_ok())
}

fn sha256_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn extract_metadata_zip(zip_path: &Path, staging: &Path) -> Result<PathBuf, LaunchBoxError> {
    let file = File::open(zip_path)?;
    let mut archive = ZipArchive::new(file)?;
    let metadata_name = (0..archive.len())
        .find_map(|index| {
            archive.by_index(index).ok().and_then(|entry| {
                let name = entry.name().replace('\\', "/");
                let file_name = Path::new(&name).file_name()?.to_str()?.to_ascii_lowercase();
                (file_name == "metadata.xml" || file_name == "metadata.json").then_some(name)
            })
        })
        .ok_or(LaunchBoxError::MetadataMissing)?;
    let mut entry = archive.by_name(&metadata_name)?;
    let destination = staging.join(
        Path::new(&metadata_name)
            .file_name()
            .ok_or(LaunchBoxError::MetadataMissing)?,
    );
    let output = File::create(&destination)?;
    let mut writer = BufWriter::new(output);
    std::io::copy(&mut entry, &mut writer)?;
    writer.flush()?;
    Ok(destination)
}

fn import_catalog_file(
    state: &DatabaseState,
    path: &Path,
    zip_size: i64,
    source_size: i64,
    duration_ms: i64,
) -> Result<LaunchBoxCatalogStatus, LaunchBoxError> {
    let connection = Connection::open(&state.path)?;
    connection.pragma_update(None, "foreign_keys", true)?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.busy_timeout(Duration::from_secs(10))?;
    let transaction = connection.unchecked_transaction()?;
    let catalog_version = format!("lb-{}-{}", now_seconds(), std::process::id());
    let mut count = 0_i64;
    let mut switch_count = 0_i64;
    let insert = |transaction: &Transaction<'_>,
                  record: LaunchBoxRecord|
     -> Result<(), LaunchBoxError> {
        if record.provider_game_id.is_empty() {
            return Ok(());
        }
        transaction.execute("INSERT INTO launchbox_games(provider_game_id, catalog_version, canonical_title, normalized_title, alternate_titles_json, platform, normalized_platform, description, developer, publisher, release_date, genres_json, normalized_genres_json, local_multiplayer, max_local_players, community_rating_raw, community_rating_scale, community_rating_count, community_rating_raw_text) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)", params![record.provider_game_id, catalog_version, record.canonical_title, record.normalized_title, serde_json::to_string(&record.alternate_titles)?, record.platform, record.normalized_platform, record.description, record.developer, record.publisher, record.release_date, serde_json::to_string(&record.genres)?, serde_json::to_string(&record.normalized_genres)?, record.local_multiplayer, record.max_local_players, record.community_rating_raw, record.community_rating_scale, record.community_rating_count, record.community_rating_raw_text])?;
        for media in record.media.into_iter() {
            transaction.execute("INSERT OR IGNORE INTO launchbox_media_refs(provider_game_id, catalog_version, provider_media_id, media_type, media_url, ordinal) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![record.provider_game_id, catalog_version, media.provider_media_id, media.media_type, media.url, media.ordinal])?;
        }
        Ok(())
    };
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if extension.eq_ignore_ascii_case("json") {
        let source = fs::read_to_string(path)?;
        parse_metadata_json(&source, |record| {
            if record.normalized_platform == "nintendo_switch" {
                switch_count += 1;
            }
            count += 1;
            let result = insert(&transaction, record);
            if result.is_ok() && count % 1_000 == 0 {
                update_catalog_progress(state, "importing", Some(count), None, None);
            }
            result
        })?;
    } else {
        let source = BufReader::new(File::open(path)?);
        parse_metadata_xml_with_media(
            source,
            |record| {
                if record.normalized_platform == "nintendo_switch" {
                    switch_count += 1;
                }
                count += 1;
                let result = insert(&transaction, record);
                if result.is_ok() && count % 1_000 == 0 {
                    update_catalog_progress(state, "importing", Some(count), None, None);
                }
                result
            },
            |provider_game_id, media| {
                transaction.execute(
                    "INSERT OR IGNORE INTO launchbox_media_refs(provider_game_id, catalog_version, provider_media_id, media_type, media_url, ordinal) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        provider_game_id,
                        catalog_version,
                        media.provider_media_id,
                        media.media_type,
                        media.url,
                        media.ordinal
                    ],
                )?;
                Ok(())
            },
        )?;
    }
    update_catalog_progress(state, "validating", Some(count), None, None);
    if count < 1 {
        return Err(LaunchBoxError::Validation(
            "record count is zero".to_string(),
        ));
    }
    if switch_count < 1 {
        return Err(LaunchBoxError::Validation(
            "Nintendo Switch records are missing".to_string(),
        ));
    }
    let source_hash = sha256_file(path)?;
    let now = now_seconds().to_string();
    transaction.execute(
        "DELETE FROM launchbox_games WHERE catalog_version <> ?1",
        params![catalog_version],
    )?;
    transaction.execute(
        "DELETE FROM launchbox_media_refs WHERE catalog_version <> ?1",
        params![catalog_version],
    )?;
    transaction.execute(
        "DELETE FROM external_identity_mappings
         WHERE provider = 'launchbox'
           AND NOT EXISTS (
               SELECT 1 FROM launchbox_games g
               WHERE g.provider_game_id = external_identity_mappings.provider_game_id
                 AND g.catalog_version = ?1
           )",
        params![catalog_version],
    )?;
    transaction.execute("DELETE FROM launchbox_negative_matches", [])?;
    transaction.execute("INSERT INTO launchbox_catalog_state(id, catalog_version, catalog_schema_version, metadata_zip_url, downloaded_at, source_hash, zip_size_bytes, source_size_bytes, record_count, switch_record_count, import_duration_ms, status, last_error) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, 'ready', NULL) ON CONFLICT(id) DO UPDATE SET catalog_version=excluded.catalog_version, catalog_schema_version=excluded.catalog_schema_version, metadata_zip_url=excluded.metadata_zip_url, downloaded_at=excluded.downloaded_at, source_hash=excluded.source_hash, zip_size_bytes=excluded.zip_size_bytes, source_size_bytes=excluded.source_size_bytes, record_count=excluded.record_count, switch_record_count=excluded.switch_record_count, import_duration_ms=excluded.import_duration_ms, status='ready', last_error=NULL", params![catalog_version, LAUNCHBOX_CATALOG_SCHEMA_VERSION, LAUNCHBOX_METADATA_ZIP_URL, now, source_hash, zip_size, source_size, count, switch_count, duration_ms])?;
    update_catalog_progress(state, "activating", Some(count), None, None);
    transaction.commit()?;
    let status = get_status(state).map_err(|error| {
        LaunchBoxError::Database(match error {
            DatabaseError::Sqlite(error) => error,
            _ => rusqlite::Error::InvalidQuery,
        })
    })?;
    Ok(status)
}

fn sha256_file(path: &Path) -> Result<String, LaunchBoxError> {
    let mut file = BufReader::new(File::open(path)?);
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn enrich_emulator_games(
    state: &DatabaseState,
) -> Result<LaunchBoxEnrichmentResult, DatabaseError> {
    let connection = state
        .connection
        .lock()
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    if state.take_connection_poisoned() {
        state.log(
            "launchbox-database",
            "launchbox_database_mutex_poisoned",
            "operation=enrich_emulator_games",
        );
        state.log(
            "launchbox-database",
            "launchbox_database_mutex_recovered",
            "operation=enrich_emulator_games",
        );
    }
    let version = active_catalog_version(&connection).map_err(|_| rusqlite::Error::InvalidQuery)?;
    let Some(version) = version else {
        return Ok(LaunchBoxEnrichmentResult {
            resolved: 0,
            exact: 0,
            high: 0,
            ambiguous: 0,
            unresolved: 0,
        });
    };
    let mut games = connection
        .prepare("SELECT id, title, platform, title_id FROM games WHERE source = 'emulator'")?;
    let rows = games
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    let mut result = LaunchBoxEnrichmentResult {
        resolved: 0,
        exact: 0,
        high: 0,
        ambiguous: 0,
        unresolved: 0,
    };
    for (game_id, title, platform, title_id) in rows {
        let cached_negative = connection.query_row("SELECT status FROM launchbox_negative_matches WHERE game_id = ?1 AND expires_at > ?2", params![game_id, now_seconds().to_string()], |row| row.get::<_, String>(0)).optional()?;
        if let Some(status) = cached_negative.as_deref() {
            state.log(
                "launchbox-identity",
                "metadata_identity_negative_cache_hit",
                &format!(
                    "game_id={game_id} title_id={} status={status}",
                    title_id.as_deref().unwrap_or("<none>")
                ),
            );
        }
        let candidate = if let Some(status) = cached_negative {
            Some(MatchCandidate {
                provider_game_id: String::new(),
                confidence: if status == "ambiguous" {
                    MatchConfidence::Ambiguous
                } else {
                    MatchConfidence::Unresolved
                },
            })
        } else {
            resolve_match(
                &connection,
                &version,
                title_id.as_deref(),
                &title,
                &platform,
            )?
        };
        let confidence = candidate
            .as_ref()
            .map(|value| value.confidence)
            .unwrap_or(MatchConfidence::Unresolved);
        if confidence == MatchConfidence::Exact || confidence == MatchConfidence::High {
            result.resolved += 1;
            match confidence {
                MatchConfidence::Exact => result.exact += 1,
                MatchConfidence::High => result.high += 1,
                _ => {}
            }
            if let (Some(title_id), Some(candidate)) = (title_id.as_deref(), candidate.as_ref()) {
                connection.execute("INSERT INTO external_identity_mappings(platform, native_id, provider, provider_game_id, confidence, resolved_at) VALUES (?1, ?2, 'launchbox', ?3, ?4, ?5) ON CONFLICT(platform, native_id, provider) DO UPDATE SET provider_game_id=excluded.provider_game_id, confidence=excluded.confidence, resolved_at=excluded.resolved_at", params![normalize_platform(&platform), title_id, candidate.provider_game_id, candidate.confidence.as_str(), now_seconds().to_string()])?;
            }
            state.log(
                "launchbox-identity",
                if confidence == MatchConfidence::Exact {
                    "metadata_match_exact"
                } else {
                    "metadata_match_fuzzy"
                },
                &format!(
                    "game_id={game_id} title_id={} launchbox_id={} confidence={}",
                    title_id.as_deref().unwrap_or("<none>"),
                    candidate
                        .as_ref()
                        .map(|value| value.provider_game_id.as_str())
                        .unwrap_or("<none>"),
                    confidence.as_str()
                ),
            );
            connection.execute(
                "DELETE FROM launchbox_negative_matches WHERE game_id = ?1",
                params![game_id],
            )?;
        } else if confidence == MatchConfidence::Ambiguous {
            result.ambiguous += 1;
            connection.execute("INSERT INTO launchbox_negative_matches(game_id, platform, normalized_title, status, expires_at, updated_at) VALUES (?1, ?2, ?3, 'ambiguous', ?4, ?5) ON CONFLICT(game_id) DO UPDATE SET status='ambiguous', expires_at=excluded.expires_at, updated_at=excluded.updated_at", params![game_id, normalize_platform(&platform), normalize_title(&title), (now_seconds() + 7 * 24 * 60 * 60).to_string(), now_seconds().to_string()])?;
            state.log(
                "launchbox-identity",
                "metadata_match_ambiguous",
                &format!("game_id={game_id}"),
            );
        } else {
            result.unresolved += 1;
            connection.execute("INSERT INTO launchbox_negative_matches(game_id, platform, normalized_title, status, expires_at, updated_at) VALUES (?1, ?2, ?3, 'unresolved', ?4, ?5) ON CONFLICT(game_id) DO UPDATE SET status='unresolved', expires_at=excluded.expires_at, updated_at=excluded.updated_at", params![game_id, normalize_platform(&platform), normalize_title(&title), (now_seconds() + 7 * 24 * 60 * 60).to_string(), now_seconds().to_string()])?;
            state.log(
                "launchbox-identity",
                "metadata_unresolved",
                &format!("game_id={game_id}"),
            );
        }
    }
    Ok(result)
}

fn enrich_single_emulator_game(
    state: &DatabaseState,
    connection: &Connection,
    version: &str,
    game_id: &str,
    title: &str,
    platform: &str,
    title_id: Option<&str>,
) -> Result<MatchConfidence, String> {
    connection
        .execute(
            "DELETE FROM launchbox_negative_matches WHERE game_id = ?1",
            params![game_id],
        )
        .map_err(|error| error.to_string())?;
    let candidate = resolve_match(connection, version, title_id, title, platform)
        .map_err(|error| error.to_string())?;
    let confidence = candidate
        .as_ref()
        .map(|value| value.confidence)
        .unwrap_or(MatchConfidence::Unresolved);
    if matches!(confidence, MatchConfidence::Exact | MatchConfidence::High) {
        if let (Some(native_id), Some(candidate)) = (title_id, candidate.as_ref()) {
            connection
                .execute(
                    "INSERT INTO external_identity_mappings(platform, native_id, provider, provider_game_id, confidence, resolved_at) VALUES (?1, ?2, 'launchbox', ?3, ?4, ?5) ON CONFLICT(platform, native_id, provider) DO UPDATE SET provider_game_id=excluded.provider_game_id, confidence=excluded.confidence, resolved_at=excluded.resolved_at",
                    params![
                        normalize_platform(platform),
                        native_id,
                        candidate.provider_game_id,
                        candidate.confidence.as_str(),
                        now_seconds().to_string()
                    ],
                )
                .map_err(|error| error.to_string())?;
        }
        state.log(
            "launchbox-identity",
            if confidence == MatchConfidence::Exact {
                "metadata_match_exact"
            } else {
                "metadata_match_fuzzy"
            },
            &format!(
                "game_id={game_id} title_id={} launchbox_id={} confidence={}",
                title_id.unwrap_or("<none>"),
                candidate
                    .as_ref()
                    .map(|value| value.provider_game_id.as_str())
                    .unwrap_or("<none>"),
                confidence.as_str()
            ),
        );
    } else {
        let status = if confidence == MatchConfidence::Ambiguous {
            "ambiguous"
        } else {
            "unresolved"
        };
        connection
            .execute(
                "INSERT INTO launchbox_negative_matches(game_id, platform, normalized_title, status, expires_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(game_id) DO UPDATE SET status=excluded.status, expires_at=excluded.expires_at, updated_at=excluded.updated_at",
                params![
                    game_id,
                    normalize_platform(platform),
                    normalize_title(title),
                    status,
                    (now_seconds() + 7 * 24 * 60 * 60).to_string(),
                    now_seconds().to_string()
                ],
            )
            .map_err(|error| error.to_string())?;
        state.log(
            "launchbox-identity",
            if confidence == MatchConfidence::Ambiguous {
                "metadata_match_ambiguous"
            } else {
                "metadata_unresolved"
            },
            &format!("game_id={game_id}"),
        );
    }
    Ok(confidence)
}

pub async fn refresh_game_metadata(
    state: &DatabaseState,
    game_id: &str,
) -> Result<LaunchBoxGameRefreshResult, String> {
    state.log(
        "game-metadata",
        "game_metadata_refresh_requested",
        &format!("game_id={game_id} source=emulator"),
    );
    let (catalog_phase, active_version, _, _) = catalog_runtime_snapshot(state);
    if catalog_phase == LaunchBoxCatalogPhase::Updating && active_version.is_none() {
        state.log(
            "launchbox-catalog",
            "launchbox_catalog_read_deferred",
            &format!("game_id={game_id} reason=first_import"),
        );
        state.log(
            "launchbox-catalog",
            "launchbox_catalog_not_ready",
            &format!("game_id={game_id}"),
        );
        return Err("LAUNCHBOX_CATALOG_NOT_READY".to_string());
    }
    if catalog_phase == LaunchBoxCatalogPhase::Updating && active_version.is_some() {
        state.log(
            "launchbox-catalog",
            "launchbox_catalog_previous_version_used",
            &format!("game_id={game_id}"),
        );
    }
    let confidence = {
        let connection = lock_connection(state, "refresh_game_metadata")?;
        let version = active_catalog_version(&connection)
            .map_err(|_| "LAUNCHBOX_CATALOG_UNAVAILABLE".to_string())?
            .ok_or_else(|| "LAUNCHBOX_CATALOG_UNAVAILABLE".to_string())?;
        let game = connection
            .query_row(
                "SELECT title, platform, title_id FROM games WHERE id = ?1 AND source = 'emulator'",
                params![game_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|_| "GAME_NOT_FOUND".to_string())?
            .ok_or_else(|| "GAME_NOT_FOUND".to_string())?;
        let confidence = enrich_single_emulator_game(
            state,
            &connection,
            &version,
            game_id,
            &game.0,
            &game.1,
            game.2.as_deref(),
        )?;
        confidence
    };
    let metadata_resolved = matches!(confidence, MatchConfidence::Exact | MatchConfidence::High);
    let report = if metadata_resolved {
        download_screenshots_with_report(state, game_id).await?
    } else {
        LaunchBoxScreenshotReport {
            paths: Vec::new(),
            cached: 0,
            downloaded: 0,
            failed: 0,
        }
    };
    let status = if !metadata_resolved || report.failed > 0 {
        "partial"
    } else {
        "success"
    };
    let result = LaunchBoxGameRefreshResult {
        status: status.to_string(),
        metadata_resolved,
        screenshots_resolved: report.paths.len() as i64,
        screenshots_cached: report.cached,
        screenshots_downloaded: report.downloaded,
        screenshots_failed: report.failed,
        confidence: confidence.as_str().to_string(),
    };
    state.log(
        "game-metadata",
        if result.status == "partial" {
            "game_metadata_refresh_partial"
        } else {
            "game_metadata_refresh_completed"
        },
        &format!(
            "game_id={game_id} metadata_resolved={} confidence={} screenshots_failed={}",
            result.metadata_resolved, result.confidence, result.screenshots_failed
        ),
    );
    Ok(result)
}

fn resolve_match(
    connection: &Connection,
    version: &str,
    title_id: Option<&str>,
    title: &str,
    platform: &str,
) -> Result<Option<MatchCandidate>, rusqlite::Error> {
    if let Some(title_id) = title_id {
        if let Some(provider_game_id) = connection.query_row("SELECT m.provider_game_id FROM external_identity_mappings m JOIN launchbox_games g ON g.provider_game_id = m.provider_game_id AND g.catalog_version = ?3 WHERE m.platform = ?1 AND m.native_id = ?2 AND m.provider = 'launchbox' AND m.confidence IN ('exact', 'high')", params![normalize_platform(platform), title_id, version], |row| row.get::<_, String>(0)).optional()? {
            return Ok(Some(MatchCandidate { provider_game_id, confidence: MatchConfidence::Exact }));
        }
    }
    let normalized_title = normalize_title(title);
    let normalized_platform = normalize_platform(platform);
    let mut statement = connection.prepare("SELECT provider_game_id FROM launchbox_games WHERE catalog_version = ?1 AND normalized_platform = ?2 AND normalized_title = ?3")?;
    let exact = statement
        .query_map(
            params![version, normalized_platform, normalized_title],
            |row| row.get::<_, String>(0),
        )?
        .collect::<Result<Vec<_>, _>>()?;
    if exact.len() == 1 {
        return Ok(Some(MatchCandidate {
            provider_game_id: exact[0].clone(),
            confidence: MatchConfidence::Exact,
        }));
    }
    if exact.len() > 1 {
        return Ok(Some(MatchCandidate {
            provider_game_id: exact[0].clone(),
            confidence: MatchConfidence::Ambiguous,
        }));
    }
    if title_id.is_some() {
        return Ok(None);
    }
    let mut statement = connection.prepare("SELECT provider_game_id, canonical_title, alternate_titles_json FROM launchbox_games WHERE catalog_version = ?1 AND normalized_platform = ?2")?;
    let mut candidates = statement
        .query_map(params![version, normalized_platform], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter_map(
            |(provider_game_id, canonical_title, alternate_titles_json)| {
                let mut score =
                    title_similarity(&normalized_title, &normalize_title(&canonical_title));
                if let Ok(alternate_titles) =
                    serde_json::from_str::<Vec<String>>(&alternate_titles_json)
                {
                    score = score.max(
                        alternate_titles
                            .iter()
                            .map(|value| {
                                title_similarity(&normalized_title, &normalize_title(value))
                            })
                            .max()
                            .unwrap_or(0),
                    );
                }
                (score >= 85).then_some((provider_game_id, score))
            },
        )
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let Some((provider_game_id, score)) = candidates.first() else {
        return Ok(None);
    };
    if candidates
        .get(1)
        .is_some_and(|candidate| candidate.1 >= *score - 2)
    {
        return Ok(Some(MatchCandidate {
            provider_game_id: provider_game_id.clone(),
            confidence: MatchConfidence::Ambiguous,
        }));
    }
    Ok(Some(MatchCandidate {
        provider_game_id: provider_game_id.clone(),
        confidence: MatchConfidence::High,
    }))
}

fn title_similarity(left: &str, right: &str) -> i64 {
    if left == right {
        return 100;
    }
    if left.contains(right) || right.contains(left) {
        return 92;
    }
    let left_tokens = left.split_whitespace().collect::<HashSet<_>>();
    let right_tokens = right.split_whitespace().collect::<HashSet<_>>();
    if left_tokens.is_empty() || right_tokens.is_empty() {
        return 0;
    }
    let overlap = left_tokens.intersection(&right_tokens).count() as i64;
    (overlap * 100 / left_tokens.len().max(right_tokens.len()) as i64).min(91)
}

fn parse_timestamp(value: &str) -> Option<i64> {
    value.parse::<i64>().ok().or_else(|| {
        chrono::DateTime::parse_from_rfc3339(value)
            .ok()
            .map(|date| date.timestamp())
    })
}
fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn normalizes_platforms_and_genres_without_collapsing_title_identity() {
        assert_eq!(normalize_platform("Nintendo Switch"), "nintendo_switch");
        assert_eq!(normalize_platform("Sony PlayStation 3"), "ps3");
        assert_eq!(normalize_genre("Beat 'em Up"), Some("BeatEmUp".to_string()));
        assert_eq!(
            normalize_genre_values("ActionPlatform"),
            vec!["Action", "Platformer"]
        );
        assert_ne!(
            normalize_title("Super Mario Bros."),
            normalize_title("Super Mario Bros. Wonder")
        );
    }

    #[test]
    fn truncates_unicode_without_splitting_multibyte_characters() {
        let value = "local … é ñ 日本語 😀 players";
        assert_eq!(truncate_unicode(value, 9), "local … é");
        assert_eq!(truncate_unicode(value, 1), "l");
        assert_eq!(truncate_unicode(value, 10_000), value);
        assert_eq!(
            parse_local_player_count(&RawRecord {
                description: Some("local … 4 players 日本語 😀".to_string()),
                ..RawRecord::default()
            }),
            Some(4)
        );
    }

    #[test]
    fn parses_xml_as_records_and_filters_media_types() {
        let xml = br#"<LaunchBox><Game><ID>42</ID><Title>Mario Kart 8 Deluxe</Title><Platform>Nintendo Switch</Platform><Overview>Play locally in up to 4-player multiplayer.</Overview><Genre>Racing</Genre><CommunityStarRating>4.3</CommunityStarRating><CommunityStarRatingTotalVotes>12</CommunityStarRatingTotalVotes><MaxPlayers>12</MaxPlayers><Cooperative>No</Cooperative><Images><Image><Type>Screenshot - Gameplay</Type><URL>shots/a.png</URL></Image><Image><Type>Screenshot - Gameplay</Type><URL>shots/a.png</URL></Image><Image><Type>Box - Front</Type><URL>box/a.png</URL></Image><Image><Type>Clear Logo</Type><URL>logo/a.png</URL></Image></Images></Game></LaunchBox>"#;
        let mut records = Vec::new();
        parse_metadata_xml(Cursor::new(xml), |record| {
            records.push(record);
            Ok(())
        })
        .expect("parse");
        assert_eq!(records.len(), 1);
        let record = &records[0];
        assert_eq!(record.provider_game_id, "42");
        assert_eq!(record.normalized_platform, "nintendo_switch");
        assert_eq!(record.normalized_genres, vec!["Racing"]);
        assert_eq!(record.community_rating_raw, Some(4.3));
        assert_eq!(record.community_rating_scale, Some(5.0));
        assert_eq!(record.local_multiplayer, "true");
        assert_eq!(record.max_local_players, Some(4));
        assert_eq!(record.media[0].media_type, "screenshot");
        assert_eq!(record.media.len(), 3);
        assert_eq!(
            record.media[0].url,
            "https://images.launchbox-app.com/shots/a.png"
        );
    }

    #[test]
    fn parses_launchbox_global_game_images_by_database_id() {
        let xml = br#"<LaunchBox><Game><DatabaseID>129188</DatabaseID><Name>Mario Kart 8 Deluxe</Name><Platform>Nintendo Switch</Platform></Game><GameImage><DatabaseID>129188</DatabaseID><FileName>df6b057b-e555-4af6-9f95-a70b5903630c.jpg</FileName><Type>Screenshot - Gameplay</Type></GameImage><GameImage><DatabaseID>129188</DatabaseID><FileName>box-front.jpg</FileName><Type>Box - Front</Type></GameImage></LaunchBox>"#;
        let mut records = Vec::new();
        let mut media = Vec::new();
        parse_metadata_xml_with_media(
            Cursor::new(xml),
            |record| {
                records.push(record);
                Ok(())
            },
            |provider_game_id, reference| {
                media.push((provider_game_id, reference));
                Ok(())
            },
        )
        .expect("parse");
        assert_eq!(records.len(), 1);
        assert_eq!(media.len(), 2);
        assert_eq!(media[0].0, "129188");
        assert_eq!(media[0].1.media_type, "screenshot");
        assert_eq!(
            media[0].1.url,
            "https://images.launchbox-app.com/df6b057b-e555-4af6-9f95-a70b5903630c.jpg"
        );
        assert_eq!(media[1].1.media_type, "box_front");
    }

    #[test]
    fn rejects_malformed_xml() {
        let result = parse_metadata_xml(Cursor::new(br#"<LaunchBox><Game>"#), |_| Ok(()));
        assert!(result.is_err());
    }

    fn match_database() -> Connection {
        let connection = Connection::open_in_memory().expect("connection");
        connection
            .execute_batch(
                "CREATE TABLE launchbox_games(
                    provider_game_id TEXT NOT NULL,
                    catalog_version TEXT NOT NULL,
                    canonical_title TEXT NOT NULL,
                    normalized_title TEXT NOT NULL,
                    alternate_titles_json TEXT NOT NULL,
                    normalized_platform TEXT NOT NULL
                );
                CREATE TABLE external_identity_mappings(
                    platform TEXT NOT NULL,
                    native_id TEXT NOT NULL,
                    provider TEXT NOT NULL,
                    provider_game_id TEXT NOT NULL,
                    confidence TEXT NOT NULL,
                    resolved_at TEXT NOT NULL
                );",
            )
            .expect("schema");
        connection
    }

    fn insert_match_game(
        connection: &Connection,
        provider_game_id: &str,
        title: &str,
        alternate_titles: &[&str],
    ) {
        connection
            .execute(
                "INSERT INTO launchbox_games(provider_game_id, catalog_version, canonical_title, normalized_title, alternate_titles_json, normalized_platform) VALUES (?1, 'v1', ?2, ?3, ?4, 'nintendo_switch')",
                params![
                    provider_game_id,
                    title,
                    normalize_title(title),
                    serde_json::to_string(alternate_titles).expect("alternate titles")
                ],
            )
            .expect("game");
    }

    #[test]
    fn preserves_strong_identity_and_never_falls_back_to_fuzzy_matching() {
        let connection = match_database();
        insert_match_game(&connection, "lb-exact", "Mario Kart 8 Deluxe Edition", &[]);
        assert!(resolve_match(
            &connection,
            "v1",
            Some("0100000000010000"),
            "Mario Kart 8 Deluxe",
            "Nintendo Switch"
        )
        .expect("match")
        .is_none());

        connection
            .execute(
                "INSERT INTO launchbox_games(provider_game_id, catalog_version, canonical_title, normalized_title, alternate_titles_json, normalized_platform) VALUES ('lb-exact-title', 'v1', 'Mario Kart 8 Deluxe', 'mario kart 8 deluxe', '[]', 'nintendo_switch')",
                [],
            )
            .expect("exact game");
        assert_eq!(
            resolve_match(
                &connection,
                "v1",
                Some("0100000000010000"),
                "Mario Kart 8 Deluxe",
                "Nintendo Switch"
            )
            .expect("exact match")
            .map(|candidate| (candidate.provider_game_id, candidate.confidence)),
            Some(("lb-exact-title".to_string(), MatchConfidence::Exact))
        );
    }

    #[test]
    fn classifies_high_ambiguous_and_unresolved_matches() {
        let connection = match_database();
        insert_match_game(&connection, "lb-high", "Super Mario Odyssey", &[]);
        assert_eq!(
            resolve_match(
                &connection,
                "v1",
                None,
                "Super Mario Odyssey (USA)",
                "Nintendo Switch"
            )
            .expect("high match")
            .map(|candidate| candidate.confidence),
            Some(MatchConfidence::High)
        );

        insert_match_game(
            &connection,
            "lb-ambiguous-a",
            "Mario Party Deluxe Edition",
            &[],
        );
        insert_match_game(
            &connection,
            "lb-ambiguous-b",
            "Mario Party Deluxe Collection",
            &[],
        );
        assert_eq!(
            resolve_match(
                &connection,
                "v1",
                None,
                "Mario Party Deluxe",
                "Nintendo Switch"
            )
            .expect("ambiguous match")
            .map(|candidate| candidate.confidence),
            Some(MatchConfidence::Ambiguous)
        );
        assert!(resolve_match(
            &connection,
            "v1",
            None,
            "The Legend of Zelda",
            "Nintendo Switch"
        )
        .expect("unresolved match")
        .is_none());
    }

    #[test]
    fn identity_mapping_is_platform_scoped_and_negative_cache_is_distinguishable_from_zero_rating()
    {
        let connection = match_database();
        connection
            .execute(
                "INSERT INTO external_identity_mappings(platform, native_id, provider, provider_game_id, confidence, resolved_at) VALUES ('nintendo_switch', '0100000000010000', 'launchbox', 'lb-1', 'exact', 'now')",
                [],
            )
            .expect("mapping");
        let mapping: (String, String) = connection
            .query_row(
                "SELECT platform, provider_game_id FROM external_identity_mappings WHERE native_id = '0100000000010000'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("mapping row");
        assert_eq!(mapping, ("nintendo_switch".to_string(), "lb-1".to_string()));

        connection
            .execute_batch(
                "CREATE TABLE launchbox_negative_matches(
                    game_id TEXT PRIMARY KEY,
                    status TEXT NOT NULL CHECK(status IN ('ambiguous', 'unresolved')),
                    expires_at TEXT NOT NULL
                );
                INSERT INTO launchbox_negative_matches(game_id, status, expires_at) VALUES ('game-1', 'unresolved', '9999999999');",
            )
            .expect("negative cache");
        let negative_status: String = connection
            .query_row(
                "SELECT status FROM launchbox_negative_matches WHERE game_id = 'game-1'",
                [],
                |row| row.get(0),
            )
            .expect("negative cache row");
        assert_eq!(negative_status, "unresolved");
        assert!(parse_rating(None).0.is_none());
        assert_eq!(parse_rating(Some("0 / 5")).0, Some(0.0));
    }
}
