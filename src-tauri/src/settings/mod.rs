mod artwork_repository;
mod crypto;
mod database;
mod models;
mod repositories;

pub use crate::display::{DisplayProfile, PendingDisplayProfileRestore, PendingDisplayRestore};
pub use artwork_repository::ArtworkApplyResult;
pub use database::{DatabaseError, DatabaseState, LaunchBoxCatalogPhase, LaunchBoxCatalogProgress};
pub use models::{
    AIConfigurationStatus, ActivityFriend, ActivitySnapshot, DatabaseStatus,
    FrameGenerationProfile, HltbPendingMatch, HltbSettings, HltbSyncStatus, LaunchGame, LocalGame,
    LocalLaunchBoxDetails, RapidApiReviewsConfigurationStatus, ReviewsCache,
    SteamAchievementSyncResult, SteamConfigurationStatus, SteamCredentials, SteamGameMetrics,
    SteamGridDbConfigurationStatus, SteamImageSyncResult, SteamImageSyncStatus, SteamLaunchGame,
    SteamLibrarySyncSettings, SteamSyncResult, SteamSyncStatus, StorageMigrationResult,
    StorageMigrationStatus, StorageStatus, TranslationConfigurationStatus,
};

use crate::data_directory::DataDirectoryResolver;
use repositories::SettingsRepository;

pub fn initialize(data_directory: DataDirectoryResolver) -> Result<DatabaseState, DatabaseError> {
    let state = DatabaseState::open(data_directory)?;
    match crate::eden::reconcile_existing_identities(&state) {
        Ok(attempted) if attempted > 0 => state.log(
            "eden-identity",
            "EDEN_IDENTITY_STARTUP_CLEANUP_COMPLETED",
            "reconciliation=existing-title-id-records",
        ),
        Err(error) => state.log(
            "eden-identity",
            "EDEN_IDENTITY_STARTUP_CLEANUP_FAILED",
            &format!("error={error}"),
        ),
        _ => {}
    }
    SettingsRepository::new(&state).recover_interrupted_syncs()?;
    let recovered_sessions = recover_stale_game_sessions(&state)?;
    if recovered_sessions > 0 {
        state.log(
            "game-session-recovery",
            "STALE_RUNNING_STATE_RECOVERED",
            &format!("sessions={recovered_sessions} duration_seconds=0"),
        );
    }
    Ok(state)
}

pub fn get_provider_configuration(
    state: &DatabaseState,
    provider_id: &str,
) -> Result<SteamConfigurationStatus, DatabaseError> {
    SettingsRepository::new(state).get_provider_configuration(provider_id)
}

pub fn get_steam_credentials(state: &DatabaseState) -> Result<SteamCredentials, DatabaseError> {
    SettingsRepository::new(state).get_steam_credentials()
}

pub fn get_ai_configuration(state: &DatabaseState) -> Result<AIConfigurationStatus, DatabaseError> {
    SettingsRepository::new(state).get_ai_configuration()
}

pub fn save_ai_configuration(
    state: &DatabaseState,
    provider_id: &str,
    model: &str,
    api_key: &str,
) -> Result<AIConfigurationStatus, DatabaseError> {
    SettingsRepository::new(state).save_ai_configuration(provider_id, model, api_key)
}

pub(crate) fn get_ai_api_key(state: &DatabaseState) -> Result<String, DatabaseError> {
    SettingsRepository::new(state).get_ai_api_key()
}

pub fn get_steamgriddb_configuration(
    state: &DatabaseState,
) -> Result<SteamGridDbConfigurationStatus, DatabaseError> {
    SettingsRepository::new(state).get_steamgriddb_configuration()
}

pub fn save_steamgriddb_api_key(
    state: &DatabaseState,
    api_key: &str,
) -> Result<SteamGridDbConfigurationStatus, DatabaseError> {
    SettingsRepository::new(state).save_steamgriddb_api_key(api_key)
}

pub fn delete_steamgriddb_api_key(
    state: &DatabaseState,
) -> Result<SteamGridDbConfigurationStatus, DatabaseError> {
    SettingsRepository::new(state).delete_steamgriddb_api_key()
}

pub(crate) fn get_steamgriddb_api_key(state: &DatabaseState) -> Result<String, DatabaseError> {
    SettingsRepository::new(state).get_steamgriddb_api_key()
}

