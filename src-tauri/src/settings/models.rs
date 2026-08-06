use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug)]
pub struct SteamCredentials {
    pub steam_id64: String,
    pub api_key: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamLibrarySyncSettings {
    pub scope: String,
}

impl Default for SteamLibrarySyncSettings {
    fn default() -> Self {
        Self {
            scope: "all".to_string(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamConfigurationStatus {
    pub provider_id: String,
    pub account_id: Option<String>,
    pub steam_id64_masked: Option<String>,
    pub api_key_configured: bool,
    pub api_key_masked: Option<String>,
    pub status: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamGridDbConfigurationStatus {
    pub provider_id: String,
    pub api_key_configured: bool,
    pub api_key_masked: Option<String>,
    pub credential_available: bool,
    pub status: String,
    pub enabled: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RapidApiReviewsConfigurationStatus {
    pub provider_id: String,
    pub api_key_configured: bool,
    pub api_key_masked: Option<String>,
    pub credential_available: bool,
    pub status: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AIConfiguration {
    pub provider_id: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AIConnectionStatus {
    pub state: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AIConfigurationStatus {
    pub configuration: AIConfiguration,
    pub api_key_configured: bool,
    pub api_key_masked: Option<String>,
    pub credential_available: bool,
    pub connection: AIConnectionStatus,
}

#[derive(Debug, Clone)]
pub struct ReviewsCache {
    pub steam_app_id: i64,
    pub metacritic_json: Option<String>,
    pub opencritic_json: Option<String>,
    pub steam_json: Option<String>,
    pub steam_updated_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationConfigurationStatus {
    pub provider_id: String,
    pub api_key_configured: bool,
    pub api_key_masked: Option<String>,
    pub credential_available: bool,
    pub status: String,
    pub enabled: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseStatus {
    pub path: String,
    pub schema_version: i64,
    pub provider_count: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageMigrationStatus {
    pub status: String,
    pub current_mode: String,
    pub current_path: String,
    pub target_mode: Option<String>,
    pub target_path: Option<String>,
    pub files_copied: u64,
    pub total_files: u64,
    pub bytes_copied: u64,
    pub total_bytes: u64,
    pub error_message: Option<String>,
    pub needs_restart: bool,
    pub delete_source: bool,
}

impl StorageMigrationStatus {
    pub fn idle(current_mode: String, current_path: String) -> Self {
        Self {
            status: "idle".to_string(),
            current_mode,
            current_path,
            target_mode: None,
            target_path: None,
            files_copied: 0,
            total_files: 0,
            bytes_copied: 0,
            total_bytes: 0,
            error_message: None,
            needs_restart: false,
            delete_source: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageStatus {
    pub mode: String,
    pub current_path: String,
    pub normal_path: String,
    pub portable_path: String,
    pub used_bytes: u64,
    pub migration: StorageMigrationStatus,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageMigrationResult {
    pub status: String,
    pub source_mode: String,
    pub target_mode: String,
    pub source_path: String,
    pub target_path: String,
    pub files_copied: u64,
    pub bytes_copied: u64,
    pub needs_restart: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamSyncStatus {
    pub status: String,
    pub found_count: i64,
    pub created_count: i64,
    pub updated_count: i64,
    pub progress_completed: i64,
    pub progress_total: i64,
    pub duration_ms: Option<i64>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub current_app_id: Option<i64>,
    pub error_message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamSyncResult {
    pub status: String,
    pub found_count: i64,
    pub created_count: i64,
    pub updated_count: i64,
    pub duration_ms: i64,
    pub completed_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamImageSyncStatus {
    pub status: String,
    pub found_count: i64,
    pub downloaded_count: i64,
    pub skipped_count: i64,
    pub progress_completed: i64,
    pub progress_total: i64,
    pub duration_ms: Option<i64>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub current_app_id: Option<i64>,
    pub error_message: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamImageSyncResult {
    pub status: String,
    pub found_count: i64,
    pub downloaded_count: i64,
    pub skipped_count: i64,
    pub duration_ms: i64,
    pub completed_at: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamAchievementSyncResult {
    pub status: String,
    pub found_count: i64,
    pub updated_count: i64,
    pub skipped_count: i64,
    pub duration_ms: i64,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HltbSettings {
    pub enabled: bool,
    pub sync_with_steam: bool,
    pub show_main_story: bool,
    pub show_main_extra: bool,
    pub show_completionist: bool,
}

impl Default for HltbSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            sync_with_steam: true,
            show_main_story: true,
            show_main_extra: true,
            show_completionist: true,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HltbSyncStatus {
    pub status: String,
    pub processed_count: i64,
    pub total_count: i64,
    pub found_count: i64,
    pub unmatched_count: i64,
    pub exact_match_count: i64,
    pub approximate_match_count: i64,
    pub error_count: i64,
    pub duration_ms: Option<i64>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HltbGameData {
    pub game_id: String,
    pub hltb_id: Option<String>,
    pub matched_title: Option<String>,
    pub main_story_minutes: Option<i64>,
    pub main_extra_minutes: Option<i64>,
    pub completionist_minutes: Option<i64>,
    pub match_confidence: Option<f64>,
    pub match_type: Option<String>,
    pub last_synced_at: Option<String>,
    pub source: String,
    pub status: String,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct HltbLocalGame {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HltbPendingMatch {
    pub game_id: String,
    pub title: String,
    pub alias_query: Option<String>,
    pub resolution_status: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalGameAchievements {
    pub total: Option<i64>,
    pub unlocked: Option<i64>,
    pub progress: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamGameMetrics {
    pub total_playtime_minutes: i64,
    pub last_played_at: Option<String>,
    pub progress: f64,
    pub achievement_total: Option<i64>,
    pub achievement_unlocked: Option<i64>,
    pub active_players: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivitySession {
    pub id: i64,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub duration_seconds: Option<i64>,
    pub status: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityEvent {
    pub id: String,
    pub event_type: String,
    pub occurred_at: String,
    pub title: String,
    pub description: Option<String>,
    pub value: Option<Value>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityStat {
    pub key: String,
    pub label: String,
    pub value: Value,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityStreak {
    pub current_days: i64,
    pub best_days: i64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivityFriend {
    pub steam_id64: String,
    pub persona_name: String,
    pub avatar_url: String,
    pub persona_state: String,
    pub game_name: Option<String>,
    pub game_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivitySourceStatus {
    pub source: String,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivitySnapshot {
    pub status: String,
    pub metrics: Option<SteamGameMetrics>,
    pub last_session: Option<ActivitySession>,
    pub sessions: Vec<ActivitySession>,
    pub events: Vec<ActivityEvent>,
    pub stats: Vec<ActivityStat>,
    pub streak: ActivityStreak,
    pub friends: Vec<ActivityFriend>,
    pub friends_status: String,
    pub sources: Vec<ActivitySourceStatus>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalSteamDetails {
    pub app_id: i64,
    pub description: Option<String>,
    pub short_description: Option<String>,
    pub tags: Vec<String>,
    pub genres: Vec<String>,
    pub categories: Vec<String>,
    pub screenshots: Vec<String>,
    pub movies: Vec<String>,
    pub multiplayer: Option<bool>,
    pub single_player: Option<bool>,
    pub cloud: Option<bool>,
    pub trading_cards: Option<bool>,
    pub workshop: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalGameDetails {
    pub steam: Option<LocalSteamDetails>,
    pub hltb: Option<HltbGameData>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalGame {
    pub id: String,
    pub title: String,
    pub sort_title: String,
    pub platform: String,
    pub provider: String,
    pub cover_url: String,
    pub vertical_cover_url: String,
    pub logo_url: String,
    pub background_url: String,
    pub icon_url: String,
    pub screenshots: Vec<String>,
    pub description: String,
    pub genres: Vec<String>,
    pub release_year: i64,
    pub playtime_minutes: i64,
    pub last_played_at: Option<String>,
    pub favorite: bool,
    pub installed: bool,
    pub progress: f64,
    pub status: String,
    pub achievements: Option<LocalGameAchievements>,
    pub details: Option<LocalGameDetails>,
}

#[derive(Debug, Clone)]
pub struct SteamLaunchGame {
    pub provider: String,
    pub platform: String,
    pub installed: bool,
    pub steam_app_id: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameGenerationProfile {
    pub game_id: String,
    pub provider: String,
    pub enabled: bool,
    pub mode: String,
    pub multiplier: i64,
    pub auto_scale: bool,
    pub auto_scale_delay: i64,
    pub target_executable: Option<String>,
    pub updated_at: Option<String>,
    #[serde(default)]
    pub restart_required: bool,
}

impl FrameGenerationProfile {
    pub fn off(game_id: &str) -> Self {
        Self {
            game_id: game_id.to_string(),
            provider: "lossless-scaling".to_string(),
            enabled: false,
            mode: "FIXED".to_string(),
            multiplier: 2,
            auto_scale: true,
            auto_scale_delay: 0,
            target_executable: None,
            updated_at: None,
            restart_required: false,
        }
    }
}
