mod artwork;
mod data_directory;
mod display;
mod frame_generation;
mod game_session;
mod hltb;
mod lossless_scaling;
mod settings;
mod steam;
pub mod steamgriddb;
mod storage_migration;

use artwork::{ApplyArtworkRequest, ArtworkDownloadError};
use data_directory::DataDirectoryMode;
use frame_generation::FrameGenerationProvider;
use game_session::{GameSessionStatus, SessionCommandError, SteamGameSessionService};
use settings::{
    DatabaseState, DatabaseStatus, SteamConfigurationStatus, StorageMigrationResult, StorageStatus,
};
use std::{
    sync::atomic::Ordering,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use steam::{SteamError, SteamProfile};
use steamgriddb::{ArtworkSearchRequest, ArtworkSearchResult, SteamGridDbClient, SteamGridDbError};
use tauri::{AppHandle, Manager, State};

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn get_provider_configuration(
    state: State<'_, DatabaseState>,
    provider_id: String,
) -> Result<SteamConfigurationStatus, String> {
    let correlation_id = "settings-read-no-correlation";
    state.log(
        correlation_id,
        "COMMAND_ENTER",
        "command=get_provider_configuration",
    );
    state.log_runtime_context(correlation_id);
    map_command_result(
        &state,
        correlation_id,
        "get_provider_configuration",
        settings::get_provider_configuration(&state, &provider_id),
    )
}

#[tauri::command]
async fn search_steamgriddb_artwork(
    state: State<'_, DatabaseState>,
    request: ArtworkSearchRequest,
) -> Result<ArtworkSearchResult, String> {
    let search_generation = state
        .steamgriddb_search_generation
        .fetch_add(1, Ordering::SeqCst)
        .saturating_add(1);
    let correlation_id = "steamgriddb-artwork-search";
    state.log(
        correlation_id,
        "COMMAND_ENTER",
        "command=search_steamgriddb_artwork",
    );
    let identity = settings::get_steamgriddb_game_identity(&state, &request.game_id)
        .map_err(|error| command_error(&state, correlation_id, "steamgriddb_identity", error))?;
    state.log(
        correlation_id,
        "ARTWORK_IDENTITY_RESOLVED",
        &format!(
            "game_id={} steam_app_id={:?} slot={:?} style_filter={:?}",
            identity.local_game_id, identity.steam_app_id, request.slot, request.style_filter
        ),
    );
    let api_key = settings::get_steamgriddb_api_key(&state)
        .map_err(|error| command_error(&state, correlation_id, "steamgriddb_credential", error))?;
    let client = SteamGridDbClient::new(api_key).map_err(|error| {
        steamgriddb_command_error(&state, correlation_id, "client_create", error)
    })?;
    let (steamgriddb_game_id, identity_source) = if let Some(steam_app_id) = identity.steam_app_id {
        (
            client
                .resolve_steam_game(steam_app_id)
                .await
                .map_err(|error| {
                    steamgriddb_command_error(&state, correlation_id, "identity_resolve", error)
                })?,
            "steam_app_id",
        )
    } else {
        (
            client
                .resolve_title(&identity.title)
                .await
                .map_err(|error| {
                    steamgriddb_command_error(
                        &state,
                        correlation_id,
                        "identity_title_search",
                        error,
                    )
                })?,
            "title_exact",
        )
    };
    if state.steamgriddb_search_generation.load(Ordering::SeqCst) != search_generation {
        return Err("ARTWORK_SEARCH_CANCELLED".to_string());
    }
    let assets = client
        .get_assets(steamgriddb_game_id, request.slot, request.style_filter)
        .await
        .map_err(|error| {
            steamgriddb_command_error(&state, correlation_id, "asset_search", error)
        })?;
    if state.steamgriddb_search_generation.load(Ordering::SeqCst) != search_generation {
        return Err("ARTWORK_SEARCH_CANCELLED".to_string());
    }
    let (query_id, candidates) = state
        .steamgriddb_query_cache
        .lock()
        .map_err(|_| "ARTWORK_TEMPORARY_CACHE_UNAVAILABLE".to_string())?
        .insert(
            request.game_id.clone(),
            request.slot,
            request.style_filter,
            assets,
        );
    Ok(ArtworkSearchResult {
        query_id,
        game_id: request.game_id,
        slot: request.slot,
        style_filter: request.style_filter,
        identity: steamgriddb::SteamGridDbGameIdentity {
            local_game_id: identity.local_game_id,
            title: identity.title,
            steam_app_id: identity.steam_app_id,
            steamgriddb_game_id: Some(steamgriddb_game_id),
            source: identity_source.to_string(),
            status: "resolved".to_string(),
        },
        candidates,
    })
}

#[tauri::command]
fn cancel_steamgriddb_artwork_search(state: State<'_, DatabaseState>) {
    state
        .steamgriddb_search_generation
        .fetch_add(1, Ordering::SeqCst);
}

#[tauri::command]
async fn apply_steamgriddb_artwork(
    state: State<'_, DatabaseState>,
    request: ApplyArtworkRequest,
) -> Result<settings::ArtworkApplyResult, String> {
    let prepared = artwork::prepare_selected_artwork(&state, request)
        .await
        .map_err(|error| artwork_command_error(&state, "prepare_artwork", error))?;
    match settings::persist_artwork_selection(&state, &prepared) {
        Ok(result) => Ok(result),
        Err(error) => {
            let _ = artwork::remove_uncommitted_file(&state, &prepared);
            Err(command_error(
                &state,
                "steamgriddb-artwork-apply",
                "persist_artwork",
                error,
            ))
        }
    }
}

#[tauri::command]
fn restore_steamgriddb_artwork(
    state: State<'_, DatabaseState>,
    game_id: String,
    slot: steamgriddb::ArtworkSlot,
) -> Result<(), String> {
    map_command_result(
        &state,
        "steamgriddb-artwork-restore",
        "restore_steamgriddb_artwork",
        settings::clear_artwork_selection(&state, &game_id, slot),
    )
}

#[tauri::command]
fn get_current_steamgriddb_artwork(
    state: State<'_, DatabaseState>,
    game_id: String,
    slot: steamgriddb::ArtworkSlot,
) -> Result<Option<String>, String> {
    map_command_result(
        &state,
        "steamgriddb-artwork-current",
        "get_current_steamgriddb_artwork",
        settings::get_current_artwork(&state, &game_id, slot),
    )
}

#[tauri::command]
fn save_steam_account_configuration(
    state: State<'_, DatabaseState>,
    steam_id64: String,
    api_key: String,
    correlation_id: Option<String>,
) -> Result<SteamConfigurationStatus, String> {
    let correlation_id =
        correlation_id.unwrap_or_else(|| "settings-save-missing-correlation".to_string());
    state.log(
        &correlation_id,
        "COMMAND_ENTER",
        &format!(
            "command=save_steam_account_configuration pid={} build={}",
            std::process::id(),
            option_env!("GIT_COMMIT").unwrap_or("dev-unavailable")
        ),
    );
    state.log(
        &correlation_id,
        "DTO_RECEIVED",
        &format!(
            "steam_id64_length={} api_key_present={} dto={{steam_id64:String, api_key:String, correlation_id:String}}",
            steam_id64.trim().len(),
            !api_key.trim().is_empty()
        ),
    );
    state.log(
        &correlation_id,
        "DATABASE_PATH_RESOLVED",
        &format!("path={}", state.path.display()),
    );
    state.log(
        &correlation_id,
        "DATABASE_OPEN_SUCCESS",
        "database_state=managed_connection",
    );
    state.log_runtime_context(&correlation_id);
    map_command_result(
        &state,
        &correlation_id,
        "save_steam_account_configuration",
        settings::save_steam_account_configuration(&state, &steam_id64, &api_key, &correlation_id),
    )
}

#[tauri::command]
fn update_steam_id(
    state: State<'_, DatabaseState>,
    steam_id64: String,
    correlation_id: Option<String>,
) -> Result<SteamConfigurationStatus, String> {
    let correlation_id =
        correlation_id.unwrap_or_else(|| "settings-save-missing-correlation".to_string());
    state.log(&correlation_id, "COMMAND_ENTER", "command=update_steam_id");
    state.log(
        &correlation_id,
        "DTO_RECEIVED",
        &format!(
            "steam_id64_length={} dto={{steam_id64:String, correlation_id:String}}",
            steam_id64.trim().len()
        ),
    );
    state.log(
        &correlation_id,
        "DATABASE_PATH_RESOLVED",
        &format!("path={}", state.path.display()),
    );
    state.log(
        &correlation_id,
        "DATABASE_OPEN_SUCCESS",
        "database_state=managed_connection",
    );
    state.log_runtime_context(&correlation_id);
    map_command_result(
        &state,
        &correlation_id,
        "update_steam_id",
        settings::update_steam_id(&state, &steam_id64, &correlation_id),
    )
}

#[tauri::command]
fn replace_steam_api_key(
    state: State<'_, DatabaseState>,
    api_key: String,
    correlation_id: Option<String>,
) -> Result<SteamConfigurationStatus, String> {
    let correlation_id =
        correlation_id.unwrap_or_else(|| "settings-save-missing-correlation".to_string());
    state.log(
        &correlation_id,
        "COMMAND_ENTER",
        "command=replace_steam_api_key",
    );
    state.log(
        &correlation_id,
        "DTO_RECEIVED",
        &format!(
            "api_key_present={} dto={{api_key:String, correlation_id:String}}",
            !api_key.trim().is_empty()
        ),
    );
    state.log(
        &correlation_id,
        "DATABASE_PATH_RESOLVED",
        &format!("path={}", state.path.display()),
    );
    state.log(
        &correlation_id,
        "DATABASE_OPEN_SUCCESS",
        "database_state=managed_connection",
    );
    state.log_runtime_context(&correlation_id);
    map_command_result(
        &state,
        &correlation_id,
        "replace_steam_api_key",
        settings::replace_steam_api_key(&state, &api_key, &correlation_id),
    )
}

#[tauri::command]
fn disconnect_provider_account(
    state: State<'_, DatabaseState>,
    account_id: String,
) -> Result<SteamConfigurationStatus, String> {
    let correlation_id = "settings-command-no-correlation";
    state.log(
        correlation_id,
        "COMMAND_ENTER",
        "command=disconnect_provider_account",
    );
    map_command_result(
        &state,
        correlation_id,
        "disconnect_provider_account",
        settings::disconnect_provider_account(&state, &account_id),
    )
}

#[tauri::command]
fn get_database_status(state: State<'_, DatabaseState>) -> Result<DatabaseStatus, String> {
    let correlation_id = "settings-command-no-correlation";
    state.log(
        correlation_id,
        "COMMAND_ENTER",
        "command=get_database_status",
    );
    map_command_result(
        &state,
        correlation_id,
        "get_database_status",
        settings::get_database_status(&state),
    )
}

#[tauri::command]
fn get_storage_status(state: State<'_, DatabaseState>) -> Result<StorageStatus, String> {
    storage_migration::get_storage_status(&state).map_err(storage_command_error)
}

#[tauri::command]
fn migrate_storage(
    state: State<'_, DatabaseState>,
    target_mode: String,
    delete_source: bool,
) -> Result<StorageMigrationResult, String> {
    let target_mode = DataDirectoryMode::from_str(&target_mode)
        .ok_or_else(|| "STORAGE_MIGRATION_INVALID_MODE".to_string())?;
    storage_migration::migrate_storage(&state, target_mode, delete_source)
        .map_err(storage_command_error)
}

#[tauri::command]
async fn get_steam_profile(state: State<'_, DatabaseState>) -> Result<SteamProfile, String> {
    let correlation_id = "steam-profile-read";
    state.log(correlation_id, "COMMAND_ENTER", "command=get_steam_profile");
    let credentials = match settings::get_steam_credentials(&state) {
        Ok(credentials) => credentials,
        Err(error) => {
            return Err(command_error(
                &state,
                correlation_id,
                "get_steam_profile",
                error,
            ))
        }
    };
    match steam::fetch_profile(&credentials.steam_id64, &credentials.api_key).await {
        Ok(profile) => {
            state.log(
                correlation_id,
                "COMMAND_RETURN_SUCCESS",
                "command=get_steam_profile",
            );
            Ok(profile)
        }
        Err(error) => {
            state.log(
                correlation_id,
                "STEAM_API_ERROR",
                &format!("error_variant={error:?}"),
            );
            Err(match error {
                SteamError::Offline => "STEAM_OFFLINE".to_string(),
                SteamError::Api(_) => "STEAM_API_ERROR".to_string(),
                SteamError::InvalidResponse => "STEAM_INVALID_RESPONSE".to_string(),
                SteamError::RequestSetup => "STEAM_API_ERROR".to_string(),
                SteamError::Cancelled => "STEAM_SYNC_CANCELLED".to_string(),
            })
        }
    }
}

#[tauri::command]
fn get_hltb_settings(state: State<'_, DatabaseState>) -> Result<settings::HltbSettings, String> {
    map_command_result(
        &state,
        "hltb-settings-read",
        "get_hltb_settings",
        settings::get_hltb_settings(&state),
    )
}

#[tauri::command]
fn set_hltb_settings(
    state: State<'_, DatabaseState>,
    settings: settings::HltbSettings,
) -> Result<settings::HltbSettings, String> {
    map_command_result(
        &state,
        "hltb-settings-save",
        "set_hltb_settings",
        settings::set_hltb_settings(&state, &settings),
    )
}

#[tauri::command]
fn get_steamgriddb_configuration(
    state: State<'_, DatabaseState>,
) -> Result<settings::SteamGridDbConfigurationStatus, String> {
    map_command_result(
        &state,
        "steamgriddb-settings-read",
        "get_steamgriddb_configuration",
        settings::get_steamgriddb_configuration(&state),
    )
}

#[tauri::command]
fn save_steamgriddb_api_key(
    state: State<'_, DatabaseState>,
    api_key: String,
) -> Result<settings::SteamGridDbConfigurationStatus, String> {
    state.log(
        "steamgriddb-settings-save",
        "DTO_RECEIVED",
        &format!(
            "api_key_present={} api_key_length={}",
            !api_key.trim().is_empty(),
            api_key.trim().len()
        ),
    );
    map_command_result(
        &state,
        "steamgriddb-settings-save",
        "save_steamgriddb_api_key",
        settings::save_steamgriddb_api_key(&state, &api_key),
    )
}

#[tauri::command]
fn delete_steamgriddb_api_key(
    state: State<'_, DatabaseState>,
) -> Result<settings::SteamGridDbConfigurationStatus, String> {
    map_command_result(
        &state,
        "steamgriddb-settings-delete",
        "delete_steamgriddb_api_key",
        settings::delete_steamgriddb_api_key(&state),
    )
}

#[tauri::command]
fn get_hltb_sync_status(
    state: State<'_, DatabaseState>,
) -> Result<settings::HltbSyncStatus, String> {
    map_command_result(
        &state,
        "hltb-status-read",
        "get_hltb_sync_status",
        settings::get_hltb_sync_status(&state),
    )
}

#[tauri::command]
fn get_hltb_pending_matches(
    state: State<'_, DatabaseState>,
) -> Result<Vec<settings::HltbPendingMatch>, String> {
    map_command_result(
        &state,
        "hltb-pending-read",
        "get_hltb_pending_matches",
        settings::get_hltb_pending_matches(&state),
    )
}

#[tauri::command]
async fn search_hltb_candidates(query: String) -> Result<Vec<hltb::HltbCandidate>, String> {
    hltb::search_game(&query).await.map_err(hltb_command_error)
}

#[tauri::command]
fn set_hltb_match_override(
    state: State<'_, DatabaseState>,
    game_id: String,
    alias_query: String,
    candidate: hltb::HltbCandidate,
) -> Result<(), String> {
    map_command_result(
        &state,
        "hltb-override-save",
        "set_hltb_match_override",
        settings::set_hltb_match_override(
            &state,
            &game_id,
            &alias_query,
            Some(&candidate),
            "manual",
        ),
    )
}

#[tauri::command]
fn ignore_hltb_match(
    state: State<'_, DatabaseState>,
    game_id: String,
    alias_query: String,
) -> Result<(), String> {
    map_command_result(
        &state,
        "hltb-override-ignore",
        "ignore_hltb_match",
        settings::set_hltb_match_override(&state, &game_id, &alias_query, None, "ignored"),
    )
}

#[tauri::command]
fn clear_hltb_match_override(
    state: State<'_, DatabaseState>,
    game_id: String,
) -> Result<(), String> {
    map_command_result(
        &state,
        "hltb-override-clear",
        "clear_hltb_match_override",
        settings::clear_hltb_match_override(&state, &game_id),
    )
}

#[tauri::command]
fn cancel_hltb_sync(state: State<'_, DatabaseState>) -> Result<settings::HltbSyncStatus, String> {
    state
        .hltb_sync_cancel_requested
        .store(true, Ordering::SeqCst);
    settings::cancel_hltb_sync(&state, 0)
        .map_err(|error| command_error(&state, "hltb-cancel", "cancel_hltb_sync", error))?;
    settings::get_hltb_sync_status(&state)
        .map_err(|error| command_error(&state, "hltb-cancel", "cancel_hltb_sync", error))
}

#[tauri::command]
async fn sync_hltb_library(
    state: State<'_, DatabaseState>,
    only_missing: bool,
) -> Result<settings::HltbSyncStatus, String> {
    if state
        .hltb_sync_running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("HLTB_SYNC_ALREADY_RUNNING".to_string());
    }
    state
        .hltb_sync_cancel_requested
        .store(false, Ordering::SeqCst);
    let started = Instant::now();
    let started_at = unix_timestamp();
    let result = async {
        let integration = settings::get_hltb_settings(&state)
            .map_err(|error| command_error(&state, "hltb-sync", "sync_hltb_library", error))?;
        if !integration.enabled {
            return Err("HLTB_DISABLED".to_string());
        }
        let games = settings::get_hltb_local_games(&state, only_missing)
            .map_err(|error| command_error(&state, "hltb-sync", "sync_hltb_library", error))?;
        settings::begin_hltb_sync(&state, games.len() as i64, &started_at)
            .map_err(|error| command_error(&state, "hltb-sync", "sync_hltb_library", error))?;
        state.hltb_sync_progress.store(0, Ordering::SeqCst);
        state.hltb_sync_total.store(games.len(), Ordering::SeqCst);
        let mut found_count = 0_i64;
        let mut unmatched_count = 0_i64;
        let mut exact_match_count = 0_i64;
        let mut approximate_match_count = 0_i64;
        let mut error_count = 0_i64;
        for (index, game) in games.iter().enumerate() {
            if state.hltb_sync_cancel_requested.load(Ordering::SeqCst) {
                settings::cancel_hltb_sync(&state, started.elapsed().as_millis() as i64).map_err(
                    |error| command_error(&state, "hltb-sync", "sync_hltb_library", error),
                )?;
                return Err("HLTB_SYNC_CANCELLED".to_string());
            }
            let lookup = hltb::search_game(&game.title).await;
            match lookup {
                Ok(candidates) => {
                    if let Some(matched) = hltb::choose_match(&game.title, &candidates) {
                        found_count += 1;
                        if matched.match_type == "exact" {
                            exact_match_count += 1;
                        } else {
                            approximate_match_count += 1;
                        }
                        settings::save_hltb_game(&state, &game.id, Some(&matched), "matched", None)
                            .map_err(|error| {
                                command_error(&state, "hltb-sync", "sync_hltb_library", error)
                            })?;
                    } else {
                        unmatched_count += 1;
                        settings::save_hltb_game(&state, &game.id, None, "unmatched", None)
                            .map_err(|error| {
                                command_error(&state, "hltb-sync", "sync_hltb_library", error)
                            })?;
                    }
                }
                Err(error) => {
                    error_count += 1;
                    let message = match error {
                        hltb::HltbError::Offline => "HLTB_OFFLINE",
                        hltb::HltbError::Api(_) => "HLTB_API_ERROR",
                        hltb::HltbError::InvalidResponse => "HLTB_INVALID_RESPONSE",
                        hltb::HltbError::RequestSetup => "HLTB_REQUEST_ERROR",
                    };
                    settings::save_hltb_game(&state, &game.id, None, "error", Some(message))
                        .map_err(|save_error| {
                            command_error(&state, "hltb-sync", "sync_hltb_library", save_error)
                        })?;
                }
            }
            let processed = (index + 1) as i64;
            state.hltb_sync_progress.store(index + 1, Ordering::SeqCst);
            settings::update_hltb_sync_progress(&state, processed)
                .map_err(|error| command_error(&state, "hltb-sync", "sync_hltb_library", error))?;
        }
        settings::finish_hltb_sync(
            &state,
            found_count,
            unmatched_count,
            exact_match_count,
            approximate_match_count,
            error_count,
            started.elapsed().as_millis() as i64,
            &unix_timestamp(),
        )
        .map_err(|error| command_error(&state, "hltb-sync", "sync_hltb_library", error))?;
        settings::get_hltb_sync_status(&state)
            .map_err(|error| command_error(&state, "hltb-sync", "sync_hltb_library", error))
    }
    .await;
    if let Err(error) = &result {
        if error != "HLTB_SYNC_CANCELLED" {
            let _ = settings::fail_hltb_sync(&state, started.elapsed().as_millis() as i64, error);
        }
    }
    state.hltb_sync_running.store(false, Ordering::SeqCst);
    result
}