pub(crate) fn get_steamgriddb_game_identity(
    state: &DatabaseState,
    game_id: &str,
) -> Result<crate::steamgriddb::LocalGameIdentity, DatabaseError> {
    SettingsRepository::new(state).get_steamgriddb_game_identity(game_id)
}

pub(crate) fn persist_artwork_selection(
    state: &DatabaseState,
    artwork: &crate::artwork::PreparedArtwork,
) -> Result<ArtworkApplyResult, DatabaseError> {
    artwork_repository::ArtworkRepository::new(state).persist_selection(artwork)
}

pub(crate) fn clear_artwork_selection(
    state: &DatabaseState,
    game_id: &str,
    slot: crate::steamgriddb::ArtworkSlot,
) -> Result<(), DatabaseError> {
    artwork_repository::ArtworkRepository::new(state).clear_selection(game_id, slot)
}

pub(crate) fn get_current_artwork(
    state: &DatabaseState,
    game_id: &str,
    slot: crate::steamgriddb::ArtworkSlot,
) -> Result<Option<String>, DatabaseError> {
    artwork_repository::ArtworkRepository::new(state).get_current_asset(game_id, slot)
}

pub fn get_steam_cache(
    state: &DatabaseState,
) -> Result<std::collections::HashMap<i64, (i64, i64)>, DatabaseError> {
    SettingsRepository::new(state).get_steam_cache()
}

pub fn get_steam_game_for_metadata(
    state: &DatabaseState,
    game_id: &str,
) -> Result<Option<crate::steam::SteamLibraryGame>, DatabaseError> {
    SettingsRepository::new(state).get_steam_game_for_metadata(game_id)
}

pub fn sync_steam_game_metadata(
    state: &DatabaseState,
    game: &crate::steam::SteamLibraryGame,
) -> Result<(), DatabaseError> {
    SettingsRepository::new(state).sync_steam_game_metadata(game)
}

pub fn update_steam_active_players(
    state: &DatabaseState,
    game_id: &str,
    active_players: i64,
) -> Result<(), DatabaseError> {
    SettingsRepository::new(state).update_steam_active_players(game_id, active_players)
}

pub fn get_steam_game_metrics(
    state: &DatabaseState,
    game_id: &str,
) -> Result<SteamGameMetrics, DatabaseError> {
    SettingsRepository::new(state).get_steam_game_metrics(game_id)
}

pub fn get_game_activity(
    state: &DatabaseState,
    game_id: &str,
) -> Result<ActivitySnapshot, DatabaseError> {
    SettingsRepository::new(state).get_game_activity(game_id)
}

pub fn get_steam_app_id(
    state: &DatabaseState,
    game_id: &str,
) -> Result<Option<i64>, DatabaseError> {
    SettingsRepository::new(state).get_steam_app_id(game_id)
}

pub fn get_steam_launch_game(
    state: &DatabaseState,
    game_id: &str,
) -> Result<Option<SteamLaunchGame>, DatabaseError> {
    SettingsRepository::new(state).get_steam_launch_game(game_id)
}

pub fn get_launch_game(
    state: &DatabaseState,
    game_id: &str,
) -> Result<Option<LaunchGame>, DatabaseError> {
    SettingsRepository::new(state).get_launch_game(game_id)
}

pub fn get_display_profile(
    state: &DatabaseState,
    game_id: &str,
) -> Result<DisplayProfile, DatabaseError> {
    SettingsRepository::new(state).get_display_profile(game_id)
}

pub fn save_display_profile(
    state: &DatabaseState,
    profile: &DisplayProfile,
) -> Result<DisplayProfile, DatabaseError> {
    SettingsRepository::new(state).save_display_profile(profile)
}

pub fn reset_display_profile(state: &DatabaseState, game_id: &str) -> Result<(), DatabaseError> {
    SettingsRepository::new(state).reset_display_profile(game_id)
}

pub fn get_frame_generation_profile(
    state: &DatabaseState,
    game_id: &str,
) -> Result<FrameGenerationProfile, DatabaseError> {
    SettingsRepository::new(state).get_frame_generation_profile(game_id)
}

