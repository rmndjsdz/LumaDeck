use crate::{
    artwork,
    settings::{self, ArtworkEnrichmentGame, ArtworkSlotState, DatabaseState},
    steamgriddb::{
        select_best_asset, ArtworkSlot, LocalGameIdentity, SteamGridDbClient, SteamGridDbError,
    },
};
use futures_util::stream::{self, StreamExt};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    future::Future,
    sync::atomic::Ordering,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const NEGATIVE_CACHE_TTL_SECONDS: i64 = 7 * 24 * 60 * 60;
const DEFAULT_MAX_DIMENSION: u32 = 4096;
const DEFAULT_CONCURRENCY: usize = 4;
const MAX_CONCURRENCY: usize = 8;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkEnrichmentRequest {
    #[serde(default)]
    pub game_ids: Vec<String>,
    pub scope: ArtworkEnrichmentScope,
    #[serde(default)]
    pub slots: Vec<ArtworkSlot>,
    #[serde(default = "default_max_dimension")]
    pub max_dimension: u32,
    #[serde(default = "default_concurrency")]
    pub concurrency: usize,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtworkEnrichmentScope {
    OnlyNonSteam,
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkEnrichmentStatus {
    pub status: String,
    pub processed_games: usize,
    pub total_games: usize,
    pub current_game: Option<String>,
    pub current_artwork: Option<String>,
    pub downloaded_assets: usize,
    pub already_complete_games: usize,
    pub no_result_games: usize,
    pub ambiguous_games: usize,
    pub error_count: usize,
    pub duration_ms: Option<u64>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub error_message: Option<String>,
}

impl Default for ArtworkEnrichmentStatus {
    fn default() -> Self {
        Self {
            status: "idle".to_string(),
            processed_games: 0,
            total_games: 0,
            current_game: None,
            current_artwork: None,
            downloaded_assets: 0,
            already_complete_games: 0,
            no_result_games: 0,
            ambiguous_games: 0,
            error_count: 0,
            duration_ms: None,
            started_at: None,
            completed_at: None,
            error_message: None,
        }
    }
}

#[derive(Debug, Default)]
struct GameOutcome {
    downloaded_assets: usize,
    had_missing_slot: bool,
    had_no_result: bool,
    ambiguous: bool,
    errors: usize,
}

pub async fn run(
    state: &DatabaseState,
    request: ArtworkEnrichmentRequest,
) -> Result<ArtworkEnrichmentStatus, String> {
    if state
        .artwork_enrichment_running
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("ARTWORK_ENRICHMENT_ALREADY_RUNNING".to_string());
    }
    state
        .artwork_enrichment_cancel_requested
        .store(false, Ordering::SeqCst);
    let started = Instant::now();
    let result = run_inner(state, request, started).await;
    if let Err(error) = &result {
        let mut status = get_status(state);
        status.status = "error".to_string();
        status.error_message = Some(error.clone());
        status.duration_ms = Some(started.elapsed().as_millis() as u64);
        status.completed_at = Some(timestamp());
        set_status(state, status.clone());
        if let Ok(summary) = serde_json::to_string(&status) {
            let _ = settings::save_artwork_enrichment_run(state, &summary, &status.status);
        }
    }
    state
        .artwork_enrichment_running
        .store(false, Ordering::SeqCst);
    state
        .artwork_enrichment_cancel_requested
        .store(false, Ordering::SeqCst);
    result
}

async fn run_inner(
    state: &DatabaseState,
    request: ArtworkEnrichmentRequest,
    started: Instant,
) -> Result<ArtworkEnrichmentStatus, String> {
    let status = ArtworkEnrichmentStatus {
        status: "running".to_string(),
        started_at: Some(timestamp()),
        ..ArtworkEnrichmentStatus::default()
    };
    set_status(state, status);

    let only_non_steam = matches!(request.scope, ArtworkEnrichmentScope::OnlyNonSteam);
    let mut games = settings::get_artwork_enrichment_games(state, only_non_steam)
        .map_err(|error| error.to_string())?;
    if !request.game_ids.is_empty() {
        let selected = request.game_ids.into_iter().collect::<HashSet<_>>();
        games.retain(|game| selected.contains(&game.id));
    }
    let slots = normalized_slots(&request.slots);
    let concurrency = request.concurrency.clamp(1, MAX_CONCURRENCY);
    let max_dimension = request.max_dimension.clamp(256, 8192);
    let api_key = settings::get_steamgriddb_api_key(state).map_err(|error| error.to_string())?;
    let client = SteamGridDbClient::new(api_key).map_err(|error| error.to_string())?;
    let _ = artwork::cleanup_artwork_temporary_files(state);

    let mut running_status = get_status(state);
    running_status.total_games = games.len();
    set_status(state, running_status);

    let _outcomes = stream::iter(
        games
            .into_iter()
            .map(|game| process_game(state, &client, game, slots.clone(), max_dimension)),
    )
    .buffer_unordered(concurrency)
    .collect::<Vec<_>>()
    .await;

    let mut final_status = get_status(state);
    final_status.status = if state
        .artwork_enrichment_cancel_requested
        .load(Ordering::SeqCst)
    {
        "cancelled".to_string()
    } else if final_status.error_count > 0 && final_status.downloaded_assets == 0 {
        "error".to_string()
    } else {
        "completed".to_string()
    };
    final_status.duration_ms = Some(started.elapsed().as_millis() as u64);
    final_status.completed_at = Some(timestamp());
    if final_status.status == "error" {
        final_status.error_message = Some("No se pudo completar ningún asset.".to_string());
    }
    settings::save_artwork_enrichment_run(
        state,
        &serde_json::to_string(&final_status).map_err(|error| error.to_string())?,
        &final_status.status,
    )
    .map_err(|error| error.to_string())?;
    set_status(state, final_status.clone());
    Ok(final_status)
}

async fn process_game(
    state: &DatabaseState,
    client: &SteamGridDbClient,
    game: ArtworkEnrichmentGame,
    slots: Vec<ArtworkSlot>,
    max_dimension: u32,
) -> GameOutcome {
    let outcome = process_game_inner(state, client, game, slots, max_dimension).await;
    record_outcome(state, &outcome);
    outcome
}

async fn process_game_inner(
    state: &DatabaseState,
    client: &SteamGridDbClient,
    game: ArtworkEnrichmentGame,
    slots: Vec<ArtworkSlot>,
    max_dimension: u32,
) -> GameOutcome {
    let mut outcome = GameOutcome::default();
    if state
        .artwork_enrichment_cancel_requested
        .load(Ordering::SeqCst)
    {
        return outcome;
    }
    let mut missing = Vec::new();
    for slot in slots {
        match settings::get_artwork_slot_state(state, &game.id, slot) {
            Ok(ArtworkSlotState::Missing) => {
                outcome.had_missing_slot = true;
                missing.push(slot);
            }
            Ok(_) => {}
            Err(_) => outcome.errors = outcome.errors.saturating_add(1),
        }
    }
    if missing.is_empty() {
        return outcome;
    }

    set_current(state, Some(game.title.clone()), None);
    let identity = LocalGameIdentity {
        local_game_id: game.id.clone(),
        title: game.title.clone(),
        steam_app_id: game.steam_app_id,
        platform: game.platform.clone(),
        source: game.source.clone(),
        title_id: game.title_id.clone(),
    };
    let external_game_id = match retry(|| async {
        if let Some(app_id) = identity.steam_app_id {
            client.resolve_steam_game(app_id).await
        } else {
            client.resolve_title(&identity.title).await
        }
    })
    .await
    {
        Ok(value) => value,
        Err(SteamGridDbError::GameAmbiguous) => {
            outcome.ambiguous = true;
            state.log(
                "artwork-enrichment",
                "artwork_ambiguous",
                &format!(
                    "game_id={} provider={} platform={}",
                    game.id, game.provider, game.platform
                ),
            );
            return outcome;
        }
        Err(_) => {
            outcome.errors = outcome.errors.saturating_add(1);
            return outcome;
        }
    };

    for slot in missing {
        if state
            .artwork_enrichment_cancel_requested
            .load(Ordering::SeqCst)
        {
            break;
        }
        set_current(
            state,
            Some(game.title.clone()),
            Some(slot_name(slot).to_string()),
        );
        if settings::artwork_negative_cache_valid(state, &game.id, slot).unwrap_or(false) {
            state.log(
                "artwork-enrichment",
                "artwork_negative_cache_hit",
                &format!("game_id={} slot={}", game.id, slot_name(slot)),
            );
            outcome.had_no_result = true;
            continue;
        }
        let assets = match retry(|| async {
            client
                .get_assets_for_enrichment(external_game_id, slot)
                .await
        })
        .await
        {
            Ok(assets) => assets,
            Err(_) => {
                outcome.errors = outcome.errors.saturating_add(1);
                continue;
            }
        };
        let Some(candidate) = select_best_asset(slot, &assets) else {
            let _ = settings::store_artwork_negative_cache(
                state,
                &game.id,
                slot,
                NEGATIVE_CACHE_TTL_SECONDS,
            );
            outcome.had_no_result = true;
            continue;
        };
        let prepared = match artwork::prepare_remote_artwork(
            state,
            &game.id,
            slot,
            &candidate,
            Some(max_dimension),
        )
        .await
        {
            Ok(prepared) => prepared,
            Err(_) => {
                outcome.errors = outcome.errors.saturating_add(1);
                continue;
            }
        };
        match settings::persist_automatic_artwork(state, &prepared) {
            Ok(true) => {
                outcome.downloaded_assets = outcome.downloaded_assets.saturating_add(1);
                state.log(
                    "artwork-enrichment",
                    "artwork_download",
                    &format!(
                        "game_id={} slot={} provider_asset_id={} source_dimensions={}x{} cached_dimensions={}x{}",
                        game.id,
                        slot_name(slot),
                        candidate.external_asset_id,
                        prepared.width,
                        prepared.height,
                        prepared.cached_width,
                        prepared.cached_height
                    ),
                );
            }
            Ok(false) => {}
            Err(_) => outcome.errors = outcome.errors.saturating_add(1),
        }
    }
    outcome
}

async fn retry<T, F, Fut>(mut operation: F) -> Result<T, SteamGridDbError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, SteamGridDbError>>,
{
    let mut attempt = 0;
    loop {
        match operation().await {
            Err(error)
                if attempt < 2
                    && matches!(
                        error,
                        SteamGridDbError::RateLimited
                            | SteamGridDbError::Offline
                            | SteamGridDbError::Timeout
                    ) =>
            {
                tokio::time::sleep(Duration::from_millis(400 * 2_u64.pow(attempt))).await;
                attempt += 1;
            }
            result => return result,
        }
    }
}

fn normalized_slots(slots: &[ArtworkSlot]) -> Vec<ArtworkSlot> {
    let requested = if slots.is_empty() {
        vec![
            ArtworkSlot::GridHorizontal,
            ArtworkSlot::GridVertical,
            ArtworkSlot::GridSquare,
            ArtworkSlot::Hero,
            ArtworkSlot::Logo,
        ]
    } else {
        slots.to_vec()
    };
    let mut seen = HashSet::new();
    requested
        .into_iter()
        .filter(|slot| *slot != ArtworkSlot::Icon && seen.insert(*slot))
        .collect()
}

fn set_status(state: &DatabaseState, status: ArtworkEnrichmentStatus) {
    if let Ok(mut current) = state.artwork_enrichment_status.lock() {
        *current = status;
    }
}

fn get_status(state: &DatabaseState) -> ArtworkEnrichmentStatus {
    state
        .artwork_enrichment_status
        .lock()
        .map(|status| status.clone())
        .unwrap_or_default()
}

fn set_current(state: &DatabaseState, game: Option<String>, artwork: Option<String>) {
    if let Ok(mut status) = state.artwork_enrichment_status.lock() {
        status.current_game = game;
        status.current_artwork = artwork;
    }
}

fn record_outcome(state: &DatabaseState, outcome: &GameOutcome) {
    let mut status = get_status(state);
    status.processed_games = status.processed_games.saturating_add(1);
    status.downloaded_assets = status
        .downloaded_assets
        .saturating_add(outcome.downloaded_assets);
    status.error_count = status.error_count.saturating_add(outcome.errors);
    if !outcome.had_missing_slot {
        status.already_complete_games = status.already_complete_games.saturating_add(1);
    }
    if outcome.had_no_result {
        status.no_result_games = status.no_result_games.saturating_add(1);
    }
    if outcome.ambiguous {
        status.ambiguous_games = status.ambiguous_games.saturating_add(1);
    }
    status.current_game = None;
    status.current_artwork = None;
    set_status(state, status);
}

fn slot_name(slot: ArtworkSlot) -> &'static str {
    match slot {
        ArtworkSlot::GridHorizontal => "grid_horizontal",
        ArtworkSlot::GridVertical => "grid_vertical",
        ArtworkSlot::GridSquare => "grid_square",
        ArtworkSlot::Hero => "hero",
        ArtworkSlot::Logo => "logo",
        ArtworkSlot::Icon => "icon",
    }
}

fn default_max_dimension() -> u32 {
    DEFAULT_MAX_DIMENSION
}

fn default_concurrency() -> usize {
    DEFAULT_CONCURRENCY
}

fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}