#[tauri::command]
fn get_steam_sync_status(
    state: State<'_, DatabaseState>,
) -> Result<settings::SteamSyncStatus, String> {
    map_command_result(
        &state,
        "steam-sync-status",
        "get_steam_sync_status",
        settings::get_steam_sync_status(&state),
    )
}

#[tauri::command]
fn get_steam_library_sync_settings(
    state: State<'_, DatabaseState>,
) -> Result<settings::SteamLibrarySyncSettings, String> {
    map_command_result(
        &state,
        "steam-sync-settings-read",
        "get_steam_library_sync_settings",
        settings::get_steam_library_sync_settings(&state),
    )
}

#[tauri::command]
fn set_steam_library_sync_scope(
    state: State<'_, DatabaseState>,
    scope: String,
) -> Result<settings::SteamLibrarySyncSettings, String> {
    map_command_result(
        &state,
        "steam-sync-settings-write",
        "set_steam_library_sync_scope",
        settings::set_steam_library_sync_scope(&state, &scope),
    )
}

#[tauri::command]
fn get_steam_image_sync_status(
    state: State<'_, DatabaseState>,
) -> Result<settings::SteamImageSyncStatus, String> {
    map_command_result(
        &state,
        "steam-image-sync-status",
        "get_steam_image_sync_status",
        settings::get_steam_image_sync_status(&state),
    )
}