pub fn save_frame_generation_profile(
    state: &DatabaseState,
    profile: &FrameGenerationProfile,
) -> Result<FrameGenerationProfile, DatabaseError> {
    SettingsRepository::new(state).save_frame_generation_profile(profile)
}

pub fn set_frame_generation_target(
    state: &DatabaseState,
    game_id: &str,
    target_executable: &str,
) -> Result<FrameGenerationProfile, DatabaseError> {
    SettingsRepository::new(state).set_frame_generation_target(game_id, target_executable)
}

pub fn get_pending_display_restore(
    state: &DatabaseState,
) -> Result<Option<PendingDisplayRestore>, DatabaseError> {
    SettingsRepository::new(state).get_pending_display_restore()
}

pub fn save_pending_display_restore(
    state: &DatabaseState,
    pending: &PendingDisplayRestore,
) -> Result<(), DatabaseError> {
    SettingsRepository::new(state).save_pending_display_restore(pending)
}

pub fn clear_pending_display_restore(state: &DatabaseState) -> Result<(), DatabaseError> {
    SettingsRepository::new(state).clear_pending_display_restore()
}

pub fn get_pending_display_profile_restore(
    state: &DatabaseState,
) -> Result<Option<PendingDisplayProfileRestore>, DatabaseError> {
    SettingsRepository::new(state).get_pending_display_profile_restore()
}

pub fn save_pending_display_profile_restore(
    state: &DatabaseState,
    pending: &PendingDisplayProfileRestore,
) -> Result<(), DatabaseError> {
    SettingsRepository::new(state).save_pending_display_profile_restore(pending)
}

pub fn clear_pending_display_profile_restore(state: &DatabaseState) -> Result<(), DatabaseError> {
    SettingsRepository::new(state).clear_pending_display_profile_restore()
}

pub fn start_game_session(state: &DatabaseState, game_id: &str) -> Result<i64, DatabaseError> {
    SettingsRepository::new(state).start_game_session(game_id)
}

pub fn end_game_session(
    state: &DatabaseState,
    game_id: &str,
    session_id: i64,
    interrupted: bool,
) -> Result<(), DatabaseError> {
    SettingsRepository::new(state).end_game_session(game_id, session_id, interrupted)
}

pub fn recover_stale_game_sessions(state: &DatabaseState) -> Result<i64, DatabaseError> {
    SettingsRepository::new(state).recover_stale_game_sessions()
}

pub fn get_steam_library_sync_settings(
    state: &DatabaseState,
) -> Result<SteamLibrarySyncSettings, DatabaseError> {
    SettingsRepository::new(state).get_steam_library_sync_settings()
}

pub fn set_steam_library_sync_scope(
    state: &DatabaseState,
    scope: &str,
) -> Result<SteamLibrarySyncSettings, DatabaseError> {
    SettingsRepository::new(state).set_steam_library_sync_scope(scope)
}

pub fn get_local_games(state: &DatabaseState) -> Result<Vec<LocalGame>, DatabaseError> {
    SettingsRepository::new(state).get_local_games()
}

pub fn set_game_favorite(
    state: &DatabaseState,
    game_id: &str,
    favorite: bool,
) -> Result<bool, DatabaseError> {
    SettingsRepository::new(state).set_game_favorite(game_id, favorite)
}

pub fn set_game_hidden(
    state: &DatabaseState,
    game_id: &str,
    hidden: bool,
) -> Result<bool, DatabaseError> {
    SettingsRepository::new(state).set_game_hidden(game_id, hidden)
}

pub fn get_hltb_settings(state: &DatabaseState) -> Result<HltbSettings, DatabaseError> {
    SettingsRepository::new(state).get_hltb_settings()
}

pub fn set_hltb_settings(
    state: &DatabaseState,
    settings: &HltbSettings,
) -> Result<HltbSettings, DatabaseError> {
    SettingsRepository::new(state).set_hltb_settings(settings)
}

pub fn get_hltb_sync_status(state: &DatabaseState) -> Result<HltbSyncStatus, DatabaseError> {
    SettingsRepository::new(state).get_hltb_sync_status()
}

pub fn get_hltb_local_games(
    state: &DatabaseState,
    only_missing: bool,
) -> Result<Vec<models::HltbLocalGame>, DatabaseError> {
    SettingsRepository::new(state).get_hltb_local_games(only_missing)
}

