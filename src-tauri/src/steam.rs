use crate::achievements::{rarity_from_percentage, Achievement};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, OnceLock,
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

const STEAM_API_BASE_URL: &str = "https://api.steampowered.com";
const STEAM_STORE_BASE_URL: &str = "https://store.steampowered.com";
const MAX_SCREENSHOTS_PER_GAME: usize = 8;
const STEAM_STORE_REFRESH_CONCURRENCY: usize = 2;
const HORIZONTAL_COVER_MIN_WIDTH: i64 = 920;
const HORIZONTAL_COVER_MIN_HEIGHT: i64 = 430;
const VERTICAL_COVER_MIN_WIDTH: i64 = 600;
const VERTICAL_COVER_MIN_HEIGHT: i64 = 900;
const HERO_MIN_WIDTH: i64 = 3840;
const HERO_MIN_HEIGHT: i64 = 1240;

#[derive(Debug, Error)]
pub enum SteamError {
    #[error("Steam is unreachable")]
    Offline,
    #[error("Steam API returned HTTP status {0}")]
    Api(u16),
    #[error("Steam returned an invalid response")]
    InvalidResponse,
    #[error("Steam request could not be created")]
    RequestSetup,
    #[error("Steam synchronization was cancelled")]
    Cancelled,
}

#[derive(Debug, Clone)]
pub struct SteamInstallation {
    pub manifest_path: PathBuf,
    pub install_dir: PathBuf,
}

pub fn resolve_steam_installation(app_id: i64) -> Option<SteamInstallation> {
    if app_id <= 0 {
        return None;
    }
    let manifest_name = format!("appmanifest_{app_id}.acf");
    for root in steam_library_roots() {
        let manifest_path = root.join("steamapps").join(&manifest_name);
        if !manifest_path.is_file() {
            continue;
        }
        let Ok(contents) = fs::read_to_string(&manifest_path) else {
            continue;
        };
        let install_dir_name = parse_vdf_value(&contents, "installdir")?;
        let install_dir = root.join("steamapps/common").join(install_dir_name);
        if install_dir.is_dir() {
            return Some(SteamInstallation {
                manifest_path,
                install_dir: install_dir.canonicalize().unwrap_or(install_dir),
            });
        }
    }
    None
}