#[tauri::command]
fn get_library_games(state: State<'_, DatabaseState>) -> Result<Vec<settings::LocalGame>, String> {
    map_command_result(
        &state,
        "library-read",
        "get_library_games",
        settings::get_local_games(&state),
    )
}

#[tauri::command]
fn set_game_favorite(
    state: State<'_, DatabaseState>,
    game_id: String,
    favorite: bool,
) -> Result<bool, String> {
    map_command_result(
        &state,
        "game-favorite-save",
        "set_game_favorite",
        settings::set_game_favorite(&state, &game_id, favorite),
    )
}

#[tauri::command]
async fn refresh_steam_game_metadata(
    state: State<'_, DatabaseState>,
    game_id: String,
) -> Result<i64, String> {
    let correlation_id = "steam-metadata-refresh";
    if state.steam_sync_running.load(Ordering::SeqCst)
        || state.steam_image_sync_running.load(Ordering::SeqCst)
        || state.steam_achievement_sync_running.load(Ordering::SeqCst)
        || state
            .steam_metadata_sync_running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
    {
        return Err("STEAM_METADATA_SYNC_ALREADY_RUNNING".to_string());
    }
    state
        .steam_sync_cancel_requested
        .store(false, Ordering::SeqCst);

    let result = async {
        let mut game = settings::get_steam_game_for_metadata(&state, &game_id)
            .map_err(|error| {
                command_error(&state, correlation_id, "get_steam_game_metadata", error)
            })?
            .ok_or_else(|| "GAME_NOT_FOUND".to_string())?;
        let credentials = settings::get_steam_credentials(&state).map_err(|error| {
            command_error(&state, correlation_id, "get_steam_credentials", error)
        })?;
        let details = steam::fetch_game_metadata(
            &credentials.steam_id64,
            &credentials.api_key,
            game.app_id,
            &game.name,
            game.logo_url.clone(),
            &state.steam_sync_cancel_requested,
        )
        .await
        .map_err(steam_command_error)?;
        if !details.complete {
            return Err("STEAM_METADATA_NOT_AVAILABLE".to_string());
        }
        let hls_count = details
            .movies
            .iter()
            .filter(|movie| {
                movie
                    .full_url
                    .as_deref()
                    .is_some_and(|url| url.contains(".m3u8"))
            })
            .count();
        state.log(
            correlation_id,
            "STEAM_TRAILER_SOURCES_RECEIVED",
            &format!(
                "app_id={} movie_count={} hls_count={}",
                game.app_id,
                details.movies.len(),
                hls_count
            ),
        );
        game.details = Some(details);
        settings::sync_steam_game_metadata(&state, &game).map_err(|error| {
            command_error(&state, correlation_id, "sync_steam_game_metadata", error)
        })?;
        Ok(game.app_id)
    }
    .await;

    state
        .steam_metadata_sync_running
        .store(false, Ordering::SeqCst);
    if let Ok(app_id) = result {
        state.log(
            correlation_id,
            "COMMAND_RETURN_SUCCESS",
            &format!("command=refresh_steam_game_metadata app_id={app_id}"),
        );
    } else if let Err(error) = &result {
        state.log(
            correlation_id,
            "COMMAND_ERROR",
            &format!("command=refresh_steam_game_metadata error_code={error}"),
        );
    }
    result
}

#[tauri::command]
async fn get_game_activity(
    state: State<'_, DatabaseState>,
    game_id: String,
) -> Result<settings::ActivitySnapshot, String> {
    let correlation_id = "game-activity-read";
    let mut snapshot = map_command_result(
        &state,
        correlation_id,
        "get_game_activity",
        settings::get_game_activity(&state, &game_id),
    )?;
    let credentials = match settings::get_steam_credentials(&state) {
        Ok(credentials) => credentials,
        Err(error) => {
            let code = command_error(&state, correlation_id, "get_steam_credentials", error);
            mark_activity_steam_unavailable(&mut snapshot, &code);
            return Ok(snapshot);
        }
    };
    let app_id = match settings::get_steam_app_id(&state, &game_id) {
        Ok(Some(app_id)) => app_id,
        Ok(None) => {
            mark_activity_steam_unavailable(&mut snapshot, "STEAM_METADATA_NOT_AVAILABLE");
            return Ok(snapshot);
        }
        Err(error) => {
            let code = command_error(&state, correlation_id, "get_steam_app_id", error);
            mark_activity_steam_unavailable(&mut snapshot, &code);
            return Ok(snapshot);
        }
    };
    match steam::fetch_friends_playing(&credentials.steam_id64, &credentials.api_key, app_id).await
    {
        Ok(friends) => {
            snapshot.friends = friends
                .into_iter()
                .map(|friend| settings::ActivityFriend {
                    steam_id64: friend.steam_id64,
                    persona_name: friend.persona_name,
                    avatar_url: friend.avatar_url,
                    persona_state: friend.persona_state,
                    game_name: friend.game_name,
                    game_id: friend.game_id,
                })
                .collect();
            snapshot.friends_status = if snapshot.friends.is_empty() {
                "no-data".to_string()
            } else {
                "ready".to_string()
            };
            mark_activity_steam_ready(&mut snapshot);
        }
        Err(error) => {
            let code = steam_command_error(error);
            state.log(
                correlation_id,
                "STEAM_FRIENDS_ERROR",
                &format!("game_id={} error_code={code}", game_id),
            );
            snapshot.friends_status = if code == "STEAM_OFFLINE" {
                "offline".to_string()
            } else {
                "unavailable".to_string()
            };
            mark_activity_steam_unavailable(&mut snapshot, &code);
        }
    }
    Ok(snapshot)
}