pub fn get_hltb_pending_matches(
    state: &DatabaseState,
) -> Result<Vec<HltbPendingMatch>, DatabaseError> {
    SettingsRepository::new(state).get_hltb_pending_matches()
}

pub fn set_hltb_match_override(
    state: &DatabaseState,
    game_id: &str,
    alias_query: &str,
    candidate: Option<&crate::hltb::HltbCandidate>,
    resolution_status: &str,
) -> Result<(), DatabaseError> {
    SettingsRepository::new(state).set_hltb_match_override(
        game_id,
        alias_query,
        candidate,
        resolution_status,
    )
}

pub fn clear_hltb_match_override(
    state: &DatabaseState,
    game_id: &str,
) -> Result<(), DatabaseError> {
    SettingsRepository::new(state).clear_hltb_match_override(game_id)
}

pub fn begin_hltb_sync(
    state: &DatabaseState,
    total_count: i64,
    started_at: &str,
) -> Result<(), DatabaseError> {
    SettingsRepository::new(state).begin_hltb_sync(total_count, started_at)
}

pub fn update_hltb_sync_progress(
    state: &DatabaseState,
    processed: i64,
) -> Result<(), DatabaseError> {
    SettingsRepository::new(state).update_hltb_sync_progress(processed)
}

pub fn save_hltb_game(
    state: &DatabaseState,
    game_id: &str,
    result: Option<&crate::hltb::HltbResult>,
    status: &str,
    error: Option<&str>,
) -> Result<(), DatabaseError> {
    SettingsRepository::new(state).save_hltb_game(game_id, result, status, error)
}

pub fn finish_hltb_sync(
    state: &DatabaseState,
    found_count: i64,
    unmatched_count: i64,
    exact_match_count: i64,
    approximate_match_count: i64,
    error_count: i64,
    duration_ms: i64,
    completed_at: &str,
) -> Result<(), DatabaseError> {
    SettingsRepository::new(state).finish_hltb_sync(
        found_count,
        unmatched_count,
        exact_match_count,
        approximate_match_count,
        error_count,
        duration_ms,
        completed_at,
    )
}

pub fn fail_hltb_sync(
    state: &DatabaseState,
    duration_ms: i64,
    error: &str,
) -> Result<(), DatabaseError> {
    SettingsRepository::new(state).fail_hltb_sync(duration_ms, error)
}

pub fn cancel_hltb_sync(state: &DatabaseState, duration_ms: i64) -> Result<(), DatabaseError> {
    SettingsRepository::new(state).cancel_hltb_sync(duration_ms)
}

pub fn get_steam_app_ids(state: &DatabaseState) -> Result<Vec<i64>, DatabaseError> {
    SettingsRepository::new(state).get_steam_app_ids()
}

pub fn clear_stale_steam_achievements(state: &DatabaseState) -> Result<i64, DatabaseError> {
    SettingsRepository::new(state).clear_stale_steam_achievements()
}

pub fn save_steam_achievements(
    state: &DatabaseState,
    app_id: i64,
    achievements: &[crate::achievements::Achievement],
    genres: &[String],
    total: i64,
    stats: &[crate::steam::SteamStat],
) -> Result<bool, DatabaseError> {
    SettingsRepository::new(state).save_steam_achievements(
        app_id,
        achievements,
        genres,
        total,
        stats,
    )
}

pub fn get_game_achievements(
    state: &DatabaseState,
    game_id: &str,
) -> Result<crate::achievements::GameAchievements, DatabaseError> {
    SettingsRepository::new(state).get_game_achievements(game_id)
}

pub fn get_achievement_summary(
    state: &DatabaseState,
    game_id: &str,
) -> Result<crate::achievements::AchievementSummary, DatabaseError> {
    SettingsRepository::new(state).get_achievement_summary(game_id)
}

pub fn get_achievement_distribution(
    state: &DatabaseState,
    game_id: &str,
) -> Result<crate::achievements::AchievementDistribution, DatabaseError> {
    SettingsRepository::new(state).get_achievement_distribution(game_id)
}

pub fn get_rapidapi_reviews_configuration(
    state: &DatabaseState,
) -> Result<RapidApiReviewsConfigurationStatus, DatabaseError> {
    SettingsRepository::new(state).get_rapidapi_reviews_configuration()
}