fn parse_vdf_value(contents: &str, key: &str) -> Option<String> {
    contents.lines().find_map(|line| {
        let values = line
            .split('"')
            .enumerate()
            .filter_map(|(index, value)| (index % 2 == 1).then_some(value))
            .collect::<Vec<_>>();
        (values.len() >= 2 && values[0].eq_ignore_ascii_case(key)).then(|| values[1].to_string())
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamProfile {
    pub steam_id64: String,
    pub avatar_url: String,
    pub persona_name: String,
    pub country_code: Option<String>,
    pub game_count: u32,
}

#[derive(Debug, Clone)]
pub struct SteamFriendPresence {
    pub steam_id64: String,
    pub persona_name: String,
    pub avatar_url: String,
    pub persona_state: String,
    pub game_name: Option<String>,
    pub game_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SteamLibraryGame {
    pub app_id: i64,
    pub name: String,
    pub total_playtime_minutes: i64,
    pub playtime_2weeks_minutes: Option<i64>,
    pub last_played_at: Option<String>,
    pub installed: Option<bool>,
    pub has_community_visible_stats: Option<bool>,
    pub icon_url: Option<String>,
    pub logo_url: Option<String>,
    pub should_persist: bool,
    pub details: Option<SteamGameDetails>,
}

#[derive(Debug, Clone, Default)]
pub struct SteamGameDetails {
    pub complete: bool,
    pub name: Option<String>,
    pub app_id: i64,
    pub tags: Vec<String>,
    pub genres: Vec<String>,
    pub categories: Vec<SteamNamedValue>,
    pub developers: Vec<String>,
    pub publishers: Vec<String>,
    pub languages: Vec<String>,
    pub platforms: Vec<String>,
    pub controller_support: Option<String>,
    pub release_date: Option<String>,
    pub description: Option<String>,
    pub short_description: Option<String>,
    pub website: Option<String>,
    pub minimum_requirements: Option<Value>,
    pub recommended_requirements: Option<Value>,
    pub header_url: Option<String>,
    pub background_url: Option<String>,
    pub movies: Vec<SteamMedia>,
    pub screenshots: Vec<SteamMedia>,
    pub review_score: Option<i64>,
    pub review_count: Option<i64>,
    pub review_score_description: Option<String>,
    pub price: Option<Value>,
    pub dlc: Vec<i64>,
    pub early_access: Option<bool>,
    pub adult_content: Option<bool>,
    pub multiplayer: Option<bool>,
    pub single_player: Option<bool>,
    pub cloud: Option<bool>,
    pub trading_cards: Option<bool>,
    pub workshop: Option<bool>,
    pub family_sharing: Option<bool>,
    pub achievements: Vec<SteamAchievement>,
    pub achievement_total: Option<i64>,
    pub stats: Vec<SteamStat>,
    pub assets: Vec<SteamImageAsset>,
}

#[derive(Debug, Clone)]
pub struct SteamNamedValue {
    pub id: Option<i64>,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct SteamMedia {
    pub media_type: &'static str,
    pub external_id: String,
    pub name: Option<String>,
    pub thumbnail_url: Option<String>,
    pub full_url: Option<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone)]
pub struct SteamImageAsset {
    pub asset_type: String,
    pub external_id: String,
    pub source_url: String,
}

#[derive(Debug, Clone)]
pub struct SteamImageSource {
    pub game_id: String,
    pub app_id: i64,
    pub asset_type: String,
    pub external_id: String,
    pub source_url: String,
    pub local_path: Option<String>,
    pub mime_type: Option<String>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub byte_size: Option<i64>,
    pub downloaded_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SteamImageRecord {
    pub game_id: String,
    pub asset_type: String,
    pub external_id: String,
    pub source_url: String,
    pub local_path: String,
    pub mime_type: String,
    pub width: i64,
    pub height: i64,
    pub byte_size: i64,
    pub downloaded_at: String,
}

#[derive(Debug, Default)]
pub struct SteamImageDownloadBatch {
    pub records: Vec<SteamImageRecord>,
    pub failures: Vec<SteamImageDownloadFailure>,
    pub skipped_count: usize,
    pub screenshot_source_count: usize,
    pub screenshot_downloaded_count: usize,
    pub screenshot_skipped_count: usize,
}

#[derive(Debug, Clone)]
pub struct SteamImageDownloadFailure {
    pub app_id: i64,
    pub asset_type: String,
    pub external_id: String,
    pub source_url: String,
    pub reason: String,
}

#[derive(Debug)]
struct SteamImageDownloadAttempt {
    record: Option<SteamImageRecord>,
    failure: Option<SteamImageDownloadFailure>,
}

#[derive(Debug, Default)]
pub struct SteamImageSourceRefreshResult {
    pub sources: Vec<SteamImageSource>,
    pub failed_app_ids: Vec<i64>,
    pub failed_errors: Vec<String>,
    pub requested_app_count: usize,
    pub returned_app_count: usize,
    pub screenshot_source_count: usize,
    pub apps_with_screenshots: usize,
}

pub type SteamAchievement = Achievement;

#[derive(Debug, Clone)]
pub struct SteamStat {
    pub name: String,
    pub value: Value,
}

#[derive(Debug, Deserialize)]
struct PlayerSummaryResponse {
    response: PlayerSummaryContainer,
}
#[derive(Debug, Deserialize)]
struct PlayerSummaryContainer {
    players: Vec<PlayerSummary>,
}
#[derive(Debug, Deserialize)]
struct PlayerSummary {
    steamid: String,
    personaname: String,
    avatarfull: String,
    #[serde(default)]
    loccountrycode: Option<String>,
    #[serde(default)]
    personastate: Option<i64>,
    #[serde(default)]
    gameid: Option<String>,
    #[serde(default)]
    gameextrainfo: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FriendsListResponse {
    #[serde(default)]
    friendslist: Option<FriendsListContainer>,
}

#[derive(Debug, Deserialize, Default)]
struct FriendsListContainer {
    #[serde(default)]
    friends: Vec<SteamFriendLink>,
}

#[derive(Debug, Deserialize)]
struct SteamFriendLink {
    steamid: String,
}

#[derive(Debug, Deserialize)]
struct OwnedGamesResponse {
    response: OwnedGamesContainer,
}
#[derive(Debug, Deserialize, Default)]
struct OwnedGamesContainer {
    #[serde(default)]
    game_count: u32,
    #[serde(default)]
    games: Vec<OwnedGame>,
}
#[derive(Debug, Clone, Deserialize)]
struct OwnedGame {
    appid: i64,
    #[serde(default)]
    name: String,
    #[serde(default)]
    playtime_forever: i64,
    #[serde(default)]
    playtime_2weeks: Option<i64>,
    #[serde(default)]
    rtime_last_played: Option<i64>,
    #[serde(default)]
    has_community_visible_stats: Option<bool>,
    #[serde(default)]
    img_icon_url: Option<String>,
    #[serde(default)]
    img_logo_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AppDetailsEnvelope {
    success: bool,
    #[serde(default)]
    data: Option<AppDetails>,
}
#[derive(Debug, Deserialize, Default)]
struct AppDetails {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    steam_appid: Option<i64>,
    #[serde(default)]
    controller_support: Option<String>,
    #[serde(default)]
    dlc: Vec<i64>,
    #[serde(default)]
    detailed_description: Option<String>,
    #[serde(default)]
    about_the_game: Option<String>,
    #[serde(default)]
    short_description: Option<String>,
    #[serde(default)]
    supported_languages: Option<String>,
    #[serde(default)]
    header_image: Option<String>,
    #[serde(default)]
    background: Option<String>,
    #[serde(default)]
    background_raw: Option<String>,
    #[serde(default)]
    website: Option<String>,
    #[serde(default)]
    pc_requirements: Option<Value>,
    #[serde(default)]
    developers: Vec<String>,
    #[serde(default)]
    publishers: Vec<String>,
    #[serde(default)]
    categories: Vec<NamedItem>,
    #[serde(default)]
    genres: Vec<NamedItem>,
    #[serde(default)]
    screenshots: Vec<StoreScreenshot>,
    #[serde(default)]
    movies: Vec<StoreMovie>,
    #[serde(default)]
    achievements: Option<StoreAchievements>,
    #[serde(default)]
    release_date: Option<ReleaseDate>,
    #[serde(default)]
    platforms: Option<PlatformFlags>,
    #[serde(default)]
    price_overview: Option<Value>,
    #[serde(default)]
    required_age: Option<Value>,
    #[serde(default)]
    content_descriptors: Option<ContentDescriptors>,
    #[serde(default)]
    tags: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct StoreImageDetailsEnvelope {
    success: bool,
    #[serde(default)]
    data: Option<StoreImageDetails>,
}

#[derive(Debug, Deserialize, Default)]
struct StoreImageDetails {
    #[serde(default)]
    header_image: Option<String>,
    #[serde(default)]
    background: Option<String>,
    #[serde(default)]
    background_raw: Option<String>,
    #[serde(default)]
    screenshots: Vec<StoreScreenshot>,
}

#[derive(Debug, Deserialize)]
struct NamedItem {
    id: Option<Value>,
    #[serde(alias = "description")]
    name: String,
}
#[derive(Debug, Deserialize)]
struct StoreScreenshot {
    id: i64,
    path_thumbnail: Option<String>,
    path_full: Option<String>,
}
#[derive(Debug, Deserialize)]
struct StoreMovie {
    id: i64,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    thumbnail: Option<String>,
    #[serde(default)]
    webm: Option<HashMap<String, String>>,
    #[serde(default)]
    mp4: Option<HashMap<String, String>>,
    #[serde(default)]
    hls_h264: Option<String>,
    #[serde(default)]
    highlight: Option<bool>,
}
#[derive(Debug, Deserialize)]
struct StoreAchievements {
    total: Option<i64>,
}
#[derive(Debug, Deserialize)]
struct ReleaseDate {
    date: Option<String>,
}
#[derive(Debug, Deserialize)]
struct PlatformFlags {
    windows: bool,
    mac: bool,
    linux: bool,
}
#[derive(Debug, Deserialize)]
struct ContentDescriptors {
    #[serde(default)]
    notes: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ReviewsResponse {
    success: Option<u32>,
    query_summary: Option<ReviewSummary>,
}

#[derive(Debug, Deserialize)]
struct CurrentPlayersResponse {
    response: CurrentPlayers,
}

#[derive(Debug, Deserialize)]
struct CurrentPlayers {
    player_count: Option<i64>,
}
#[derive(Debug, Deserialize)]
struct ReviewSummary {
    review_score: Option<i64>,
    total_reviews: Option<i64>,
    review_score_desc: Option<String>,
}
#[derive(Debug, Deserialize)]
struct SchemaResponse {
    game: Option<GameSchema>,
}
#[derive(Debug, Deserialize)]
struct GameSchema {
    #[serde(default, rename = "availableGameStats")]
    available_game_stats: Option<AvailableGameStats>,
}
#[derive(Debug, Deserialize)]
struct AvailableGameStats {
    #[serde(default)]
    achievements: Vec<SchemaAchievement>,
}
#[derive(Debug, Deserialize)]
struct SchemaAchievement {
    apiname: String,
    #[serde(default, rename = "displayName")]
    displayname: Option<String>,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    hidden: Option<i64>,
    #[serde(default)]
    icon: Option<String>,
    #[serde(default, rename = "icongray")]
    icon_gray: Option<String>,
}
#[derive(Debug, Deserialize)]
struct PlayerAchievementsResponse {
    playerstats: Option<PlayerStats>,
}
#[derive(Debug, Deserialize)]
struct PlayerStats {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    achievements: Vec<PlayerAchievement>,
    #[serde(default)]
    stats: Vec<PlayerStat>,
}
#[derive(Debug, Deserialize)]
struct PlayerAchievement {
    apiname: String,
    achieved: Option<i64>,
    unlocktime: Option<i64>,
}
#[derive(Debug, Deserialize)]
struct PlayerStat {
    name: String,
    value: Value,
}

#[derive(Debug, Deserialize)]
struct GlobalAchievementPercentagesResponse {
    #[serde(default, rename = "achievementpercentages")]
    achievement_percentages: Option<GlobalAchievementPercentages>,
}

#[derive(Debug, Deserialize, Default)]
struct GlobalAchievementPercentages {
    #[serde(default)]
    achievements: Vec<GlobalAchievementPercentage>,
}

#[derive(Debug, Deserialize)]
struct GlobalAchievementPercentage {
    name: String,
    percent: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct SteamAchievementSnapshot {
    pub achievements: Vec<SteamAchievement>,
    pub genres: Vec<String>,
    pub total: i64,
    pub stats: Vec<SteamStat>,
}

pub async fn fetch_profile(steam_id64: &str, api_key: &str) -> Result<SteamProfile, SteamError> {
    fetch_profile_from_base(STEAM_API_BASE_URL, steam_id64, api_key).await
}

pub async fn fetch_friends_playing(
    steam_id64: &str,
    api_key: &str,
    app_id: i64,
) -> Result<Vec<SteamFriendPresence>, SteamError> {
    let client = build_client()?;
    let friends_response: FriendsListResponse = request_json(
        &client,
        &format!(
            "{}/ISteamUser/GetFriendList/v0001/",
            STEAM_API_BASE_URL.trim_end_matches('/')
        ),
        &[
            ("key", api_key),
            ("steamid", steam_id64),
            ("relationship", "friend"),
            ("format", "json"),
        ],
    )
    .await?;
    let friend_ids = friends_response
        .friendslist
        .unwrap_or_default()
        .friends
        .into_iter()
        .map(|friend| friend.steamid)
        .collect::<Vec<_>>();
    if friend_ids.is_empty() {
        return Ok(Vec::new());
    }

    let target_app_id = app_id.to_string();
    let mut playing = Vec::new();
    for chunk in friend_ids.chunks(100) {
        let steam_ids = chunk.join(",");
        let response: PlayerSummaryResponse = request_json(
            &client,
            &format!(
                "{}/ISteamUser/GetPlayerSummaries/v0002/",
                STEAM_API_BASE_URL.trim_end_matches('/')
            ),
            &[
                ("key", api_key),
                ("steamids", &steam_ids),
                ("format", "json"),
            ],
        )
        .await?;
        playing.extend(
            response
                .response
                .players
                .into_iter()
                .filter(|player| player.gameid.as_deref() == Some(target_app_id.as_str()))
                .map(|player| SteamFriendPresence {
                    steam_id64: player.steamid,
                    persona_name: player.personaname,
                    avatar_url: player.avatarfull,
                    persona_state: steam_persona_state(player.personastate),
                    game_name: player.gameextrainfo,
                    game_id: player.gameid,
                }),
        );
    }
    Ok(playing)
}

fn steam_persona_state(value: Option<i64>) -> String {
    match value.unwrap_or(0) {
        1 => "online".to_string(),
        2 => "busy".to_string(),
        3 => "away".to_string(),
        4 => "snooze".to_string(),
        5 => "looking-to-trade".to_string(),
        6 => "looking-to-play".to_string(),
        _ => "offline".to_string(),
    }
}

async fn fetch_profile_from_base(
    base_url: &str,
    steam_id64: &str,
    api_key: &str,
) -> Result<SteamProfile, SteamError> {
    let client = build_client()?;
    let player_summary: PlayerSummaryResponse = request_json(
        &client,
        &format!(
            "{}/ISteamUser/GetPlayerSummaries/v0002/",
            base_url.trim_end_matches('/')
        ),
        &[
            ("key", api_key),
            ("steamids", steam_id64),
            ("format", "json"),
        ],
    )
    .await?;
    let summary = player_summary
        .response
        .players
        .into_iter()
        .find(|player| player.steamid == steam_id64)
        .filter(|player| !player.personaname.is_empty())
        .ok_or(SteamError::InvalidResponse)?;
    let owned_games: OwnedGamesResponse = request_json(
        &client,
        &format!(
            "{}/IPlayerService/GetOwnedGames/v0001/",
            base_url.trim_end_matches('/')
        ),
        &[
            ("key", api_key),
            ("steamid", steam_id64),
            ("format", "json"),
            ("include_appinfo", "0"),
        ],
    )
    .await?;
    Ok(SteamProfile {
        steam_id64: summary.steamid,
        avatar_url: summary.avatarfull,
        persona_name: summary.personaname,
        country_code: summary.loccountrycode,
        game_count: owned_games.response.game_count,
    })
}

fn steam_library_roots() -> HashSet<PathBuf> {
    let mut library_roots = HashSet::new();
    let mut root_candidates = cached_steam_install_roots().to_vec();
    root_candidates.extend(windows_drive_library_roots());
    for root in root_candidates {
        if !root.is_dir() {
            continue;
        }
        library_roots.insert(root.clone());
        if let Ok(contents) = fs::read_to_string(root.join("steamapps/libraryfolders.vdf")) {
            for line in contents.lines() {
                let values = line
                    .split('"')
                    .enumerate()
                    .filter_map(|(index, value)| (index % 2 == 1).then_some(value))
                    .collect::<Vec<_>>();
                if values.len() >= 2 && values[0] == "path" {
                    library_roots.insert(PathBuf::from(values[1].replace("\\\\", "\\")));
                }
            }
        }
    }
    library_roots
}

pub fn find_installed_app_ids() -> Option<HashSet<i64>> {
    let mut installed = HashSet::new();
    let library_roots = steam_library_roots();
    if library_roots.is_empty() {
        return None;
    }

    for root in library_roots {
        let Ok(entries) = fs::read_dir(root.join("steamapps")) else {
            continue;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(app_id) = name
                .strip_prefix("appmanifest_")
                .and_then(|value| value.strip_suffix(".acf"))
                .and_then(|value| value.parse::<i64>().ok())
            else {
                continue;
            };
            installed.insert(app_id);
        }
    }
    Some(installed)
}

fn steam_install_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for variable in ["PROGRAMFILES(X86)", "PROGRAMFILES", "LOCALAPPDATA"] {
        if let Ok(value) = std::env::var(variable) {
            roots.push(PathBuf::from(value).join("Steam"));
        }
    }
    roots.extend(windows_registry_steam_roots());
    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        roots.push(PathBuf::from(user_profile).join("AppData/Local/Steam"));
    }
    if let Ok(home) = std::env::var("HOME") {
        roots.extend([
            PathBuf::from(&home).join(".steam/steam"),
            PathBuf::from(&home).join(".local/share/Steam"),
        ]);
    }
    roots
}

fn cached_steam_install_roots() -> &'static [PathBuf] {
    static ROOTS: OnceLock<Vec<PathBuf>> = OnceLock::new();
    ROOTS.get_or_init(steam_install_roots).as_slice()
}

#[cfg(windows)]
fn windows_registry_steam_roots() -> Vec<PathBuf> {
    use std::os::windows::process::CommandExt;

    [
        ("HKCU\\Software\\Valve\\Steam", "SteamPath"),
        ("HKLM\\SOFTWARE\\WOW6432Node\\Valve\\Steam", "InstallPath"),
    ]
    .into_iter()
    .filter_map(|(key, value)| {
        let output = std::process::Command::new("reg")
            .args(["query", key, "/v", value])
            .creation_flags(0x08000000)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .find(|line| line.contains(value))
            .and_then(|line| line.split_whitespace().last())
            .map(|path| PathBuf::from(path.replace('/', "\\")))
    })
    .collect()
}

#[cfg(windows)]
fn windows_drive_library_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    for letter in b'A'..=b'Z' {
        let drive = PathBuf::from(format!("{}:\\", char::from(letter)));
        if !drive.is_dir() {
            continue;
        }
        if drive.join("steamapps").is_dir() {
            roots.push(drive.clone());
        }
        let Ok(entries) = fs::read_dir(&drive) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("steamapps").is_dir() {
                roots.push(path);
            }
        }
    }
    roots
}

#[cfg(not(windows))]
fn windows_registry_steam_roots() -> Vec<PathBuf> {
    Vec::new()
}

#[cfg(not(windows))]
fn windows_drive_library_roots() -> Vec<PathBuf> {
    Vec::new()
}

pub async fn fetch_library(
    steam_id64: &str,
    api_key: &str,
    cached_games: &HashMap<i64, (i64, i64)>,
    cancel_requested: Arc<AtomicBool>,
    progress_completed: Arc<AtomicUsize>,
    progress_total: Arc<AtomicUsize>,
    installed_app_ids: Option<&HashSet<i64>>,
) -> Result<Vec<SteamLibraryGame>, SteamError> {
    fetch_library_from_bases(
        STEAM_API_BASE_URL,
        STEAM_STORE_BASE_URL,
        steam_id64,
        api_key,
        cached_games,
        cancel_requested,
        progress_completed,
        progress_total,
        installed_app_ids,
    )
    .await
}

async fn fetch_library_from_bases(
    api_base: &str,
    store_base: &str,
    steam_id64: &str,
    api_key: &str,
    cached_games: &HashMap<i64, (i64, i64)>,
    cancel_requested: Arc<AtomicBool>,
    progress_completed: Arc<AtomicUsize>,
    progress_total: Arc<AtomicUsize>,
    installed_app_ids: Option<&HashSet<i64>>,
) -> Result<Vec<SteamLibraryGame>, SteamError> {
    ensure_not_cancelled(cancel_requested.as_ref())?;
    let client = build_client()?;
    let owned: OwnedGamesResponse = request_json(
        &client,
        &format!(
            "{}/IPlayerService/GetOwnedGames/v0001/",
            api_base.trim_end_matches('/')
        ),
        &[
            ("key", api_key),
            ("steamid", steam_id64),
            ("format", "json"),
            ("include_appinfo", "1"),
            ("include_played_free_games", "1"),
        ],
    )
    .await?;
    let owned_games = owned
        .response
        .games
        .into_iter()
        .filter(|game| {
            installed_app_ids
                .map(|installed| installed.contains(&game.appid))
                .unwrap_or(true)
        })
        .collect::<Vec<_>>();
    progress_completed.store(0, Ordering::SeqCst);
    progress_total.store(owned_games.len(), Ordering::SeqCst);
    let mut games = Vec::with_capacity(owned_games.len());
    for chunk in owned_games.chunks(8) {
        ensure_not_cancelled(cancel_requested.as_ref())?;
        let mut handles = Vec::new();
        for game in chunk
            .iter()
            .filter(|game| should_refresh(game, cached_games))
        {
            ensure_not_cancelled(cancel_requested.as_ref())?;
            let client = client.clone();
            let cancel_requested = Arc::clone(&cancel_requested);
            let api_base = api_base.to_string();
            let store_base = store_base.to_string();
            let api_key = api_key.to_string();
            let steam_id64 = steam_id64.to_string();
            let game = game.clone();
            handles.push((
                game.clone(),
                tauri::async_runtime::spawn(async move {
                    fetch_game_details(
                        &client,
                        &api_base,
                        &store_base,
                        &api_key,
                        &steam_id64,
                        &game,
                        cancel_requested.as_ref(),
                    )
                    .await
                }),
            ));
        }
        for game in chunk {
            ensure_not_cancelled(cancel_requested.as_ref())?;
            let details = if let Some((_, handle)) = handles
                .iter_mut()
                .find(|(candidate, _)| candidate.appid == game.appid)
            {
                Some(handle.await.map_err(|_| SteamError::InvalidResponse)??)
            } else {
                None
            };
            games.push(SteamLibraryGame {
                app_id: game.appid,
                name: game.name.clone(),
                total_playtime_minutes: game.playtime_forever,
                playtime_2weeks_minutes: game.playtime_2weeks,
                last_played_at: game.rtime_last_played.filter(|value| *value > 0).map(|value| value.to_string()),
                installed: installed_app_ids.map(|ids| ids.contains(&game.appid)),
                has_community_visible_stats: game.has_community_visible_stats,
                icon_url: game.img_icon_url.clone().map(|hash| format!("https://media.steampowered.com/steamcommunity/public/images/apps/{}/{hash}.jpg", game.appid)),
                logo_url: game.img_logo_url.clone().map(|hash| format!("https://media.steampowered.com/steamcommunity/public/images/apps/{}/{hash}.jpg", game.appid)),
                should_persist: details.is_some() || !cached_games.contains_key(&game.appid),
                details,
            });
        }
        progress_completed.fetch_add(chunk.len(), Ordering::SeqCst);
    }
    Ok(games)
}

pub async fn fetch_game_achievements(
    steam_id64: &str,
    api_key: &str,
    app_id: i64,
) -> Result<SteamAchievementSnapshot, SteamError> {
    let client = build_client()?;
    let app_id_text = app_id.to_string();
    let store_details = request_json::<HashMap<String, AppDetailsEnvelope>>(
        &client,
        &format!(
            "{}/api/appdetails/",
            STEAM_STORE_BASE_URL.trim_end_matches('/')
        ),
        &[("appids", &app_id_text), ("l", "english")],
    )
    .await
    .ok()
    .and_then(|mut responses| responses.remove(&app_id_text))
    .and_then(|response| response.data);
    let store_total = store_details
        .as_ref()
        .and_then(|details| details.achievements.as_ref())
        .and_then(|achievements| achievements.total);
    let genres = store_details
        .map(|details| {
            details
                .genres
                .into_iter()
                .map(|genre| genre.name)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let schema = request_json::<SchemaResponse>(
        &client,
        &format!(
            "{}/ISteamUserStats/GetSchemaForGame/v2/",
            STEAM_API_BASE_URL.trim_end_matches('/')
        ),
        &[
            ("key", api_key),
            ("appid", &app_id_text),
            ("format", "json"),
        ],
    )
    .await
    .ok()
    .and_then(|response| response.game)
    .and_then(|game| game.available_game_stats);
    let definitions = schema.map(|value| value.achievements).unwrap_or_default();
    let player = request_json::<PlayerAchievementsResponse>(
        &client,
        &format!(
            "{}/ISteamUserStats/GetPlayerAchievements/v0001/",
            STEAM_API_BASE_URL.trim_end_matches('/')
        ),
        &[
            ("key", api_key),
            ("steamid", steam_id64),
            ("appid", &app_id_text),
            ("format", "json"),
        ],
    )
    .await
    .ok()
    .and_then(|response| response.playerstats)
    .filter(|stats| stats.error.is_none());
    let global = request_json::<GlobalAchievementPercentagesResponse>(
        &client,
        &format!(
            "{}/ISteamUserStats/GetGlobalAchievementPercentagesForApp/v0002/",
            STEAM_API_BASE_URL.trim_end_matches('/')
        ),
        &[("gameid", &app_id_text), ("format", "json")],
    )
    .await
    .ok()
    .and_then(|response| response.achievement_percentages)
    .unwrap_or_default();
    let global_by_name = global
        .achievements
        .into_iter()
        .map(|achievement| (achievement.name, achievement.percent))
        .collect::<HashMap<_, _>>();
    let mut player_by_name = HashMap::new();
    for achievement in player
        .as_ref()
        .map(|stats| stats.achievements.as_slice())
        .unwrap_or(&[])
    {
        player_by_name.insert(achievement.apiname.as_str(), achievement);
    }
    let user_stats = request_json::<PlayerAchievementsResponse>(
        &client,
        &format!(
            "{}/ISteamUserStats/GetUserStatsForGame/v0002/",
            STEAM_API_BASE_URL.trim_end_matches('/')
        ),
        &[
            ("key", api_key),
            ("steamid", steam_id64),
            ("appid", &app_id_text),
            ("format", "json"),
        ],
    )
    .await
    .ok()
    .and_then(|response| response.playerstats)
    .filter(|stats| stats.error.is_none());
    let build_achievement = |api_name: String,
                             display_name: Option<String>,
                             description: Option<String>,
                             hidden: bool,
                             icon_unlocked: Option<String>,
                             icon_locked: Option<String>| {
        let player = player_by_name.get(api_name.as_str()).copied();
        let unlock_percentage = global_by_name.get(api_name.as_str()).copied().flatten();
        let rarity = rarity_from_percentage(unlock_percentage);
        let display_name = display_name.unwrap_or_else(|| api_name.clone());
        Achievement {
            api_name,
            display_name,
            description: description.unwrap_or_default(),
            hidden,
            unlocked: player.and_then(|value| value.achieved).unwrap_or(0) > 0,
            unlock_time: player
                .and_then(|value| value.unlocktime)
                .filter(|value| *value > 0)
                .map(|value| value.to_string()),
            unlock_percentage,
            rarity,
            virtual_tier: rarity.virtual_tier(),
            icon_unlocked,
            icon_locked,
            local_icon_unlocked: None,
            local_icon_locked: None,
        }
    };
    let achievements = if definitions.is_empty() {
        player
            .as_ref()
            .map(|stats| stats.achievements.as_slice())
            .unwrap_or(&[])
            .iter()
            .map(|achievement| {
                build_achievement(achievement.apiname.clone(), None, None, false, None, None)
            })
            .collect::<Vec<_>>()
    } else {
        definitions
            .into_iter()
            .map(|definition| {
                build_achievement(
                    definition.apiname,
                    definition.displayname,
                    definition.description,
                    definition.hidden.unwrap_or(0) > 0,
                    definition.icon,
                    definition.icon_gray,
                )
            })
            .collect::<Vec<_>>()
    };
    let stats = user_stats
        .map(|value| {
            value
                .stats
                .into_iter()
                .map(|stat| SteamStat {
                    name: stat.name,
                    value: stat.value,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let total = store_total.unwrap_or(0).max(achievements.len() as i64).max(
        player
            .as_ref()
            .map_or(0, |value| value.achievements.len() as i64),
    );
    if total == 0 && achievements.is_empty() {
        return Ok(SteamAchievementSnapshot {
            achievements,
            genres,
            total,
            stats,
        });
    }
    Ok(SteamAchievementSnapshot {
        achievements,
        genres,
        total,
        stats,
    })
}

pub async fn fetch_game_metadata(
    steam_id64: &str,
    api_key: &str,
    app_id: i64,
    name: &str,
    _logo_url: Option<String>,
    cancel_requested: &AtomicBool,
) -> Result<SteamGameDetails, SteamError> {
    let client = build_client()?;
    let game = OwnedGame {
        appid: app_id,
        name: name.to_string(),
        playtime_forever: 0,
        playtime_2weeks: None,
        rtime_last_played: None,
        has_community_visible_stats: None,
        img_icon_url: None,
        img_logo_url: None,
    };
    fetch_game_details(
        &client,
        STEAM_API_BASE_URL,
        STEAM_STORE_BASE_URL,
        api_key,
        steam_id64,
        &game,
        cancel_requested,
    )
    .await
}

pub async fn fetch_current_players(app_id: i64) -> Result<i64, SteamError> {
    let client = build_client()?;
    let app_id_text = app_id.to_string();
    let response: CurrentPlayersResponse = request_json(
        &client,
        &format!(
            "{}/ISteamUserStats/GetNumberOfCurrentPlayers/v1/",
            STEAM_API_BASE_URL.trim_end_matches('/')
        ),
        &[("appid", &app_id_text)],
    )
    .await?;
    response
        .response
        .player_count
        .ok_or(SteamError::InvalidResponse)
}

async fn fetch_game_details(
    client: &reqwest::Client,
    api_base: &str,
    store_base: &str,
    api_key: &str,
    steam_id64: &str,
    game: &OwnedGame,
    cancel_requested: &AtomicBool,
) -> Result<SteamGameDetails, SteamError> {
    ensure_not_cancelled(cancel_requested)?;
    let app_id = game.appid.to_string();
    let app_details_result = request_json::<HashMap<String, AppDetailsEnvelope>>(
        client,
        &format!("{}/api/appdetails/", store_base.trim_end_matches('/')),
        &[("appids", &app_id), ("l", "english"), ("cc", "us")],
    )
    .await
    .ok()
    .and_then(|mut responses| responses.remove(&game.appid.to_string()))
    .and_then(|response| {
        if response.success {
            response.data
        } else {
            None
        }
    });
    let details_complete = app_details_result.is_some();
    let app_details = app_details_result.unwrap_or_default();
    ensure_not_cancelled(cancel_requested)?;
    let reviews = request_json::<ReviewsResponse>(
        client,
        &format!(
            "{}/appreviews/{}",
            store_base.trim_end_matches('/'),
            game.appid
        ),
        &[("json", "1"), ("language", "all"), ("filter", "all")],
    )
    .await
    .ok()
    .and_then(|response| {
        if response.success.unwrap_or(0) == 1 {
            response.query_summary
        } else {
            None
        }
    });
    ensure_not_cancelled(cancel_requested)?;
    let schema = request_json::<SchemaResponse>(
        client,
        &format!(
            "{}/ISteamUserStats/GetSchemaForGame/v2/",
            api_base.trim_end_matches('/')
        ),
        &[("key", api_key), ("appid", &app_id), ("format", "json")],
    )
    .await
    .ok()
    .and_then(|response| response.game)
    .and_then(|game| game.available_game_stats);
    ensure_not_cancelled(cancel_requested)?;
    let player = request_json::<PlayerAchievementsResponse>(
        client,
        &format!(
            "{}/ISteamUserStats/GetPlayerAchievements/v0001/",
            api_base.trim_end_matches('/')
        ),
        &[
            ("key", api_key),
            ("steamid", steam_id64),
            ("appid", &app_id),
            ("format", "json"),
        ],
    )
    .await
    .ok()
    .and_then(|response| response.playerstats);
    ensure_not_cancelled(cancel_requested)?;
    let user_stats = request_json::<PlayerAchievementsResponse>(
        client,
        &format!(
            "{}/ISteamUserStats/GetUserStatsForGame/v0002/",
            api_base.trim_end_matches('/')
        ),
        &[
            ("key", api_key),
            ("steamid", steam_id64),
            ("appid", &app_id),
            ("format", "json"),
        ],
    )
    .await
    .ok()
    .and_then(|response| response.playerstats);

    let categories = app_details
        .categories
        .iter()
        .map(|item| SteamNamedValue {
            id: item.id.as_ref().and_then(value_as_i64),
            value: item.name.clone(),
        })
        .collect::<Vec<_>>();
    let category_names = categories
        .iter()
        .map(|item| item.value.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let schema_achievements = schema
        .as_ref()
        .map(|value| value.achievements.as_slice())
        .unwrap_or(&[]);
    let player_achievements = player
        .as_ref()
        .map(|value| value.achievements.as_slice())
        .unwrap_or(&[]);
    let mut player_by_name = HashMap::new();
    for achievement in player_achievements {
        player_by_name.insert(achievement.apiname.as_str(), achievement);
    }
    let achievements = schema_achievements
        .iter()
        .map(|definition| {
            let player = player_by_name.get(definition.apiname.as_str()).copied();
            let rarity = rarity_from_percentage(None);
            SteamAchievement {
                api_name: definition.apiname.clone(),
                display_name: definition
                    .displayname
                    .clone()
                    .unwrap_or_else(|| definition.apiname.clone()),
                description: definition.description.clone().unwrap_or_default(),
                hidden: definition.hidden.unwrap_or(0) > 0,
                unlocked: player.and_then(|value| value.achieved).unwrap_or(0) > 0,
                unlock_time: player
                    .and_then(|value| value.unlocktime)
                    .filter(|value| *value > 0)
                    .map(|value| value.to_string()),
                unlock_percentage: None,
                rarity,
                virtual_tier: rarity.virtual_tier(),
                icon_unlocked: definition.icon.clone(),
                icon_locked: definition.icon_gray.clone(),
                local_icon_unlocked: None,
                local_icon_locked: None,
            }
        })
        .collect::<Vec<_>>();
    let stats = user_stats
        .as_ref()
        .map(|value| {
            value
                .stats
                .iter()
                .map(|stat| SteamStat {
                    name: stat.name.clone(),
                    value: stat.value.clone(),
                })
                .collect()
        })
        .unwrap_or_default();
    let mut details = SteamGameDetails {
        complete: details_complete,
        name: app_details.name.clone(),
        app_id: app_details.steam_appid.unwrap_or(game.appid),
        tags: parse_tags(app_details.tags.as_ref()),
        genres: app_details
            .genres
            .iter()
            .map(|item| item.name.clone())
            .collect(),
        categories,
        developers: app_details.developers.clone(),
        publishers: app_details.publishers.clone(),
        languages: parse_languages(app_details.supported_languages.as_deref()),
        platforms: parse_platforms(app_details.platforms.as_ref()),
        controller_support: app_details.controller_support.clone(),
        release_date: app_details
            .release_date
            .as_ref()
            .and_then(|date| date.date.clone()),
        description: app_details
            .detailed_description
            .clone()
            .or(app_details.about_the_game.clone()),
        short_description: app_details.short_description.clone(),
        website: app_details.website.clone(),
        minimum_requirements: requirements_value(app_details.pc_requirements.as_ref(), "minimum"),
        recommended_requirements: requirements_value(
            app_details.pc_requirements.as_ref(),
            "recommended",
        ),
        header_url: app_details.header_image.clone(),
        background_url: app_details
            .background_raw
            .clone()
            .or(app_details.background.clone()),
        movies: app_details.movies.iter().map(movie_to_media).collect(),
        screenshots: app_details
            .screenshots
            .iter()
            .map(|shot| SteamMedia {
                media_type: "screenshot",
                external_id: shot.id.to_string(),
                name: None,
                thumbnail_url: shot.path_thumbnail.clone(),
                full_url: shot.path_full.clone(),
                metadata: Value::Null,
            })
            .collect(),
        review_score: reviews.as_ref().and_then(|value| value.review_score),
        review_count: reviews.as_ref().and_then(|value| value.total_reviews),
        review_score_description: reviews
            .as_ref()
            .and_then(|value| value.review_score_desc.clone()),
        price: app_details.price_overview.clone(),
        dlc: app_details.dlc.clone(),
        early_access: Some(
            category_names
                .iter()
                .any(|value| value.contains("early access")),
        ),
        adult_content: Some(
            app_details
                .required_age
                .as_ref()
                .and_then(value_as_i64)
                .unwrap_or(0)
                >= 18
                || app_details
                    .content_descriptors
                    .as_ref()
                    .and_then(|value| value.notes.as_deref())
                    .map(|value| value.to_ascii_lowercase().contains("adult"))
                    .unwrap_or(false),
        ),
        multiplayer: Some(category_names.iter().any(|value| {
            value.contains("multi-player")
                || value.contains("multiplayer")
                || value.contains("co-op")
        })),
        single_player: Some(
            category_names
                .iter()
                .any(|value| value.contains("single-player") || value.contains("single player")),
        ),
        cloud: Some(
            category_names
                .iter()
                .any(|value| value.contains("steam cloud")),
        ),
        trading_cards: Some(
            category_names
                .iter()
                .any(|value| value.contains("trading cards")),
        ),
        workshop: Some(
            category_names
                .iter()
                .any(|value| value.contains("steam workshop")),
        ),
        family_sharing: Some(
            category_names
                .iter()
                .any(|value| value.contains("family sharing")),
        ),
        achievements,
        achievement_total: app_details
            .achievements
            .as_ref()
            .and_then(|value| value.total),
        stats,
        assets: image_assets_for(
            game.appid,
            app_details.header_image.as_deref(),
            app_details.background.as_deref(),
            game.img_logo_url.as_deref().map(|hash| {
                format!(
                    "https://media.steampowered.com/steamcommunity/public/images/apps/{}/{hash}.jpg",
                    game.appid
                )
            }),
            game.img_icon_url.as_deref().map(|hash| {
                format!(
                    "https://media.steampowered.com/steamcommunity/public/images/apps/{}/{hash}.jpg",
                    game.appid
                )
            }),
            &app_details.screenshots,
        ),
    };
    if details.name.is_none() {
        details.name = Some(game.name.clone());
    }
    Ok(details)
}

fn image_assets_for(
    app_id: i64,
    _header_url: Option<&str>,
    _background_url: Option<&str>,
    logo_url: Option<String>,
    icon_url: Option<String>,
    screenshots: &[StoreScreenshot],
) -> Vec<SteamImageAsset> {
    let mut assets = Vec::new();
    assets.push(SteamImageAsset {
        asset_type: "horizontal_cover".to_string(),
        external_id: app_id.to_string(),
        source_url: format!(
            "https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/library_header_2x.jpg"
        ),
    });
    assets.push(SteamImageAsset {
        asset_type: "vertical_cover".to_string(),
        external_id: app_id.to_string(),
        source_url: format!(
            "https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/library_600x900_2x.jpg"
        ),
    });
    assets.push(SteamImageAsset {
        asset_type: "logo".to_string(),
        external_id: app_id.to_string(),
        source_url: logo_url
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| {
                format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/logo.png")
            }),
    });
    assets.push(SteamImageAsset {
        asset_type: "icon".to_string(),
        external_id: app_id.to_string(),
        source_url: icon_url
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| {
                format!("https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/icon.jpg")
            }),
    });
    assets.push(SteamImageAsset {
        asset_type: "hero".to_string(),
        external_id: app_id.to_string(),
        source_url: format!(
            "https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/library_hero_2x.jpg"
        ),
    });
    assets.extend(
        screenshots
            .iter()
            .take(MAX_SCREENSHOTS_PER_GAME)
            .filter_map(|screenshot| {
                screenshot.path_full.as_ref().map(|url| SteamImageAsset {
                    asset_type: "screenshot".to_string(),
                    external_id: screenshot.id.to_string(),
                    source_url: url.clone(),
                })
            }),
    );
    assets
}

fn fallback_image_assets(app_id: i64) -> Vec<SteamImageAsset> {
    image_assets_for(app_id, None, None, None, None, &[])
}

pub async fn download_image_assets(
    sources: Vec<SteamImageSource>,
    root: PathBuf,
    cancel_requested: Arc<AtomicBool>,
    progress_completed: Arc<AtomicUsize>,
) -> Result<SteamImageDownloadBatch, SteamError> {
    ensure_not_cancelled(cancel_requested.as_ref())?;
    let client = build_client()?;
    fs::create_dir_all(&root).map_err(|_| SteamError::RequestSetup)?;
    progress_completed.store(0, Ordering::SeqCst);
    let mut batch = SteamImageDownloadBatch::default();
    batch.screenshot_source_count = sources
        .iter()
        .filter(|source| source.asset_type == "screenshot")
        .count();
    for chunk in sources.chunks(8) {
        ensure_not_cancelled(cancel_requested.as_ref())?;
        let mut handles = Vec::with_capacity(chunk.len());
        for source in chunk {
            let client = client.clone();
            let root = root.clone();
            let cancel_requested = Arc::clone(&cancel_requested);
            let progress_completed = Arc::clone(&progress_completed);
            let source = source.clone();
            handles.push(tauri::async_runtime::spawn(async move {
                let result =
                    download_one_image(&client, &root, &source, cancel_requested.as_ref()).await;
                progress_completed.fetch_add(1, Ordering::SeqCst);
                result
            }));
        }
        for handle in handles {
            let attempt = handle.await.map_err(|_| SteamError::InvalidResponse)??;
            if let Some(failure) = attempt.failure {
                batch.failures.push(failure);
            }
            match attempt.record {
                Some(record) => {
                    if record.asset_type == "screenshot" {
                        batch.screenshot_downloaded_count += 1;
                    }
                    batch.records.push(record);
                }
                None => {
                    batch.skipped_count += 1;
                }
            }
        }
    }
    batch.screenshot_skipped_count = batch
        .screenshot_source_count
        .saturating_sub(batch.screenshot_downloaded_count);
    Ok(batch)
}

pub async fn refresh_steam_image_sources(
    sources: Vec<SteamImageSource>,
    cancel_requested: Arc<AtomicBool>,
) -> Result<SteamImageSourceRefreshResult, SteamError> {
    refresh_steam_image_sources_internal(sources, cancel_requested, false).await
}

pub async fn refresh_steam_image_sources_for_game(
    sources: Vec<SteamImageSource>,
    cancel_requested: Arc<AtomicBool>,
) -> Result<SteamImageSourceRefreshResult, SteamError> {
    refresh_steam_image_sources_internal(sources, cancel_requested, true).await
}

async fn refresh_steam_image_sources_internal(
    sources: Vec<SteamImageSource>,
    cancel_requested: Arc<AtomicBool>,
    force_refresh: bool,
) -> Result<SteamImageSourceRefreshResult, SteamError> {
    let mut by_app: HashMap<i64, Vec<SteamImageSource>> = HashMap::new();
    for source in sources {
        by_app.entry(source.app_id).or_default().push(source);
    }

    let app_ids = by_app
        .iter()
        .filter_map(|(app_id, sources)| {
            (force_refresh || needs_image_source_refresh(sources)).then_some(*app_id)
        })
        .collect::<Vec<_>>();
    if app_ids.is_empty() {
        return Ok(SteamImageSourceRefreshResult {
            sources: by_app.into_values().flatten().collect(),
            ..SteamImageSourceRefreshResult::default()
        });
    }

    let client = build_client()?;
    let mut failed_app_ids = Vec::new();
    let mut failed_errors = Vec::new();
    let mut returned_app_count = 0;
    let mut screenshot_source_count = 0;
    let mut apps_with_screenshots = 0;
    for chunk in app_ids.chunks(STEAM_STORE_REFRESH_CONCURRENCY) {
        ensure_not_cancelled(cancel_requested.as_ref())?;
        let mut handles = Vec::with_capacity(chunk.len());
        for app_id in chunk {
            let app_id = *app_id;
            let client = client.clone();
            let cancel_requested = Arc::clone(&cancel_requested);
            let existing_logo_url = by_app.get(&app_id).and_then(|sources| {
                sources
                    .iter()
                    .find(|source| source.asset_type == "logo")
                    .map(|source| source.source_url.clone())
            });
            let existing_icon_url = by_app.get(&app_id).and_then(|sources| {
                sources
                    .iter()
                    .find(|source| source.asset_type == "icon")
                    .map(|source| source.source_url.clone())
            });
            handles.push(tauri::async_runtime::spawn(async move {
                let assets = fetch_store_image_assets(
                    &client,
                    app_id,
                    existing_logo_url.as_deref(),
                    existing_icon_url.as_deref(),
                    cancel_requested.as_ref(),
                )
                .await;
                (app_id, assets)
            }));
        }
        for handle in handles {
            let (app_id, assets) = handle.await.map_err(|_| SteamError::InvalidResponse)?;
            let Some(game_sources) = by_app.get_mut(&app_id) else {
                continue;
            };
            let assets = match assets {
                Ok(assets) => {
                    returned_app_count += 1;
                    screenshot_source_count += assets
                        .iter()
                        .filter(|asset| asset.asset_type == "screenshot")
                        .count();
                    assets
                }
                Err(error) => {
                    failed_app_ids.push(app_id);
                    failed_errors.push(format!("app_id={app_id}:{error}"));
                    fallback_image_assets(app_id)
                }
            };
            let game_id = game_sources
                .first()
                .map(|source| source.game_id.clone())
                .unwrap_or_default();
            let mut existing = game_sources
                .drain(..)
                .map(|source| {
                    (
                        (source.asset_type.clone(), source.external_id.clone()),
                        source,
                    )
                })
                .collect::<HashMap<_, _>>();
            if assets.iter().any(|asset| asset.asset_type == "screenshot") {
                apps_with_screenshots += 1;
            }
            for asset in assets {
                let key = (asset.asset_type.clone(), asset.external_id.clone());
                if let Some(source) = existing.get_mut(&key) {
                    if source.source_url != asset.source_url {
                        source.local_path = None;
                        source.mime_type = None;
                        source.width = None;
                        source.height = None;
                        source.byte_size = None;
                        source.downloaded_at = None;
                    }
                    source.source_url = asset.source_url;
                } else {
                    existing.insert(
                        key,
                        SteamImageSource {
                            game_id: game_id.clone(),
                            app_id,
                            asset_type: asset.asset_type,
                            external_id: asset.external_id,
                            source_url: asset.source_url,
                            local_path: None,
                            mime_type: None,
                            width: None,
                            height: None,
                            byte_size: None,
                            downloaded_at: None,
                        },
                    );
                }
            }
            *game_sources = existing.into_values().collect();
        }
    }

    Ok(SteamImageSourceRefreshResult {
        sources: by_app.into_values().flatten().collect(),
        failed_app_ids,
        failed_errors,
        requested_app_count: app_ids.len(),
        returned_app_count,
        screenshot_source_count,
        apps_with_screenshots,
    })
}

fn needs_image_source_refresh(sources: &[SteamImageSource]) -> bool {
    ["horizontal_cover", "vertical_cover", "logo", "hero"]
        .iter()
        .any(|asset_type| {
            !sources
                .iter()
                .any(|source| source.asset_type == *asset_type)
        })
        || !sources
            .iter()
            .any(|source| source.asset_type == "screenshot")
}

fn source_has_usable_local_file(source: &SteamImageSource) -> bool {
    let Some(local_path) = source.local_path.as_deref() else {
        return false;
    };
    if !Path::new(local_path).is_file() {
        return false;
    }
    asset_dimensions_meet_target(source.asset_type.as_str(), source.width, source.height)
}

fn asset_dimensions_meet_target(asset_type: &str, width: Option<i64>, height: Option<i64>) -> bool {
    let Some((width, height)) = width.zip(height) else {
        return !matches!(asset_type, "horizontal_cover" | "vertical_cover" | "hero");
    };
    match asset_type {
        "horizontal_cover" => {
            width >= HORIZONTAL_COVER_MIN_WIDTH && height >= HORIZONTAL_COVER_MIN_HEIGHT
        }
        "vertical_cover" => {
            width >= VERTICAL_COVER_MIN_WIDTH && height >= VERTICAL_COVER_MIN_HEIGHT
        }
        "hero" => width >= HERO_MIN_WIDTH && height >= HERO_MIN_HEIGHT,
        _ => true,
    }
}

async fn fetch_store_image_assets(
    client: &reqwest::Client,
    app_id: i64,
    existing_logo_url: Option<&str>,
    existing_icon_url: Option<&str>,
    cancel_requested: &AtomicBool,
) -> Result<Vec<SteamImageAsset>, SteamError> {
    ensure_not_cancelled(cancel_requested)?;
    let app_id_query = app_id.to_string();
    let mut responses = request_json::<HashMap<String, StoreImageDetailsEnvelope>>(
        client,
        &format!("{STEAM_STORE_BASE_URL}/api/appdetails/"),
        &[
            ("appids", app_id_query.as_str()),
            ("l", "english"),
            ("cc", "us"),
        ],
    )
    .await?;
    let response = responses
        .remove(&app_id.to_string())
        .ok_or(SteamError::InvalidResponse)?;
    let data = response
        .data
        .filter(|_| response.success)
        .ok_or(SteamError::InvalidResponse)?;
    ensure_not_cancelled(cancel_requested)?;
    Ok(image_assets_for(
        app_id,
        data.header_image.as_deref(),
        data.background_raw
            .as_deref()
            .or(data.background.as_deref()),
        existing_logo_url.map(str::to_string),
        existing_icon_url.map(str::to_string),
        &data.screenshots,
    ))
}

async fn download_one_image(
    client: &reqwest::Client,
    root: &Path,
    source: &SteamImageSource,
    cancel_requested: &AtomicBool,
) -> Result<SteamImageDownloadAttempt, SteamError> {
    ensure_not_cancelled(cancel_requested)?;
    if let Some(local_path) = source.local_path.as_deref() {
        if source_has_usable_local_file(source) {
            return Ok(SteamImageDownloadAttempt {
                record: Some(existing_image_record(source, local_path)),
                failure: None,
            });
        }
    }
    let mut fallback: Option<(String, image::DynamicImage)> = None;
    let mut last_failure = "no_candidate_succeeded".to_string();
    for candidate_url in image_candidate_urls(source) {
        ensure_not_cancelled(cancel_requested)?;
        let response = match client.get(&candidate_url).send().await {
            Ok(response) => response,
            Err(error) => {
                last_failure = format!("request={error}");
                continue;
            }
        };
        if !response.status().is_success() {
            last_failure = format!("http_status={}", response.status().as_u16());
            continue;
        }
        ensure_not_cancelled(cancel_requested)?;
        let bytes = match response.bytes().await {
            Ok(bytes) => bytes,
            Err(error) => {
                last_failure = format!("read_body={error}");
                continue;
            }
        };
        let decoded = match image::load_from_memory(&bytes) {
            Ok(image) => image,
            Err(error) => {
                last_failure = format!("decode={error}");
                continue;
            }
        };
        let width = i64::from(decoded.width());
        let height = i64::from(decoded.height());
        if !asset_dimensions_meet_target(source.asset_type.as_str(), Some(width), Some(height)) {
            fallback.get_or_insert((candidate_url, decoded));
            last_failure = format!("resolution={width}x{height}");
            continue;
        }
        return persist_downloaded_image(root, source, &candidate_url, decoded);
    }
    if let Some((candidate_url, decoded)) = fallback {
        return persist_downloaded_image(root, source, &candidate_url, decoded);
    }
    Ok(failed_image_download(source, last_failure))
}

fn image_candidate_urls(source: &SteamImageSource) -> Vec<String> {
    let app_id = source.app_id;
    let mut candidates = Vec::new();
    let mut add = |url: String| {
        if !candidates.iter().any(|candidate| candidate == &url) {
            candidates.push(url);
        }
    };
    for url in steam_library_cache_candidates(app_id, source.asset_type.as_str()) {
        add(url);
    }
    match source.asset_type.as_str() {
        "horizontal_cover" => {
            add(format!(
                "https://cdn.akamai.steamstatic.com/steam/apps/{app_id}/library_header_2x.jpg"
            ));
            add(format!(
                "https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/library_header_2x.jpg"
            ));
            add(format!(
                "https://cdn.akamai.steamstatic.com/steam/apps/{app_id}/library_header.jpg"
            ));
            add(format!(
                "https://cdn.akamai.steamstatic.com/steam/apps/{app_id}/header_2x.jpg"
            ));
            add(source.source_url.clone());
            add(format!(
                "https://cdn.akamai.steamstatic.com/steam/apps/{app_id}/header.jpg"
            ));
        }
        "vertical_cover" => {
            add(format!(
                "https://cdn.akamai.steamstatic.com/steam/apps/{app_id}/library_600x900_2x.jpg"
            ));
            add(format!(
                "https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/library_600x900_2x.jpg"
            ));
            add(source.source_url.clone());
            add(format!(
                "https://cdn.akamai.steamstatic.com/steam/apps/{app_id}/library_600x900.jpg"
            ));
        }
        "hero" => {
            add(format!(
                "https://cdn.akamai.steamstatic.com/steam/apps/{app_id}/library_hero_2x.jpg"
            ));
            add(format!(
                "https://cdn.cloudflare.steamstatic.com/steam/apps/{app_id}/library_hero_2x.jpg"
            ));
            add(source.source_url.clone());
            add(format!(
                "https://cdn.akamai.steamstatic.com/steam/apps/{app_id}/library_hero.jpg"
            ));
            add(format!(
                "https://store.akamai.steamstatic.com/images/storepagebackground/app/{app_id}"
            ));
        }
        _ => add(source.source_url.clone()),
    }
    candidates
}

fn steam_library_cache_candidates(app_id: i64, asset_type: &str) -> Vec<String> {
    let cache_names: &[&str] = match asset_type {
        "horizontal_cover" => &["library_header.jpg", "library_header_2x.jpg"],
        "vertical_cover" => &[
            "library_600x900.jpg",
            "library_600x900_2x.jpg",
            "library_capsule.jpg",
            "library_capsule_2x.jpg",
        ],
        "hero" => &["library_hero.jpg", "library_hero_2x.jpg"],
        "logo" => &["logo.png", "logo_2x.png"],
        _ => &[],
    };
    if cache_names.is_empty() {
        return Vec::new();
    }
    let mut urls = Vec::new();
    for root in cached_steam_install_roots() {
        let cache_root = root
            .join("appcache")
            .join("librarycache")
            .join(app_id.to_string());
        let Ok(entries) = fs::read_dir(cache_root) else {
            continue;
        };
        for entry in entries.flatten().filter(|entry| entry.path().is_dir()) {
            let hash = entry.file_name().to_string_lossy().into_owned();
            let files = entry
                .path()
                .read_dir()
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(|file| file.file_name().to_str().map(str::to_string))
                .collect::<HashSet<_>>();
            if !cache_names.iter().any(|name| files.contains(*name)) {
                continue;
            }
            for name in cache_names.iter().rev() {
                let url = format!(
                    "https://shared.fastly.steamstatic.com/store_item_assets/steam/apps/{app_id}/{hash}/{name}"
                );
                if !urls.contains(&url) {
                    urls.push(url);
                }
            }
        }
    }
    urls
}

fn persist_downloaded_image(
    root: &Path,
    source: &SteamImageSource,
    resolved_source_url: &str,
    decoded: image::DynamicImage,
) -> Result<SteamImageDownloadAttempt, SteamError> {
    let width = i64::from(decoded.width());
    let height = i64::from(decoded.height());
    let mut encoded = Cursor::new(Vec::new());
    if decoded
        .write_to(&mut encoded, image::ImageFormat::WebP)
        .is_err()
    {
        return Ok(failed_image_download(source, "encode_webp".to_string()));
    }
    let file_name = format!(
        "{}-{}-{}.webp",
        source.app_id,
        sanitize_asset_part(&source.asset_type),
        sanitize_asset_part(&source.external_id)
    );
    let target = root.join(file_name);
    let temporary = target.with_extension("webp.tmp");
    if let Err(error) = fs::write(&temporary, encoded.into_inner()) {
        return Ok(failed_image_download(source, format!("write_temp={error}")));
    }
    if target.exists() {
        if let Err(error) = fs::remove_file(&target) {
            let _ = fs::remove_file(&temporary);
            return Ok(failed_image_download(
                source,
                format!("replace_existing={error}"),
            ));
        }
    }
    if let Err(error) = fs::rename(&temporary, &target) {
        let _ = fs::remove_file(&temporary);
        return Ok(failed_image_download(source, format!("rename={error}")));
    }
    let downloaded_at = unix_timestamp_string();
    Ok(SteamImageDownloadAttempt {
        record: Some(SteamImageRecord {
            game_id: source.game_id.clone(),
            asset_type: source.asset_type.clone(),
            external_id: source.external_id.clone(),
            source_url: resolved_source_url.to_string(),
            local_path: target.to_string_lossy().into_owned(),
            mime_type: "image/webp".to_string(),
            width,
            height,
            byte_size: fs::metadata(&target)
                .map(|metadata| metadata.len() as i64)
                .unwrap_or_default(),
            downloaded_at,
        }),
        failure: None,
    })
}

fn failed_image_download(source: &SteamImageSource, reason: String) -> SteamImageDownloadAttempt {
    SteamImageDownloadAttempt {
        record: None,
        failure: Some(SteamImageDownloadFailure {
            app_id: source.app_id,
            asset_type: source.asset_type.clone(),
            external_id: source.external_id.clone(),
            source_url: source.source_url.clone(),
            reason,
        }),
    }
}

fn existing_image_record(source: &SteamImageSource, local_path: &str) -> SteamImageRecord {
    SteamImageRecord {
        game_id: source.game_id.clone(),
        asset_type: source.asset_type.clone(),
        external_id: source.external_id.clone(),
        source_url: source.source_url.clone(),
        local_path: local_path.to_string(),
        mime_type: source
            .mime_type
            .clone()
            .unwrap_or_else(|| "image/webp".to_string()),
        width: source.width.unwrap_or_default(),
        height: source.height.unwrap_or_default(),
        byte_size: source.byte_size.unwrap_or_default(),
        downloaded_at: source
            .downloaded_at
            .clone()
            .unwrap_or_else(unix_timestamp_string),
    }
}

fn sanitize_asset_part(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn unix_timestamp_string() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn build_client() -> Result<reqwest::Client, SteamError> {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(20))
        .user_agent("LumaDeck/SteamLibraryV1")
        .build()
        .map_err(|_| SteamError::RequestSetup)
}

async fn request_json<T: DeserializeOwned>(
    client: &reqwest::Client,
    url: &str,
    query: &[(&str, &str)],
) -> Result<T, SteamError> {
    let response = client
        .get(url)
        .query(query)
        .send()
        .await
        .map_err(|_| SteamError::Offline)?;
    if !response.status().is_success() {
        return Err(SteamError::Api(response.status().as_u16()));
    }
    response
        .json::<T>()
        .await
        .map_err(|_| SteamError::InvalidResponse)
}

fn ensure_not_cancelled(cancel_requested: &AtomicBool) -> Result<(), SteamError> {
    if cancel_requested.load(Ordering::SeqCst) {
        Err(SteamError::Cancelled)
    } else {
        Ok(())
    }
}

fn should_refresh(game: &OwnedGame, cached_games: &HashMap<i64, (i64, i64)>) -> bool {
    let Some((cached_playtime, updated_at)) = cached_games.get(&game.appid) else {
        return true;
    };
    if *cached_playtime != game.playtime_forever {
        return true;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or_default();
    now.saturating_sub(*updated_at) >= 86_400
}

fn parse_languages(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or_default()
        .replace("<strong>*</strong>", "")
        .split(',')
        .map(|part| part.replace("<br>", "").replace("*", "").trim().to_string())
        .filter(|part| !part.is_empty())
        .collect()
}

fn parse_platforms(value: Option<&PlatformFlags>) -> Vec<String> {
    let Some(platforms) = value else {
        return Vec::new();
    };
    [
        (platforms.windows, "Windows"),
        (platforms.mac, "macOS"),
        (platforms.linux, "Linux"),
    ]
    .into_iter()
    .filter_map(|(enabled, name)| enabled.then_some(name.to_string()))
    .collect()
}

fn parse_tags(value: Option<&Value>) -> Vec<String> {
    let Some(Value::Object(object)) = value else {
        return Vec::new();
    };
    object
        .values()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

fn requirements_value(value: Option<&Value>, key: &str) -> Option<Value> {
    value.and_then(|value| value.get(key)).cloned()
}

fn value_as_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_str().and_then(|text| text.parse::<i64>().ok()))
}

fn movie_to_media(movie: &StoreMovie) -> SteamMedia {
    let mut metadata = Map::new();
    if let Some(webm) = &movie.webm {
        metadata.insert(
            "webm".to_string(),
            serde_json::to_value(webm).unwrap_or(Value::Null),
        );
    }
    if let Some(mp4) = &movie.mp4 {
        metadata.insert(
            "mp4".to_string(),
            serde_json::to_value(mp4).unwrap_or(Value::Null),
        );
    }
    if let Some(hls_h264) = &movie.hls_h264 {
        metadata.insert("hls_h264".to_string(), Value::String(hls_h264.clone()));
    }
    metadata.insert(
        "highlight".to_string(),
        Value::Bool(movie.highlight.unwrap_or(false)),
    );
    SteamMedia {
        media_type: "movie",
        external_id: movie.id.to_string(),
        name: movie.name.clone(),
        thumbnail_url: movie.thumbnail.clone(),
        full_url: movie
            .hls_h264
            .clone()
            .or_else(|| {
                movie
                    .mp4
                    .as_ref()
                    .and_then(|values| values.get("max"))
                    .cloned()
            })
            .or_else(|| {
                movie
                    .webm
                    .as_ref()
                    .and_then(|values| values.get("max"))
                    .cloned()
            }),
        metadata: Value::Object(metadata),
    }
}

#[cfg(test)]
fn parse_player_summary(body: &str, steam_id64: &str) -> Result<PlayerSummary, SteamError> {
    let response: PlayerSummaryResponse =
        serde_json::from_str(body).map_err(|_| SteamError::InvalidResponse)?;
    response
        .response
        .players
        .into_iter()
        .find(|player| player.steamid == steam_id64)
        .filter(|player| !player.personaname.is_empty())
        .ok_or(SteamError::InvalidResponse)
}

#[cfg(test)]
fn parse_owned_game_count(body: &str) -> Result<u32, SteamError> {
    let response: OwnedGamesResponse =
        serde_json::from_str(body).map_err(|_| SteamError::InvalidResponse)?;
    Ok(response.response.game_count)
}

#[cfg(test)]
mod tests {
    use super::{
        asset_dimensions_meet_target, image_assets_for, image_candidate_urls,
        needs_image_source_refresh, parse_languages, parse_owned_game_count, parse_player_summary,
        AppDetailsEnvelope, GlobalAchievementPercentagesResponse, SchemaResponse, SteamError,
        SteamImageSource,
    };

    const STEAM_ID: &str = "76561198012345678";

    #[test]
    fn parses_player_summary_and_owned_games() {
        let player = parse_player_summary(r#"{"response":{"players":[{"steamid":"76561198012345678","personaname":"Luma Player","avatarfull":"https://avatars.steamstatic.com/avatar.jpg","loccountrycode":"SV"}]}}"#, STEAM_ID).expect("player summary");
        assert_eq!(player.personaname, "Luma Player");
        assert_eq!(
            parse_owned_game_count(r#"{"response":{"game_count":42}}"#).expect("owned games"),
            42
        );
    }

    #[test]
    fn parses_steam_achievement_schema_fields() {
        let response: SchemaResponse = serde_json::from_str(
            r#"{"game":{"availableGameStats":{"achievements":[{"apiname":"ACH_WIN","displayName":"Winner","description":"Win once"}]}}}"#,
        )
        .expect("achievement schema");
        let achievement = &response
            .game
            .and_then(|game| game.available_game_stats)
            .expect("available stats")
            .achievements[0];
        assert_eq!(achievement.apiname, "ACH_WIN");
        assert_eq!(achievement.displayname.as_deref(), Some("Winner"));
    }

    #[test]
    fn parses_global_achievement_percentages() {
        let response: GlobalAchievementPercentagesResponse = serde_json::from_str(
            r#"{"achievementpercentages":{"achievements":[{"name":"ACH_WIN","percent":12.5}]}}"#,
        )
        .expect("global achievement percentages");
        let achievement = &response
            .achievement_percentages
            .expect("achievement percentages")
            .achievements[0];
        assert_eq!(achievement.name, "ACH_WIN");
        assert_eq!(achievement.percent, Some(12.5));
    }

    #[test]
    fn parses_store_genres_for_home_tags() {
        let response: std::collections::HashMap<String, AppDetailsEnvelope> =
            serde_json::from_str(
                r#"{"3035570":{"success":true,"data":{"genres":[{"id":"1","description":"Action"},{"id":"2","description":"Adventure"}]}}}"#,
            )
            .expect("store details");
        let genres = response
            .get("3035570")
            .and_then(|value| value.data.as_ref())
            .map(|value| {
                value
                    .genres
                    .iter()
                    .map(|genre| genre.name.as_str())
                    .collect::<Vec<_>>()
            })
            .expect("store genres");
        assert_eq!(genres, vec!["Action", "Adventure"]);
    }

    #[test]
    fn parses_store_metadata_used_by_game_details() {
        let response: std::collections::HashMap<String, AppDetailsEnvelope> = serde_json::from_str(
            r#"{
                    "3768760": {
                        "success": true,
                        "data": {
                            "steam_appid": 3768760,
                            "controller_support": "full",
                            "detailed_description": "<p>Full description</p>",
                            "short_description": "Short description",
                            "release_date": {"coming_soon": false, "date": "May 26, 2026"},
                            "pc_requirements": {"minimum": "min", "recommended": "recommended"},
                            "price_overview": {"currency": "USD", "final": 6999},
                            "required_age": 0,
                            "categories": [
                                {"id": 2, "description": "Single-player"},
                                {"id": 28, "description": "Full controller support"},
                                {"id": 23, "description": "Steam Cloud"}
                            ],
                            "genres": [{"id": "1", "description": "Action"}],
                            "movies": [{
                                "id": 870337973,
                                "thumbnail": "https://example.test/trailer.jpg",
                                "hls_h264": "https://example.test/trailer/master.m3u8",
                                "highlight": true
                            }]
                        }
                    }
                }"#,
        )
        .expect("store metadata");
        let details = response
            .get("3768760")
            .and_then(|value| value.data.as_ref())
            .expect("store metadata payload");

        assert_eq!(details.controller_support.as_deref(), Some("full"));
        assert_eq!(
            details
                .release_date
                .as_ref()
                .and_then(|value| value.date.as_deref()),
            Some("May 26, 2026")
        );
        assert_eq!(
            details.detailed_description.as_deref(),
            Some("<p>Full description</p>")
        );
        assert_eq!(
            details.short_description.as_deref(),
            Some("Short description")
        );
        assert!(details.pc_requirements.is_some());
        assert!(details.price_overview.is_some());
        assert_eq!(
            details.required_age.as_ref().and_then(super::value_as_i64),
            Some(0)
        );
        assert_eq!(details.categories.len(), 3);
        assert_eq!(details.genres[0].name, "Action");
        assert_eq!(
            details.movies[0].hls_h264.as_deref(),
            Some("https://example.test/trailer/master.m3u8")
        );
        assert_eq!(
            super::movie_to_media(&details.movies[0])
                .full_url
                .as_deref(),
            Some("https://example.test/trailer/master.m3u8")
        );
    }

    #[test]
    fn parses_store_languages_without_html_markers() {
        assert_eq!(
            parse_languages(Some("English<strong>*</strong>, Spanish<br>")),
            vec!["English", "Spanish"]
        );
    }

    #[test]
    fn exposes_cancellation_error() {
        assert!(matches!(SteamError::Cancelled, SteamError::Cancelled));
    }

    #[test]
    fn creates_all_required_image_sources_with_cdn_fallbacks() {
        let assets = image_assets_for(
            123,
            None,
            None,
            None,
            None,
            &[super::StoreScreenshot {
                id: 7,
                path_thumbnail: None,
                path_full: Some("https://example.test/123/screenshot.png".to_string()),
            }],
        );
        let types = assets
            .iter()
            .map(|asset| asset.asset_type.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            types,
            vec![
                "horizontal_cover",
                "vertical_cover",
                "logo",
                "icon",
                "hero",
                "screenshot"
            ]
        );
        assert!(assets.iter().all(|asset| asset.source_url.contains("123")));
    }

    #[test]
    fn limits_screenshots_to_eight_per_game() {
        let screenshots = (1..=12)
            .map(|id| super::StoreScreenshot {
                id,
                path_thumbnail: None,
                path_full: Some(format!("https://example.test/123/screenshot-{id}.png")),
            })
            .collect::<Vec<_>>();
        let assets = image_assets_for(123, None, None, None, None, &screenshots);
        assert_eq!(
            assets
                .iter()
                .filter(|asset| asset.asset_type == "screenshot")
                .count(),
            8
        );
    }

    #[test]
    fn prefers_high_resolution_library_asset_urls_over_store_previews() {
        let assets = image_assets_for(
            3768760,
            Some("https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/3768760/header_alt_assets_2.jpg"),
            Some("https://store.akamai.steamstatic.com/images/storepagebackground/app/3768760"),
            None,
            None,
            &[],
        );
        assert_eq!(
            assets
                .iter()
                .find(|asset| asset.asset_type == "horizontal_cover")
                .expect("horizontal cover")
                .source_url,
            "https://cdn.cloudflare.steamstatic.com/steam/apps/3768760/library_header_2x.jpg"
        );
        assert_eq!(
            assets
                .iter()
                .find(|asset| asset.asset_type == "hero")
                .expect("hero")
                .source_url,
            "https://cdn.cloudflare.steamstatic.com/steam/apps/3768760/library_hero_2x.jpg"
        );
    }

    #[test]
    fn rejects_low_resolution_required_library_assets() {
        assert!(!asset_dimensions_meet_target(
            "horizontal_cover",
            Some(460),
            Some(215)
        ));
        assert!(asset_dimensions_meet_target(
            "horizontal_cover",
            Some(920),
            Some(430)
        ));
        assert!(!asset_dimensions_meet_target("hero", Some(1438), Some(808)));
        assert!(asset_dimensions_meet_target("hero", Some(3840), Some(1240)));
        assert!(asset_dimensions_meet_target(
            "vertical_cover",
            Some(600),
            Some(900)
        ));
    }

    #[test]
    fn orders_high_resolution_candidates_before_fallbacks() {
        let source = SteamImageSource {
            game_id: "steam-123".to_string(),
            app_id: 123,
            asset_type: "hero".to_string(),
            external_id: "123".to_string(),
            source_url: "https://store.example/low-background".to_string(),
            local_path: None,
            mime_type: None,
            width: None,
            height: None,
            byte_size: None,
            downloaded_at: None,
        };
        let candidates = image_candidate_urls(&source);
        assert!(candidates[0].contains("library_hero_2x.jpg"));
        assert!(
            candidates
                .iter()
                .position(|url| url == &source.source_url)
                .expect("source URL")
                < candidates
                    .iter()
                    .position(|url| url.ends_with("library_hero.jpg"))
                    .expect("one-times hero fallback")
        );
    }

    #[test]
    fn refreshes_image_sources_when_screenshots_are_missing() {
        let assets = image_assets_for(123, None, None, None, None, &[])
            .into_iter()
            .map(|asset| SteamImageSource {
                game_id: "steam-123".to_string(),
                app_id: 123,
                asset_type: asset.asset_type,
                external_id: asset.external_id,
                source_url: asset.source_url,
                local_path: None,
                mime_type: None,
                width: None,
                height: None,
                byte_size: None,
                downloaded_at: None,
            })
            .collect::<Vec<_>>();
        assert!(needs_image_source_refresh(&assets));
    }

    #[test]
    fn does_not_refresh_sources_when_only_local_files_are_missing() {
        let mut assets = image_assets_for(3768760, None, None, None, None, &[])
            .into_iter()
            .map(|asset| SteamImageSource {
                game_id: "steam-3768760".to_string(),
                app_id: 3768760,
                asset_type: asset.asset_type,
                external_id: asset.external_id,
                source_url: asset.source_url,
                local_path: None,
                mime_type: None,
                width: None,
                height: None,
                byte_size: None,
                downloaded_at: None,
            })
            .collect::<Vec<_>>();
        assets.push(SteamImageSource {
            game_id: "steam-3768760".to_string(),
            app_id: 3768760,
            asset_type: "screenshot".to_string(),
            external_id: "screenshot-1".to_string(),
            source_url: "https://example.test/screenshot.jpg".to_string(),
            local_path: None,
            mime_type: None,
            width: None,
            height: None,
            byte_size: None,
            downloaded_at: None,
        });
        assert!(!needs_image_source_refresh(&assets));
    }
}