#[tauri::command]
fn start_game_session(state: State<'_, DatabaseState>, game_id: String) -> Result<i64, String> {
    map_command_result(
        &state,
        "game-session-start",
        "start_game_session",
        settings::start_game_session(&state, &game_id),
    )
}

#[tauri::command]
fn end_game_session(
    state: State<'_, DatabaseState>,
    game_id: String,
    session_id: i64,
    interrupted: bool,
) -> Result<(), String> {
    map_command_result(
        &state,
        "game-session-end",
        "end_game_session",
        settings::end_game_session(&state, &game_id, session_id, interrupted),
    )
}

#[tauri::command]
fn get_display_modes() -> Result<Vec<display::DisplayMode>, String> {
    display::enumerate_modes()
}

#[tauri::command]
fn get_current_display_mode() -> Result<display::DisplayMode, String> {
    display::current_mode(None)
}

#[tauri::command]
fn get_display_profile(
    state: State<'_, DatabaseState>,
    game_id: String,
) -> Result<display::DisplayProfile, String> {
    map_command_result(
        &state,
        "display-profile-read",
        "get_display_profile",
        settings::get_display_profile(&state, &game_id),
    )
}

#[tauri::command]
fn set_display_profile(
    state: State<'_, DatabaseState>,
    mut profile: display::DisplayProfile,
) -> Result<display::DisplayProfile, String> {
    if profile.enabled {
        let display_id = profile
            .display_id
            .clone()
            .unwrap_or(display::primary_display_id()?);
        let (Some(width), Some(height), Some(refresh_rate)) =
            (profile.width, profile.height, profile.refresh_rate)
        else {
            return Err("DISPLAY_PROFILE_MODE_REQUIRED".to_string());
        };
        let available = display::enumerate_modes()?;
        if !available.iter().any(|mode| {
            mode.display_id == display_id
                && mode.width == width
                && mode.height == height
                && mode.refresh_rate == refresh_rate
        }) {
            return Err("DISPLAY_MODE_UNAVAILABLE".to_string());
        }
        profile.display_id = Some(display_id);
    } else {
        profile.display_id = None;
        profile.device_name = None;
        profile.width = None;
        profile.height = None;
        profile.refresh_rate = None;
    }
    map_command_result(
        &state,
        "display-profile-save",
        "set_display_profile",
        settings::save_display_profile(&state, &profile),
    )
}

#[tauri::command]
fn reset_display_profile(state: State<'_, DatabaseState>, game_id: String) -> Result<(), String> {
    map_command_result(
        &state,
        "display-profile-reset",
        "reset_display_profile",
        settings::reset_display_profile(&state, &game_id),
    )
}

#[tauri::command]
fn get_frame_generation_profile(
    state: State<'_, DatabaseState>,
    game_id: String,
) -> Result<settings::FrameGenerationProfile, String> {
    map_command_result(
        &state,
        "frame-generation-read",
        "get_frame_generation_profile",
        settings::get_frame_generation_profile(&state, &game_id),
    )
}

#[tauri::command]
fn set_frame_generation_profile(
    state: State<'_, DatabaseState>,
    profile: settings::FrameGenerationProfile,
) -> Result<settings::FrameGenerationProfile, String> {
    if profile.provider != "lossless-scaling"
        || profile.mode != "FIXED"
        || !matches!(profile.multiplier, 2 | 3 | 4)
        || profile.auto_scale_delay < 0
    {
        return Err("FRAME_GENERATION_PROFILE_INVALID".to_string());
    }
    let saved = settings::save_frame_generation_profile(&state, &profile)
        .map_err(|error| command_error(&state, "frame-generation-save", "save_intent", error))?;
    let Some(target) = saved.target_executable.as_deref() else {
        return Ok(saved);
    };
    if !std::path::Path::new(target).is_file() {
        return Ok(saved);
    }
    let provider = lossless_scaling::LosslessScalingProvider;
    match provider.synchronize_if_needed(&saved) {
        Ok(sync) => Ok(settings::FrameGenerationProfile {
            restart_required: sync.restart_required,
            ..saved
        }),
        Err(error)
            if error == "LOSSLESS_SCALING_NOT_INSTALLED"
                || error == "LOSSLESS_SCALING_SETTINGS_INVALID"
                || error == "LOSSLESS_SCALING_DEFAULT_PROFILE_MISSING" =>
        {
            Ok(saved)
        }
        Err(error) => Err(error),
    }
}

#[tauri::command]
fn get_lossless_scaling_status() -> frame_generation::LosslessScalingStatus {
    lossless_scaling::LosslessScalingProvider.status()
}

#[tauri::command]
fn open_lossless_scaling() -> Result<(), String> {
    lossless_scaling::LosslessScalingProvider.open_application()
}

#[tauri::command]
fn restore_lossless_scaling_backup() -> Result<(), String> {
    lossless_scaling::LosslessScalingProvider.restore_backup()
}

#[tauri::command]
fn restart_lossless_scaling(service: State<'_, SteamGameSessionService>) -> Result<(), String> {
    if matches!(
        service.current_status().state,
        game_session::GameSessionState::Preparing
            | game_session::GameSessionState::Launching
            | game_session::GameSessionState::Running
            | game_session::GameSessionState::Finishing
    ) {
        return Err("LOSSLESS_SCALING_RESTART_BLOCKED_DURING_SESSION".to_string());
    }
    lossless_scaling::LosslessScalingProvider.restart_background()
}

#[tauri::command]
fn get_pending_display_restore(
    state: State<'_, DatabaseState>,
) -> Result<Option<display::PendingDisplayRestore>, String> {
    map_command_result(
        &state,
        "display-restore-read",
        "get_pending_display_restore",
        settings::get_pending_display_restore(&state),
    )
}

#[tauri::command]
fn restore_pending_display_mode(state: State<'_, DatabaseState>) -> Result<(), String> {
    let pending = settings::get_pending_display_restore(&state)
        .map_err(|error| command_error(&state, "display-restore", "read_pending", error))?;
    let Some(pending) = pending else {
        return Ok(());
    };
    display::restore_mode(&pending)?;
    settings::clear_pending_display_restore(&state)
        .map_err(|error| command_error(&state, "display-restore", "clear_pending", error))
}

#[tauri::command]
fn start_steam_game_session(
    app: AppHandle,
    service: State<'_, SteamGameSessionService>,
    game_id: String,
    steam_app_id: i64,
) -> Result<GameSessionStatus, String> {
    service
        .start(app, game_id, steam_app_id)
        .map_err(game_session_command_error)
}

#[tauri::command]
fn get_steam_game_session(service: State<'_, SteamGameSessionService>) -> GameSessionStatus {
    service.current_status()
}

#[tauri::command]
fn dismiss_steam_game_session(
    app: AppHandle,
    service: State<'_, SteamGameSessionService>,
) -> Result<GameSessionStatus, String> {
    service.dismiss(app).map_err(game_session_command_error)
}

#[tauri::command]
fn minimize_lumadeck_window(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "MAIN_WINDOW_NOT_FOUND".to_string())?;
    window.minimize().map_err(|error| error.to_string())
}

#[tauri::command]
fn restore_lumadeck_window(app: AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "MAIN_WINDOW_NOT_FOUND".to_string())?;
    window.unminimize().map_err(|error| error.to_string())?;
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())
}

#[tauri::command]
async fn refresh_steam_game_metrics(
    state: State<'_, DatabaseState>,
    game_id: String,
) -> Result<settings::SteamGameMetrics, String> {
    let correlation_id = "steam-game-metrics-refresh";
    let result = async {
        let game = settings::get_steam_game_for_metadata(&state, &game_id)
            .map_err(|error| {
                command_error(&state, correlation_id, "get_steam_game_for_metrics", error)
            })?
            .ok_or_else(|| "GAME_NOT_FOUND".to_string())?;
        let active_players = steam::fetch_current_players(game.app_id)
            .await
            .map_err(steam_command_error)?;
        settings::update_steam_active_players(&state, &game_id, active_players).map_err(
            |error| command_error(&state, correlation_id, "update_steam_active_players", error),
        )?;
        let metrics = settings::get_steam_game_metrics(&state, &game_id).map_err(|error| {
            command_error(&state, correlation_id, "get_steam_game_metrics", error)
        })?;
        state.log(
            correlation_id,
            "STEAM_GAME_METRICS_REFRESH_SUCCESS",
            &format!(
                "game_id={} app_id={} active_players={}",
                game_id, game.app_id, active_players
            ),
        );
        Ok(metrics)
    }
    .await;
    if let Err(error) = &result {
        state.log(
            correlation_id,
            "COMMAND_ERROR",
            &format!("command=refresh_steam_game_metrics error_code={error}"),
        );
    }
    result
}