pub fn save_rapidapi_reviews_api_key(
    state: &DatabaseState,
    api_key: &str,
) -> Result<RapidApiReviewsConfigurationStatus, DatabaseError> {
    SettingsRepository::new(state).save_rapidapi_reviews_api_key(api_key)
}

pub fn delete_rapidapi_reviews_api_key(
    state: &DatabaseState,
) -> Result<RapidApiReviewsConfigurationStatus, DatabaseError> {
    SettingsRepository::new(state).delete_rapidapi_reviews_api_key()
}

pub(crate) fn get_rapidapi_reviews_api_key(state: &DatabaseState) -> Result<String, DatabaseError> {
    SettingsRepository::new(state).get_rapidapi_reviews_api_key()
}

pub(crate) fn get_reviews_cache(
    state: &DatabaseState,
    game_id: &str,
) -> Result<Option<ReviewsCache>, DatabaseError> {
    SettingsRepository::new(state).get_reviews_cache(game_id)
}

pub(crate) fn save_reviews_provider_cache(
    state: &DatabaseState,
    game_id: &str,
    steam_app_id: i64,
    provider: &str,
    payload_json: &str,
) -> Result<(), DatabaseError> {
    SettingsRepository::new(state).save_reviews_provider_cache(
        game_id,
        steam_app_id,
        provider,
        payload_json,
    )
}

pub(crate) fn get_game_review_consensus(
    state: &DatabaseState,
    game_id: &str,
) -> Result<Option<crate::consensus::GameReviewConsensus>, DatabaseError> {
    SettingsRepository::new(state).get_game_review_consensus(game_id)
}

pub(crate) fn save_game_review_consensus(
    state: &DatabaseState,
    consensus: &crate::consensus::GameReviewConsensus,
) -> Result<(), DatabaseError> {
    SettingsRepository::new(state).save_game_review_consensus(consensus)
}

pub fn get_translation_configuration(
    state: &DatabaseState,
) -> Result<TranslationConfigurationStatus, DatabaseError> {
    SettingsRepository::new(state).get_translation_configuration()
}

pub fn save_translation_api_key(
    state: &DatabaseState,
    api_key: &str,
) -> Result<TranslationConfigurationStatus, DatabaseError> {
    SettingsRepository::new(state).save_translation_api_key(api_key)
}

pub fn delete_translation_api_key(
    state: &DatabaseState,
) -> Result<TranslationConfigurationStatus, DatabaseError> {
    SettingsRepository::new(state).delete_translation_api_key()
}

pub(crate) fn get_translation_api_key(state: &DatabaseState) -> Result<String, DatabaseError> {
    SettingsRepository::new(state).get_translation_api_key()
}

pub(crate) fn get_translation_provider_selection(
    state: &DatabaseState,
) -> Result<Option<String>, DatabaseError> {
    SettingsRepository::new(state).get_translation_provider_selection()
}

pub(crate) fn set_translation_provider_selection(
    state: &DatabaseState,
    provider_id: &str,
) -> Result<String, DatabaseError> {
    SettingsRepository::new(state).set_translation_provider_selection(provider_id)
}

pub fn get_achievement_distributions(
    state: &DatabaseState,
    game_id: &str,
) -> Result<crate::achievements::AchievementDistributions, DatabaseError> {
    SettingsRepository::new(state).get_achievement_distributions(game_id)
}

pub fn begin_steam_sync(
    state: &DatabaseState,
    found_count: i64,
    started_at: &str,
) -> Result<(), DatabaseError> {
    SettingsRepository::new(state).begin_steam_sync(found_count, started_at)
}

pub fn update_steam_sync_progress(
    state: &DatabaseState,
    completed: i64,
    total: i64,
    app_id: Option<i64>,
) -> Result<(), DatabaseError> {
    SettingsRepository::new(state).update_steam_sync_progress(completed, total, app_id)
}

pub fn get_steam_sync_status(state: &DatabaseState) -> Result<SteamSyncStatus, DatabaseError> {
    SettingsRepository::new(state).get_steam_sync_status()
}

pub fn get_steam_image_sources(
    state: &DatabaseState,
) -> Result<Vec<crate::steam::SteamImageSource>, DatabaseError> {
    SettingsRepository::new(state).get_steam_image_sources()
}