#[tauri::command]
async fn refresh_steam_game_achievements(
    state: State<'_, DatabaseState>,
    game_id: String,
) -> Result<settings::SteamGameMetrics, String> {
    let correlation_id = "steam-game-achievements-refresh";
    if state.steam_sync_running.load(Ordering::SeqCst)
        || state.steam_image_sync_running.load(Ordering::SeqCst)
        || state
            .steam_achievement_sync_running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
    {
        return Err("STEAM_ACHIEVEMENT_SYNC_ALREADY_RUNNING".to_string());
    }
    let result = async {
        let credentials = settings::get_steam_credentials(&state).map_err(|error| {
            command_error(&state, correlation_id, "get_steam_credentials", error)
        })?;
        let game = settings::get_steam_game_for_metadata(&state, &game_id)
            .map_err(|error| {
                command_error(
                    &state,
                    correlation_id,
                    "get_steam_game_for_achievements",
                    error,
                )
            })?
            .ok_or_else(|| "GAME_NOT_FOUND".to_string())?;
        let snapshot = steam::fetch_game_achievements(
            &credentials.steam_id64,
            &credentials.api_key,
            game.app_id,
        )
        .await
        .map_err(steam_command_error)?;
        settings::save_steam_achievements(
            &state,
            game.app_id,
            &snapshot.achievements,
            &snapshot.genres,
            snapshot.total,
        )
        .map_err(|error| command_error(&state, correlation_id, "save_steam_achievements", error))?;
        let metrics = settings::get_steam_game_metrics(&state, &game_id).map_err(|error| {
            command_error(&state, correlation_id, "get_steam_game_metrics", error)
        })?;
        state.log(
            correlation_id,
            "STEAM_GAME_ACHIEVEMENTS_REFRESH_SUCCESS",
            &format!(
                "game_id={} app_id={} achievement_count={} unlocked_count={}",
                game_id,
                game.app_id,
                snapshot.total,
                snapshot
                    .achievements
                    .iter()
                    .filter(|achievement| achievement.achieved)
                    .count()
            ),
        );
        Ok(metrics)
    }
    .await;
    state
        .steam_achievement_sync_running
        .store(false, Ordering::SeqCst);
    result
}

#[tauri::command]
async fn download_steam_game_media(
    state: State<'_, DatabaseState>,
    game_id: String,
) -> Result<i64, String> {
    let correlation_id = "steam-game-media-download";
    if state.steam_sync_running.load(Ordering::SeqCst)
        || state.steam_metadata_sync_running.load(Ordering::SeqCst)
        || state.steam_achievement_sync_running.load(Ordering::SeqCst)
        || state
            .steam_image_sync_running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
    {
        return Err("STEAM_IMAGE_SYNC_ALREADY_RUNNING".to_string());
    }
    state
        .steam_image_sync_cancel_requested
        .store(false, Ordering::SeqCst);
    state.steam_image_sync_progress.store(0, Ordering::SeqCst);
    state.steam_image_sync_total.store(0, Ordering::SeqCst);

    let result = async {
        let all_sources = settings::get_steam_image_sources(&state).map_err(|error| {
            command_error(&state, correlation_id, "get_steam_game_media_sources", error)
        })?;
        let selected_sources = all_sources
            .into_iter()
            .filter(|source| {
                source.game_id == game_id
                    && matches!(
                        source.asset_type.as_str(),
                        "vertical_cover" | "horizontal_cover" | "logo" | "hero" | "icon"
                    )
            })
            .collect::<Vec<_>>();
        if selected_sources.is_empty() {
            return Err("GAME_NOT_FOUND".to_string());
        }

        state.log(
            correlation_id,
            "STEAM_GAME_MEDIA_SOURCE_REFRESH_START",
            &format!("game_id={game_id} source_count={}", selected_sources.len()),
        );
        let refreshed = steam::refresh_steam_image_sources_for_game(
            selected_sources,
            state.steam_image_sync_cancel_requested.clone(),
        )
        .await
        .map_err(steam_image_command_error)?;
        if !refreshed.failed_errors.is_empty() {
            state.log(
                correlation_id,
                "STEAM_GAME_MEDIA_SOURCE_REFRESH_FAILURES",
                &refreshed.failed_errors.join(";"),
            );
        }
        settings::upsert_steam_image_sources(&state, &refreshed.sources).map_err(|error| {
            command_error(&state, correlation_id, "upsert_steam_game_media_sources", error)
        })?;

        let download_sources = refreshed
            .sources
            .into_iter()
            .filter(|source| {
                matches!(
                    source.asset_type.as_str(),
                    "vertical_cover" | "horizontal_cover" | "logo" | "hero" | "icon"
                )
            })
            .collect::<Vec<_>>();
        let total = download_sources.len();
        state.steam_image_sync_total.store(total, Ordering::SeqCst);
        let batch = steam::download_image_assets(
            download_sources,
            state.data_directory.steam_images_directory(),
            state.steam_image_sync_cancel_requested.clone(),
            state.steam_image_sync_progress.clone(),
        )
        .await
        .map_err(steam_image_command_error)?;
        if state
            .steam_image_sync_cancel_requested
            .load(Ordering::SeqCst)
        {
            return Err("STEAM_IMAGE_SYNC_CANCELLED".to_string());
        }
        settings::persist_steam_image_records(&state, &batch.records).map_err(|error| {
            command_error(&state, correlation_id, "persist_steam_game_media", error)
        })?;
        state.log(
            correlation_id,
            "STEAM_GAME_MEDIA_DOWNLOAD_RESULT",
            &format!(
                "game_id={game_id} requested_count={total} downloaded_count={} skipped_count={} failed_count={}",
                batch.records.len(),
                batch.skipped_count,
                batch.failures.len()
            ),
        );
        if !batch.failures.is_empty() {
            let failures = batch
                .failures
                .iter()
                .map(|failure| {
                    format!(
                        "asset_type={} external_id={} reason={} url={}",
                        failure.asset_type, failure.external_id, failure.reason, failure.source_url
                    )
                })
                .collect::<Vec<_>>();
            state.log(
                correlation_id,
                "STEAM_GAME_MEDIA_DOWNLOAD_FAILURES",
                &failures.join(";"),
            );
        }
        if batch.records.is_empty() {
            return Err("STEAM_IMAGE_DOWNLOAD_FAILED".to_string());
        }
        Ok(batch.records.len() as i64)
    }
    .await;

    state
        .steam_image_sync_running
        .store(false, Ordering::SeqCst);
    if let Err(error) = &result {
        state.log(
            correlation_id,
            "COMMAND_ERROR",
            &format!("command=download_steam_game_media error_code={error}"),
        );
    }
    result
}

#[tauri::command]
fn cancel_steam_library_sync(
    state: State<'_, DatabaseState>,
) -> Result<settings::SteamSyncStatus, String> {
    if state.steam_sync_running.load(Ordering::SeqCst) {
        state
            .steam_sync_cancel_requested
            .store(true, Ordering::SeqCst);
    }
    map_command_result(
        &state,
        "steam-sync-cancel",
        "cancel_steam_library_sync",
        settings::get_steam_sync_status(&state),
    )
}

#[tauri::command]
fn cancel_steam_image_sync(
    state: State<'_, DatabaseState>,
) -> Result<settings::SteamImageSyncStatus, String> {
    if state.steam_image_sync_running.load(Ordering::SeqCst) {
        state
            .steam_image_sync_cancel_requested
            .store(true, Ordering::SeqCst);
    }
    map_command_result(
        &state,
        "steam-image-sync-cancel",
        "cancel_steam_image_sync",
        settings::get_steam_image_sync_status(&state),
    )
}