pub fn upsert_steam_image_sources(
    state: &DatabaseState,
    sources: &[crate::steam::SteamImageSource],
) -> Result<(), DatabaseError> {
    SettingsRepository::new(state).upsert_steam_image_sources(sources)
}

pub fn persist_steam_image_records(
    state: &DatabaseState,
    records: &[crate::steam::SteamImageRecord],
) -> Result<(), DatabaseError> {
    SettingsRepository::new(state).persist_steam_image_records(records)
}

pub fn begin_steam_image_sync(
    state: &DatabaseState,
    found_count: i64,
    started_at: &str,
) -> Result<(), DatabaseError> {
    SettingsRepository::new(state).begin_steam_image_sync(found_count, started_at)
}

pub fn update_steam_image_sync_progress(
    state: &DatabaseState,
    completed: i64,
    total: i64,
    app_id: Option<i64>,
) -> Result<(), DatabaseError> {
    SettingsRepository::new(state).update_steam_image_sync_progress(completed, total, app_id)
}

pub fn get_steam_image_sync_status(
    state: &DatabaseState,
) -> Result<SteamImageSyncStatus, DatabaseError> {
    SettingsRepository::new(state).get_steam_image_sync_status()
}

pub fn sync_steam_image_records(
    state: &DatabaseState,
    records: &[crate::steam::SteamImageRecord],
    found_count: i64,
    skipped_count: i64,
    duration_ms: i64,
    completed_at: &str,
) -> Result<SteamImageSyncResult, DatabaseError> {
    SettingsRepository::new(state).sync_steam_image_records(
        records,
        found_count,
        skipped_count,
        duration_ms,
        completed_at,
    )
}

pub fn fail_steam_image_sync(
    state: &DatabaseState,
    duration_ms: i64,
    error_message: &str,
) -> Result<(), DatabaseError> {
    SettingsRepository::new(state).fail_steam_image_sync(duration_ms, error_message)
}

pub fn cancel_steam_image_sync(
    state: &DatabaseState,
    duration_ms: i64,
) -> Result<(), DatabaseError> {
    SettingsRepository::new(state).cancel_steam_image_sync(duration_ms)
}

pub fn sync_steam_games(
    state: &DatabaseState,
    games: &[crate::steam::SteamLibraryGame],
    installed_scope: bool,
    duration_ms: i64,
    completed_at: &str,
) -> Result<SteamSyncResult, DatabaseError> {
    SettingsRepository::new(state).sync_steam_games(
        games,
        installed_scope,
        duration_ms,
        completed_at,
    )
}

pub fn fail_steam_sync(
    state: &DatabaseState,
    duration_ms: i64,
    error_message: &str,
) -> Result<(), DatabaseError> {
    SettingsRepository::new(state).fail_steam_sync(duration_ms, error_message)
}

pub fn cancel_steam_sync(state: &DatabaseState, duration_ms: i64) -> Result<(), DatabaseError> {
    SettingsRepository::new(state).cancel_steam_sync(duration_ms)
}

pub fn save_steam_account_configuration(
    state: &DatabaseState,
    steam_id64: &str,
    api_key: &str,
    correlation_id: &str,
) -> Result<SteamConfigurationStatus, DatabaseError> {
    SettingsRepository::new(state).save_steam_account_configuration(
        steam_id64,
        api_key,
        correlation_id,
    )
}

pub fn update_steam_id(
    state: &DatabaseState,
    steam_id64: &str,
    correlation_id: &str,
) -> Result<SteamConfigurationStatus, DatabaseError> {
    SettingsRepository::new(state).update_steam_id(steam_id64, correlation_id)
}

pub fn replace_steam_api_key(
    state: &DatabaseState,
    api_key: &str,
    correlation_id: &str,
) -> Result<SteamConfigurationStatus, DatabaseError> {
    SettingsRepository::new(state).replace_steam_api_key(api_key, correlation_id)
}

pub fn disconnect_provider_account(
    state: &DatabaseState,
    account_id: &str,
) -> Result<SteamConfigurationStatus, DatabaseError> {
    SettingsRepository::new(state).disconnect_provider_account(account_id)
}

pub fn get_database_status(state: &DatabaseState) -> Result<DatabaseStatus, DatabaseError> {
    SettingsRepository::new(state).get_database_status()
}