#[tauri::command]
async fn sync_steam_library(
    state: State<'_, DatabaseState>,
) -> Result<settings::SteamSyncResult, String> {
    if state.steam_image_sync_running.load(Ordering::SeqCst) {
        return Err("STEAM_SYNC_ALREADY_RUNNING".to_string());
    }
    if state
        .steam_sync_running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("STEAM_SYNC_ALREADY_RUNNING".to_string());
    }
    state
        .steam_sync_cancel_requested
        .store(false, Ordering::SeqCst);
    state.steam_sync_progress.store(0, Ordering::SeqCst);
    state.steam_sync_total.store(0, Ordering::SeqCst);
    let started = Instant::now();
    let started_at = unix_timestamp();
    let result = async {
        settings::begin_steam_sync(&state, 0, &started_at)
            .map_err(|error| command_error(&state, "steam-sync", "sync_steam_library", error))?;
        let sync_settings = settings::get_steam_library_sync_settings(&state)
            .map_err(|error| command_error(&state, "steam-sync", "sync_steam_library", error))?;
        let installed_app_ids = if sync_settings.scope == "installed" {
            let detected = steam::find_installed_app_ids()
                .ok_or_else(|| "STEAM_INSTALLED_GAMES_UNAVAILABLE".to_string())?;
            state.log(
                "steam-sync",
                "STEAM_INSTALLED_APPS_DETECTED",
                &format!("scope=installed app_id_count={}", detected.len()),
            );
            Some(detected)
        } else {
            state.log("steam-sync", "STEAM_SYNC_SCOPE_RESOLVED", "scope=all");
            None
        };
        let credentials = match settings::get_steam_credentials(&state) {
            Ok(credentials) => {
                state.log(
                    "steam-sync",
                    "STEAM_CREDENTIALS_AVAILABLE",
                    "steam_id64_present=true api_key_present=true",
                );
                credentials
            }
            Err(error) => {
                state.log(
                    "steam-sync",
                    "STEAM_CREDENTIALS_UNAVAILABLE",
                    &format!("error_variant={}", error.variant_name()),
                );
                return Err(command_error(
                    &state,
                    "steam-sync",
                    "sync_steam_library",
                    error,
                ));
            }
        };
        let cache = settings::get_steam_cache(&state)
            .map_err(|error| command_error(&state, "steam-sync", "sync_steam_library", error))?;
        let games = match steam::fetch_library(
            &credentials.steam_id64,
            &credentials.api_key,
            &cache,
            state.steam_sync_cancel_requested.clone(),
            state.steam_sync_progress.clone(),
            state.steam_sync_total.clone(),
            installed_app_ids.as_ref(),
        )
        .await
        {
            Ok(games) => games,
            Err(SteamError::Cancelled) => {
                let _ = settings::cancel_steam_sync(&state, started.elapsed().as_millis() as i64);
                return Err("STEAM_SYNC_CANCELLED".to_string());
            }
            Err(error) => {
                return Err(steam_command_error(error));
            }
        };
        state.log(
            "steam-sync",
            "STEAM_LIBRARY_FETCH_SUCCESS",
            &format!(
                "scope={} installed_app_id_count={} owned_games_after_filter={}",
                sync_settings.scope,
                installed_app_ids.as_ref().map_or(0, |ids| ids.len()),
                games.len()
            ),
        );
        state
            .steam_sync_progress
            .store(games.len(), Ordering::SeqCst);
        state.steam_sync_total.store(games.len(), Ordering::SeqCst);
        settings::update_steam_sync_progress(&state, games.len() as i64, games.len() as i64, None)
            .map_err(|error| command_error(&state, "steam-sync", "sync_steam_library", error))?;
        if state.steam_sync_cancel_requested.load(Ordering::SeqCst) {
            settings::cancel_steam_sync(&state, started.elapsed().as_millis() as i64).map_err(
                |error| command_error(&state, "steam-sync", "sync_steam_library", error),
            )?;
            return Err("STEAM_SYNC_CANCELLED".to_string());
        }
        settings::sync_steam_games(
            &state,
            &games,
            sync_settings.scope == "installed",
            started.elapsed().as_millis() as i64,
            &unix_timestamp(),
        )
        .map_err(|error| command_error(&state, "steam-sync", "sync_steam_library", error))
    }
    .await;
    if let Err(error) = &result {
        state.log(
            "steam-sync",
            "STEAM_LIBRARY_SYNC_ERROR",
            &format!("error_code={error}"),
        );
        if error != "STEAM_SYNC_CANCELLED" {
            let _ = settings::fail_steam_sync(&state, started.elapsed().as_millis() as i64, error);
        }
    }
    state.steam_sync_running.store(false, Ordering::SeqCst);
    result
}

#[tauri::command]
async fn sync_steam_achievements(
    state: State<'_, DatabaseState>,
) -> Result<settings::SteamAchievementSyncResult, String> {
    state.log(
        "steam-achievements",
        "COMMAND_ENTER",
        "command=sync_steam_achievements source=existing_catalog_only",
    );
    if state.steam_sync_running.load(Ordering::SeqCst)
        || state.steam_image_sync_running.load(Ordering::SeqCst)
        || state
            .steam_achievement_sync_running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
    {
        return Err("STEAM_ACHIEVEMENT_SYNC_ALREADY_RUNNING".to_string());
    }
    let started = Instant::now();
    let result = async {
        let credentials = settings::get_steam_credentials(&state).map_err(|error| {
            command_error(
                &state,
                "steam-achievements",
                "sync_steam_achievements",
                error,
            )
        })?;
        let app_ids = settings::get_steam_app_ids(&state).map_err(|error| {
            command_error(
                &state,
                "steam-achievements",
                "sync_steam_achievements",
                error,
            )
        })?;
        state.log(
            "steam-achievements",
            "APP_IDS_RESOLVED",
            &format!("app_id_count={}", app_ids.len()),
        );
        let cleared_count = settings::clear_stale_steam_achievements(&state).map_err(|error| {
            command_error(
                &state,
                "steam-achievements",
                "sync_steam_achievements",
                error,
            )
        })?;
        state.log(
            "steam-achievements",
            "STALE_VALUES_CLEARED",
            &format!("cleared_count={cleared_count}"),
        );
        let mut updated_count = 0_i64;
        let mut skipped_count = 0_i64;
        for chunk in app_ids.chunks(8) {
            let mut handles = Vec::with_capacity(chunk.len());
            for app_id in chunk {
                let steam_id64 = credentials.steam_id64.clone();
                let api_key = credentials.api_key.clone();
                let app_id = *app_id;
                handles.push((
                    app_id,
                    tauri::async_runtime::spawn(async move {
                        steam::fetch_game_achievements(&steam_id64, &api_key, app_id).await
                    }),
                ));
            }
            for (app_id, handle) in handles {
                match handle.await {
                    Ok(Ok(snapshot)) => {
                        if settings::save_steam_achievements(
                            &state,
                            app_id,
                            &snapshot.achievements,
                            &snapshot.genres,
                            snapshot.total,
                        )
                        .map_err(|error| {
                            command_error(
                                &state,
                                "steam-achievements",
                                "sync_steam_achievements",
                                error,
                            )
                        })? {
                            updated_count += 1;
                        } else {
                            skipped_count += 1;
                        }
                    }
                    Ok(Err(error)) => {
                        skipped_count += 1;
                        state.log(
                            "steam-achievements",
                            "GAME_SKIPPED",
                            &format!("app_id={} error_variant={error:?}", app_id),
                        );
                    }
                    Err(error) => {
                        skipped_count += 1;
                        state.log(
                            "steam-achievements",
                            "GAME_TASK_FAILED",
                            &format!("app_id={} join_error={error}", app_id),
                        );
                    }
                }
            }
        }
        let status = if !app_ids.is_empty() && updated_count == 0 {
            "error"
        } else {
            "completed"
        };
        state.log(
            "steam-achievements",
            "COMMAND_RETURN_SUCCESS",
            &format!(
                "status={} found_count={} updated_count={} skipped_count={}",
                status,
                app_ids.len(),
                updated_count,
                skipped_count
            ),
        );
        Ok(settings::SteamAchievementSyncResult {
            status: status.to_string(),
            found_count: app_ids.len() as i64,
            updated_count,
            skipped_count,
            duration_ms: started.elapsed().as_millis() as i64,
            completed_at: Some(unix_timestamp()),
        })
    }
    .await;
    state
        .steam_achievement_sync_running
        .store(false, Ordering::SeqCst);
    result
}

#[tauri::command]
async fn sync_steam_images(
    state: State<'_, DatabaseState>,
) -> Result<settings::SteamImageSyncResult, String> {
    if state.steam_sync_running.load(Ordering::SeqCst) {
        return Err("STEAM_IMAGE_SYNC_ALREADY_RUNNING".to_string());
    }
    if state
        .steam_image_sync_running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("STEAM_IMAGE_SYNC_ALREADY_RUNNING".to_string());
    }
    state
        .steam_image_sync_cancel_requested
        .store(false, Ordering::SeqCst);
    state.steam_image_sync_progress.store(0, Ordering::SeqCst);
    state.steam_image_sync_total.store(0, Ordering::SeqCst);
    let started = Instant::now();
    let started_at = unix_timestamp();
    state.log(
        "steam-image-sync",
        "STEAM_IMAGE_SYNC_START",
        "image_cache_rebuild=true",
    );
    let result = async {
        let sync_settings = settings::get_steam_library_sync_settings(&state).map_err(|error| {
            command_error(&state, "steam-image-sync", "sync_steam_images", error)
        })?;
        state.log(
            "steam-image-sync",
            "STEAM_IMAGE_SETTINGS_RESOLVED",
            &format!("scope={}", sync_settings.scope),
        );
        let installed_app_ids = if sync_settings.scope == "installed" {
            state.log(
                "steam-image-sync",
                "STEAM_IMAGE_INSTALLED_SCAN_START",
                "scanning_steam_library_roots=true",
            );
            let ids = steam::find_installed_app_ids()
                .ok_or_else(|| "STEAM_INSTALLED_GAMES_UNAVAILABLE".to_string())?;
            state.log(
                "steam-image-sync",
                "STEAM_IMAGE_INSTALLED_SCAN_RESULT",
                &format!("installed_app_id_count={}", ids.len()),
            );
            Some(ids)
        } else {
            None
        };
        let mut sources = settings::get_steam_image_sources(&state).map_err(|error| {
            command_error(&state, "steam-image-sync", "sync_steam_images", error)
        })?;
        if let Some(installed_app_ids) = installed_app_ids.as_ref() {
            sources.retain(|source| installed_app_ids.contains(&source.app_id));
        }
        state.log(
            "steam-image-sync",
            "STEAM_IMAGE_SCOPE_RESOLVED",
            &format!(
                "scope={} selected_app_id_count={} selected_asset_source_count={}",
                sync_settings.scope,
                installed_app_ids.as_ref().map_or(0, |ids| ids.len()),
                sources.len()
            ),
        );
        let refreshed = steam::refresh_steam_image_sources(
            sources,
            state.steam_image_sync_cancel_requested.clone(),
        )
        .await
        .map_err(steam_image_command_error)?;
        state.log(
            "steam-image-sync",
            "STEAM_IMAGE_SOURCE_REFRESH_RESULT",
            &format!(
                "requested_app_count={} returned_app_count={} screenshot_source_count={} failed_app_count={} apps_with_screenshots={} first_error={}",
                refreshed.requested_app_count,
                refreshed.returned_app_count,
                refreshed.screenshot_source_count,
                refreshed.failed_app_ids.len(),
                refreshed.apps_with_screenshots,
                refreshed.failed_errors.first().map(String::as_str).unwrap_or("none")
            ),
        );
        if !refreshed.failed_errors.is_empty() {
            state.log(
                "steam-image-sync",
                "STEAM_IMAGE_SOURCE_REFRESH_FAILURES",
                &refreshed.failed_errors.join(";")
            );
        }
        let source_refresh_error = if !refreshed.failed_app_ids.is_empty() {
            let error = format!(
                "STEAM_IMAGE_SOURCE_REFRESH_FAILED:{}:{}",
                refreshed.failed_app_ids.len(),
                refreshed.failed_errors.first().map(String::as_str).unwrap_or("unknown")
            );
            state.log(
                "steam-image-sync",
                "STEAM_IMAGE_PARTIAL_SOURCE_REFRESH",
                "continuing_with_successful_app_sources=true",
            );
            Some(error)
        } else {
            None
        };
        let refreshed_sources = refreshed.sources;
        settings::upsert_steam_image_sources(&state, &refreshed_sources).map_err(|error| {
            command_error(&state, "steam-image-sync", "sync_steam_images", error)
        })?;
        let mut sources = settings::get_steam_image_sources(&state).map_err(|error| {
            command_error(&state, "steam-image-sync", "sync_steam_images", error)
        })?;
        if let Some(installed_app_ids) = installed_app_ids.as_ref() {
            sources.retain(|source| installed_app_ids.contains(&source.app_id));
        }
        let total = sources.len();
        let screenshot_source_count = sources
            .iter()
            .filter(|source| source.asset_type == "screenshot")
            .count();
        state.log(
            "steam-image-sync",
            "STEAM_IMAGE_DOWNLOAD_PLAN",
            &format!(
                "scope={} asset_source_count={} screenshot_source_count={}",
                sync_settings.scope, total, screenshot_source_count
            ),
        );
        state.steam_image_sync_total.store(total, Ordering::SeqCst);
        settings::begin_steam_image_sync(&state, total as i64, &started_at).map_err(|error| {
            command_error(&state, "steam-image-sync", "sync_steam_images", error)
        })?;
        let batch = match steam::download_image_assets(
            sources,
            state.data_directory.steam_images_directory(),
            state.steam_image_sync_cancel_requested.clone(),
            state.steam_image_sync_progress.clone(),
        )
        .await
        {
            Ok(batch) => batch,
            Err(SteamError::Cancelled) => {
                let _ =
                    settings::cancel_steam_image_sync(&state, started.elapsed().as_millis() as i64);
                return Err("STEAM_IMAGE_SYNC_CANCELLED".to_string());
            }
            Err(error) => {
                let _ = settings::fail_steam_image_sync(
                    &state,
                    started.elapsed().as_millis() as i64,
                    &error.to_string(),
                );
                return Err(steam_image_command_error(error));
            }
        };
        settings::update_steam_image_sync_progress(&state, total as i64, total as i64, None)
            .map_err(|error| {
                command_error(&state, "steam-image-sync", "sync_steam_images", error)
            })?;
        if state
            .steam_image_sync_cancel_requested
            .load(Ordering::SeqCst)
        {
            settings::cancel_steam_image_sync(&state, started.elapsed().as_millis() as i64)
                .map_err(|error| {
                    command_error(&state, "steam-image-sync", "sync_steam_images", error)
                })?;
            return Err("STEAM_IMAGE_SYNC_CANCELLED".to_string());
        }
        let image_result = settings::sync_steam_image_records(
            &state,
            &batch.records,
            total as i64,
            batch.skipped_count as i64,
            started.elapsed().as_millis() as i64,
            &unix_timestamp(),
        )
        .map_err(|error| command_error(&state, "steam-image-sync", "sync_steam_images", error))
        .inspect(|value| {
            state.log(
                "steam-image-sync",
                "STEAM_IMAGE_DOWNLOAD_RESULT",
                &format!(
                    "asset_source_count={} screenshot_source_count={} screenshot_downloaded_count={} screenshot_skipped_count={} failed_asset_count={}",
                    total,
                    batch.screenshot_source_count,
                    batch.screenshot_downloaded_count,
                    batch.screenshot_skipped_count,
                    batch.failures.len()
                ),
            );
            if !batch.failures.is_empty() {
                let failures = batch
                    .failures
                    .iter()
                    .map(|failure| {
                        format!(
                            "app_id={} asset_type={} external_id={} reason={} url={}",
                            failure.app_id,
                            failure.asset_type,
                            failure.external_id,
                            failure.reason,
                            failure.source_url
                        )
                    })
                    .collect::<Vec<_>>();
                state.log(
                    "steam-image-sync",
                    "STEAM_IMAGE_DOWNLOAD_FAILURES",
                    &failures.join(";"),
                );
            }
            let _ = value;
        });
        if let Some(source_refresh_error) = source_refresh_error {
            if image_result.is_ok() {
                settings::fail_steam_image_sync(
                    &state,
                    started.elapsed().as_millis() as i64,
                    &source_refresh_error,
                )
                .map_err(|error| {
                    command_error(&state, "steam-image-sync", "sync_steam_images", error)
                })?;
                return Err(source_refresh_error);
            }
        }
        image_result
    }
    .await;
    match &result {
        Ok(value) => state.log(
            "steam-image-sync",
            "STEAM_IMAGE_SYNC_SUCCESS",
            &format!(
                "found_count={} downloaded_count={} skipped_count={}",
                value.found_count, value.downloaded_count, value.skipped_count
            ),
        ),
        Err(error) => state.log(
            "steam-image-sync",
            "STEAM_IMAGE_SYNC_ERROR",
            &format!("error_code={error}"),
        ),
    }
    state
        .steam_image_sync_running
        .store(false, Ordering::SeqCst);
    result
}

fn unix_timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn mark_activity_steam_ready(snapshot: &mut settings::ActivitySnapshot) {
    if let Some(source) = snapshot
        .sources
        .iter_mut()
        .find(|source| source.source == "steam")
    {
        source.status = "ready".to_string();
        source.error = None;
    }
}

fn mark_activity_steam_unavailable(snapshot: &mut settings::ActivitySnapshot, error: &str) {
    snapshot.friends_status = if error == "STEAM_OFFLINE" {
        "offline".to_string()
    } else {
        "unavailable".to_string()
    };
    if let Some(source) = snapshot
        .sources
        .iter_mut()
        .find(|source| source.source == "steam")
    {
        source.status = snapshot.friends_status.clone();
        source.error = Some(error.to_string());
    }
}

fn steam_command_error(error: SteamError) -> String {
    match error {
        SteamError::Offline => "STEAM_OFFLINE".to_string(),
        SteamError::Api(_) => "STEAM_API_ERROR".to_string(),
        SteamError::InvalidResponse | SteamError::RequestSetup => {
            "STEAM_INVALID_RESPONSE".to_string()
        }
        SteamError::Cancelled => "STEAM_SYNC_CANCELLED".to_string(),
    }
}

fn hltb_command_error(error: hltb::HltbError) -> String {
    match error {
        hltb::HltbError::Offline => "HLTB_OFFLINE".to_string(),
        hltb::HltbError::Api(_) => "HLTB_API_ERROR".to_string(),
        hltb::HltbError::InvalidResponse => "HLTB_INVALID_RESPONSE".to_string(),
        hltb::HltbError::RequestSetup => "HLTB_REQUEST_ERROR".to_string(),
    }
}

fn steam_image_command_error(error: SteamError) -> String {
    match error {
        SteamError::Offline => "STEAM_OFFLINE".to_string(),
        SteamError::Api(_) => "STEAM_API_ERROR".to_string(),
        SteamError::InvalidResponse | SteamError::RequestSetup => {
            "STEAM_INVALID_RESPONSE".to_string()
        }
        SteamError::Cancelled => "STEAM_IMAGE_SYNC_CANCELLED".to_string(),
    }
}

fn storage_command_error(error: storage_migration::StorageMigrationError) -> String {
    match error {
        storage_migration::StorageMigrationError::AlreadyRunning => {
            "STORAGE_MIGRATION_ALREADY_RUNNING".to_string()
        }
        storage_migration::StorageMigrationError::SyncRunning => {
            "STORAGE_MIGRATION_BUSY".to_string()
        }
        storage_migration::StorageMigrationError::SameMode => {
            "STORAGE_MIGRATION_SAME_MODE".to_string()
        }
        storage_migration::StorageMigrationError::StateUnavailable => {
            "STORAGE_MIGRATION_STATE_UNAVAILABLE".to_string()
        }
        storage_migration::StorageMigrationError::Io(_) => "STORAGE_MIGRATION_IO_ERROR".to_string(),
        storage_migration::StorageMigrationError::Database(_) => {
            "STORAGE_MIGRATION_DATABASE_INVALID".to_string()
        }
        storage_migration::StorageMigrationError::Validation(_) => {
            "STORAGE_MIGRATION_VALIDATION_ERROR".to_string()
        }
    }
}

fn map_command_result<T>(
    state: &DatabaseState,
    correlation_id: &str,
    command: &str,
    result: Result<T, settings::DatabaseError>,
) -> Result<T, String> {
    match result {
        Ok(value) => {
            state.log(
                correlation_id,
                "COMMAND_RETURN_SUCCESS",
                &format!("command={command}"),
            );
            Ok(value)
        }
        Err(error) => Err(command_error(state, correlation_id, command, error)),
    }
}

fn command_error(
    state: &DatabaseState,
    correlation_id: &str,
    stage: &str,
    error: settings::DatabaseError,
) -> String {
    let sqlite = error
        .sqlite_diagnostic()
        .map(|value| format!(" sqlite={value}"))
        .unwrap_or_default();
    state.log(
        correlation_id,
        "COMMAND_ERROR",
        &format!(
            "stage={stage} error_variant={} display={} source_chain={}{}",
            error.variant_name(),
            error,
            error.source_chain(),
            sqlite
        ),
    );
    match error {
        settings::DatabaseError::InvalidSteamId => "INVALID_STEAM_ID".to_string(),
        settings::DatabaseError::InvalidApiKey => "INVALID_API_KEY".to_string(),
        settings::DatabaseError::InvalidSteamSyncScope => "INVALID_STEAM_SYNC_SCOPE".to_string(),
        settings::DatabaseError::GameNotFound => "GAME_NOT_FOUND".to_string(),
        settings::DatabaseError::SteamMetadataUnavailable => {
            "STEAM_METADATA_NOT_AVAILABLE".to_string()
        }
        settings::DatabaseError::AccountNotConfigured => "ACCOUNT_NOT_CONFIGURED".to_string(),
        settings::DatabaseError::UnsupportedProvider => "UNSUPPORTED_PROVIDER".to_string(),
        settings::DatabaseError::Credential(_) => "CREDENTIAL_UNAVAILABLE".to_string(),
        settings::DatabaseError::Path(_) => "DATABASE_PATH_UNAVAILABLE".to_string(),
        settings::DatabaseError::Sqlite(_) => "DATABASE_ERROR".to_string(),
    }
}

fn steamgriddb_command_error(
    state: &DatabaseState,
    correlation_id: &str,
    stage: &str,
    error: SteamGridDbError,
) -> String {
    state.log(
        correlation_id,
        "STEAMGRIDDB_COMMAND_ERROR",
        &format!("stage={stage} error_variant={error:?}"),
    );
    match error {
        SteamGridDbError::CredentialUnavailable => "CREDENTIAL_UNAVAILABLE".to_string(),
        SteamGridDbError::InvalidRequest => "ARTWORK_REQUEST_INVALID".to_string(),
        SteamGridDbError::Offline => "ARTWORK_SOURCE_OFFLINE".to_string(),
        SteamGridDbError::Timeout => "ARTWORK_SOURCE_TIMEOUT".to_string(),
        SteamGridDbError::Api(401) | SteamGridDbError::Api(403) => {
            "ARTWORK_CREDENTIAL_REJECTED".to_string()
        }
        SteamGridDbError::Api(_) => "ARTWORK_SOURCE_ERROR".to_string(),
        SteamGridDbError::RateLimited => "ARTWORK_RATE_LIMITED".to_string(),
        SteamGridDbError::InvalidResponse => "ARTWORK_INVALID_RESPONSE".to_string(),
        SteamGridDbError::GameUnresolved => "ARTWORK_GAME_NOT_FOUND".to_string(),
        SteamGridDbError::GameAmbiguous => "ARTWORK_GAME_AMBIGUOUS".to_string(),
        SteamGridDbError::CandidateExpired => "ARTWORK_CANDIDATE_EXPIRED".to_string(),
        SteamGridDbError::CandidateContextMismatch => "ARTWORK_CANDIDATE_INVALID".to_string(),
    }
}

fn artwork_command_error(
    state: &DatabaseState,
    stage: &str,
    error: ArtworkDownloadError,
) -> String {
    state.log(
        "steamgriddb-artwork-apply",
        "ARTWORK_COMMAND_ERROR",
        &format!("stage={stage} error_variant={error:?}"),
    );
    match error {
        ArtworkDownloadError::Candidate(error) => steamgriddb_command_error(
            state,
            "steamgriddb-artwork-apply",
            "candidate_context",
            error,
        ),
        ArtworkDownloadError::RequestSetup => "ARTWORK_REQUEST_ERROR".to_string(),
        ArtworkDownloadError::HostNotAllowed => "ARTWORK_HOST_NOT_ALLOWED".to_string(),
        ArtworkDownloadError::Offline => "ARTWORK_DOWNLOAD_OFFLINE".to_string(),
        ArtworkDownloadError::Timeout => "ARTWORK_DOWNLOAD_TIMEOUT".to_string(),
        ArtworkDownloadError::Http(_) => "ARTWORK_DOWNLOAD_ERROR".to_string(),
        ArtworkDownloadError::TooLarge => "ARTWORK_TOO_LARGE".to_string(),
        ArtworkDownloadError::InvalidImage => "ARTWORK_IMAGE_INVALID".to_string(),
        ArtworkDownloadError::AnimatedUnsupported => "ARTWORK_ANIMATION_UNSUPPORTED".to_string(),
        ArtworkDownloadError::InvalidDimensions => "ARTWORK_DIMENSIONS_INVALID".to_string(),
        ArtworkDownloadError::Compression => "ARTWORK_COMPRESSION_ERROR".to_string(),
        ArtworkDownloadError::Storage(_) => "ARTWORK_STORAGE_ERROR".to_string(),
    }
}

fn game_session_command_error(error: SessionCommandError) -> String {
    match error {
        SessionCommandError::AnotherGameSessionIsActive => {
            "ANOTHER_GAME_SESSION_ACTIVE".to_string()
        }
        SessionCommandError::NotDismissable => "GAME_SESSION_NOT_DISMISSABLE".to_string(),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let executable_path = std::env::current_exe()?;
            let data_directory =
                data_directory::DataDirectoryResolver::new(executable_path, app_data_dir);
            let database = settings::initialize(data_directory)
                .map_err(|error| -> Box<dyn std::error::Error> { Box::new(error) })?;
            if let Ok(Some(pending)) = settings::get_pending_display_restore(&database) {
                match display::restore_mode(&pending) {
                    Ok(_) => {
                        let _ = settings::clear_pending_display_restore(&database);
                        database.log(
                            "display-restore",
                            "RECOVERED",
                            "pending display mode restored during startup",
                        );
                    }
                    Err(error) => database.log(
                        "display-restore",
                        "RECOVERY_FAILED",
                        &format!("error={error}"),
                    ),
                }
            }
            app.manage(database);
            app.manage(SteamGameSessionService::default());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            search_steamgriddb_artwork,
            cancel_steamgriddb_artwork_search,
            apply_steamgriddb_artwork,
            restore_steamgriddb_artwork,
            get_current_steamgriddb_artwork,
            get_provider_configuration,
            save_steam_account_configuration,
            update_steam_id,
            replace_steam_api_key,
            disconnect_provider_account,
            get_database_status,
            get_storage_status,
            migrate_storage,
            get_steam_profile,
            get_hltb_settings,
            set_hltb_settings,
            get_steamgriddb_configuration,
            save_steamgriddb_api_key,
            delete_steamgriddb_api_key,
            get_hltb_sync_status,
            get_hltb_pending_matches,
            search_hltb_candidates,
            set_hltb_match_override,
            ignore_hltb_match,
            clear_hltb_match_override,
            sync_hltb_library,
            cancel_hltb_sync,
            get_library_games,
            set_game_favorite,
            get_game_activity,
            start_game_session,
            end_game_session,
            get_display_modes,
            get_current_display_mode,
            get_display_profile,
            set_display_profile,
            reset_display_profile,
            get_frame_generation_profile,
            set_frame_generation_profile,
            get_lossless_scaling_status,
            open_lossless_scaling,
            restore_lossless_scaling_backup,
            restart_lossless_scaling,
            get_pending_display_restore,
            restore_pending_display_mode,
            start_steam_game_session,
            get_steam_game_session,
            dismiss_steam_game_session,
            minimize_lumadeck_window,
            restore_lumadeck_window,
            refresh_steam_game_metadata,
            refresh_steam_game_metrics,
            refresh_steam_game_achievements,
            download_steam_game_media,
            get_steam_sync_status,
            get_steam_library_sync_settings,
            set_steam_library_sync_scope,
            cancel_steam_library_sync,
            sync_steam_library,
            sync_steam_achievements,
            get_steam_image_sync_status,
            cancel_steam_image_sync,
            sync_steam_images
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
