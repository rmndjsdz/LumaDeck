use crate::settings::DatabaseState;
use futures_util::lock::Mutex as AsyncMutex;
use reqwest::{header, redirect::Policy, Client, StatusCode, Url};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex, OnceLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

pub const PCGAMINGWIKI_PROVIDER_VERSION: i64 = 1;
pub const PCGAMINGWIKI_SOURCE: &str = "PCGAMINGWIKI";
const PCGAMINGWIKI_HOST: &str = "www.pcgamingwiki.com";
const PCGAMINGWIKI_HOST_NO_WWW: &str = "pcgamingwiki.com";
const MEDIAWIKI_ENDPOINT: &str = "https://www.pcgamingwiki.com/w/api.php";
const CACHE_TTL_SECONDS: i64 = 7 * 24 * 60 * 60;
const IDENTITY_TTL_SECONDS: i64 = 30 * 24 * 60 * 60;
const MAX_REDIRECTS: usize = 5;
const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;
const MAX_RETRIES: usize = 2;
const MEDIAWIKI_ACCEPT: &str = "application/json";
const CARGO_PAGE_SIZE: usize = 500;
const MAX_GOG_CARGO_SCAN_ROWS: usize = 5_000;

type InflightSlot = Arc<AsyncMutex<Option<Result<PcgamingwikiCapabilitiesResponse, String>>>>;

fn inflight_requests() -> &'static Mutex<HashMap<String, InflightSlot>> {
    static REQUESTS: OnceLock<Mutex<HashMap<String, InflightSlot>>> = OnceLock::new();
    REQUESTS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PcgamingwikiResolutionStatus {
    Resolved,
    NotFound,
    #[serde(rename = "PCGW_FORBIDDEN")]
    Forbidden,
    IdentityUnavailable,
    RateLimited,
    NetworkError,
    Timeout,
    TemporaryFailure,
    InvalidRedirect,
    ParseFailure,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PcgamingwikiResolvedVia {
    SteamAppId,
    GogProductId,
    MediaWikiSteamId,
    MediaWikiGogId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PcgamingwikiCapability {
    NativeHdr,
    HighFidelityUpscaling,
    FrameGeneration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum PcgamingwikiNormalizedValue {
    Yes,
    No,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum PcgamingwikiConfidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcgamingwikiGameRef {
    pub page_title: String,
    pub page_id: Option<String>,
    pub canonical_url: String,
    pub steam_app_id: Option<i64>,
    pub gog_product_id: Option<String>,
    pub resolved_via: PcgamingwikiResolvedVia,
    pub resolved_at: String,
    pub redirect_chain: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcgamingwikiCapabilityEvidence {
    pub capability: PcgamingwikiCapability,
    pub normalized_value: PcgamingwikiNormalizedValue,
    pub source_value: Option<String>,
    pub alternative_available: PcgamingwikiNormalizedValue,
    pub source_note: Option<String>,
    pub technologies: Vec<String>,
    pub source: String,
    pub source_page: String,
    pub source_field: String,
    pub confidence: PcgamingwikiConfidence,
    pub observed_at: String,
    pub provider_version: i64,
    pub stale: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcgamingwikiCapabilities {
    pub native_hdr: PcgamingwikiCapabilityEvidence,
    pub high_fidelity_upscaling: PcgamingwikiCapabilityEvidence,
    pub frame_generation: PcgamingwikiCapabilityEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcgamingwikiIdentityConflict {
    pub steam: PcgamingwikiGameRef,
    pub gog: PcgamingwikiGameRef,
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcgamingwikiCapabilitiesResponse {
    pub status: PcgamingwikiResolutionStatus,
    pub game_ref: Option<PcgamingwikiGameRef>,
    pub capabilities: Option<PcgamingwikiCapabilities>,
    pub source: String,
    pub provider_version: i64,
    pub stale: bool,
    pub conflict: Option<PcgamingwikiIdentityConflict>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PcgamingwikiCapabilitiesRequest {
    pub game_id: String,
    pub steam_app_id: Option<i64>,
    pub gog_product_id: Option<String>,
    #[serde(default)]
    pub force_refresh: bool,
    #[serde(default)]
    pub cross_check_identities: bool,
}

#[derive(Debug, Error, Clone)]
enum ProviderError {
    #[error("identity unavailable")]
    IdentityUnavailable,
    #[error("not found")]
    NotFound,
    #[error("PCGW_FORBIDDEN")]
    Forbidden,
    #[error("rate limited")]
    RateLimited,
    #[error("network error")]
    Network,
    #[error("request timed out")]
    Timeout,
    #[error("temporary HTTP failure: {0}")]
    Temporary(u16),
    #[error("invalid PCGamingWiki redirect")]
    InvalidRedirect,
    #[error("page data could not be parsed")]
    Parse,
    #[error("Cargo query unavailable: {0}")]
    CargoUnavailable(String),
    #[error("MediaWiki API error: {0}")]
    MediaWikiApi(String),
    #[error("request setup failed")]
    RequestSetup,
}

impl ProviderError {
    fn status(&self) -> PcgamingwikiResolutionStatus {
        match self {
            Self::IdentityUnavailable => PcgamingwikiResolutionStatus::IdentityUnavailable,
            Self::NotFound => PcgamingwikiResolutionStatus::NotFound,
            Self::Forbidden => PcgamingwikiResolutionStatus::Forbidden,
            Self::RateLimited => PcgamingwikiResolutionStatus::RateLimited,
            Self::Network => PcgamingwikiResolutionStatus::NetworkError,
            Self::Timeout => PcgamingwikiResolutionStatus::Timeout,
            Self::Temporary(_) => PcgamingwikiResolutionStatus::TemporaryFailure,
            Self::InvalidRedirect => PcgamingwikiResolutionStatus::InvalidRedirect,
            Self::Parse => PcgamingwikiResolutionStatus::ParseFailure,
            Self::CargoUnavailable(_) => PcgamingwikiResolutionStatus::TemporaryFailure,
            Self::MediaWikiApi(_) => PcgamingwikiResolutionStatus::TemporaryFailure,
            Self::RequestSetup => PcgamingwikiResolutionStatus::TemporaryFailure,
        }
    }
}

#[derive(Debug, Clone)]
struct HttpResponse {
    status: StatusCode,
    body: Vec<u8>,
    etag: Option<String>,
    last_modified: Option<String>,
}

#[derive(Debug, Clone)]
struct IdentityResolution {
    game_ref: PcgamingwikiGameRef,
    identity_checked_at: i64,
}

#[derive(Debug, Clone, Default)]
struct VideoFields {
    hdr: Option<String>,
    hdr_notes: Option<String>,
    upscaling: Option<String>,
    upscaling_technologies: Option<String>,
    upscaling_notes: Option<String>,
    frame_generation: Option<String>,
    frame_generation_technologies: Option<String>,
    frame_generation_notes: Option<String>,
}

#[derive(Debug, Clone)]
struct CachedResult {
    game_ref: PcgamingwikiGameRef,
    capabilities: PcgamingwikiCapabilities,
    checked_at: i64,
    identity_checked_at: i64,
    etag: Option<String>,
    last_modified: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MediaWikiParseResponse {
    parse: Option<MediaWikiParse>,
    error: Option<MediaWikiApiError>,
}

#[derive(Debug, Clone, Deserialize)]
struct CargoIdentityResponse {
    cargoquery: Option<Vec<CargoIdentityEnvelope>>,
    error: Option<MediaWikiApiError>,
}

#[derive(Debug, Clone, Deserialize)]
struct CargoIdentityEnvelope {
    title: CargoIdentityRow,
}

#[derive(Debug, Clone, Deserialize)]
struct CargoIdentityRow {
    #[serde(rename = "Page")]
    page: Option<String>,
    #[serde(rename = "PageID")]
    page_id: Option<String>,
    #[serde(rename = "Steam AppID")]
    steam_app_id: Option<String>,
    #[serde(rename = "GOGcom ID")]
    gog_product_id: Option<String>,
}

impl CargoIdentityRow {
    fn matches_identity(&self, identity: &ExternalIdentity) -> bool {
        let value = match identity {
            ExternalIdentity::Steam(_) => self.steam_app_id.as_deref(),
            ExternalIdentity::Gog(_) => self.gog_product_id.as_deref(),
        };
        value
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .any(|candidate| candidate == identity.value())
    }
}

#[derive(Debug, Clone, Deserialize)]
struct MediaWikiApiError {
    code: Option<String>,
    info: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MediaWikiParse {
    pageid: Option<i64>,
    title: Option<String>,
    wikitext: Option<MediaWikiWikitext>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum MediaWikiWikitext {
    Text(String),
    Legacy {
        #[serde(rename = "*")]
        text: Option<String>,
    },
}

impl MediaWikiWikitext {
    fn text(self) -> Option<String> {
        match self {
            Self::Text(value) => Some(value),
            Self::Legacy { text } => text,
        }
    }
}

#[derive(Debug, Clone)]
struct NormalizedField {
    value: PcgamingwikiNormalizedValue,
    source_value: Option<String>,
    alternative_available: PcgamingwikiNormalizedValue,
    source_note: Option<String>,
    technologies: Vec<String>,
    confidence: PcgamingwikiConfidence,
}

#[tauri::command]
pub async fn get_pcgamingwiki_capabilities(
    state: tauri::State<'_, DatabaseState>,
    game_id: String,
    steam_app_id: Option<i64>,
    gog_product_id: Option<String>,
    force_refresh: Option<bool>,
    cross_check_identities: Option<bool>,
) -> Result<PcgamingwikiCapabilitiesResponse, String> {
    let request = PcgamingwikiCapabilitiesRequest {
        game_id,
        steam_app_id,
        gog_product_id,
        force_refresh: force_refresh.unwrap_or(false),
        cross_check_identities: cross_check_identities.unwrap_or(false),
    };
    get_capabilities(&state, request).await
}

pub async fn get_capabilities(
    state: &DatabaseState,
    request: PcgamingwikiCapabilitiesRequest,
) -> Result<PcgamingwikiCapabilitiesResponse, String> {
    if request.game_id.trim().is_empty() {
        return Err("PCGW_INVALID_GAME_ID".to_string());
    }
    let steam_app_id = valid_steam_id(request.steam_app_id);
    let gog_product_id = valid_gog_id(request.gog_product_id);
    let key = format!(
        "{}:{}:{}",
        request.game_id,
        steam_app_id
            .map(|value| value.to_string())
            .unwrap_or_default(),
        gog_product_id.clone().unwrap_or_default()
    );

    let slot = {
        let mut requests = inflight_requests()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Arc::clone(
            requests
                .entry(key.clone())
                .or_insert_with(|| Arc::new(AsyncMutex::new(None))),
        )
    };
    let result = {
        let mut shared = slot.lock().await;
        if let Some(result) = shared.clone() {
            result
        } else {
            let result = execute_pipeline(
                state,
                &request.game_id,
                steam_app_id,
                gog_product_id,
                request.force_refresh,
                request.cross_check_identities,
            )
            .await;
            *shared = Some(result.clone());
            result
        }
    };
    inflight_requests()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .remove(&key);
    result
}

async fn execute_pipeline(
    state: &DatabaseState,
    game_id: &str,
    steam_app_id: Option<i64>,
    gog_product_id: Option<String>,
    force_refresh: bool,
    cross_check_identities: bool,
) -> Result<PcgamingwikiCapabilitiesResponse, String> {
    state.log(
        "pcgamingwiki",
        "pcgw.resolve.start",
        &format!(
            "game_id={game_id} steam={} gog={}",
            steam_app_id.is_some(),
            gog_product_id.is_some()
        ),
    );
    if steam_app_id.is_none() && gog_product_id.is_none() {
        state.log("pcgamingwiki", "pcgw.error", "status=IDENTITY_UNAVAILABLE");
        return Ok(empty_response(
            PcgamingwikiResolutionStatus::IdentityUnavailable,
            None,
        ));
    }

    let cached = load_cached_result(state, game_id)
        .map_err(|error| error.to_string())?
        .filter(|value| same_external_ids(value, steam_app_id, gog_product_id.as_deref()));
    if let Some(cached_result) = cached.clone() {
        if !force_refresh && cached_result.checked_at + CACHE_TTL_SECONDS > now_seconds() {
            state.log("pcgamingwiki", "pcgw.fetch.cache_hit", "fresh=true");
            return Ok(response_from_cache(cached_result, false, None));
        }
    }

    let client = build_client().map_err(|error| error.to_string())?;
    let primary = if let Some(cached_result) = cached
        .clone()
        .filter(|value| value.identity_checked_at + IDENTITY_TTL_SECONDS > now_seconds())
    {
        state.log(
            "pcgamingwiki",
            "pcgw.resolve.cache_hit",
            "identity_fresh=true",
        );
        Ok(IdentityResolution {
            game_ref: cached_result.game_ref,
            identity_checked_at: cached_result.identity_checked_at,
        })
    } else if let Some(app_id) = steam_app_id {
        state.log(
            "pcgamingwiki",
            "pcgw.resolve.steam",
            &format!("app_id={app_id}"),
        );
        resolve_by_steam(state, &client, app_id).await
    } else {
        let product_id = gog_product_id
            .as_deref()
            .ok_or_else(|| ProviderError::IdentityUnavailable.to_string())?;
        state.log(
            "pcgamingwiki",
            "pcgw.resolve.gog",
            "product_id_present=true",
        );
        resolve_by_gog(state, &client, product_id).await
    };

    let mut primary = match primary {
        Ok(value) => value,
        Err(error) => {
            if let Some(cached_result) = cached {
                state.log(
                    "pcgamingwiki",
                    "pcgw.error",
                    &format!(
                        "status={} fallback=stale_cache",
                        status_name(error.status())
                    ),
                );
                return Ok(response_from_cache(cached_result, true, None));
            }
            state.log(
                "pcgamingwiki",
                "pcgw.error",
                &format!("status={}", status_name(error.status())),
            );
            return Ok(empty_response(error.status(), Some(error.to_string())));
        }
    };
    let identity_checked_at = primary.identity_checked_at;
    primary.game_ref.steam_app_id = steam_app_id;
    primary.game_ref.gog_product_id = gog_product_id.clone();

    let conflict = if cross_check_identities && steam_app_id.is_some() && gog_product_id.is_some() {
        let gog = resolve_by_gog(
            state,
            &client,
            gog_product_id.as_deref().unwrap_or_default(),
        )
        .await;
        match gog {
            Ok(gog_resolution) if !same_identity(&primary.game_ref, &gog_resolution.game_ref) => {
                state.log(
                    "pcgamingwiki",
                    "pcgw.resolve.conflict",
                    "code=PCGW_IDENTITY_CONFLICT",
                );
                Some(PcgamingwikiIdentityConflict {
                    steam: primary.game_ref.clone(),
                    gog: gog_resolution.game_ref,
                    code: "PCGW_IDENTITY_CONFLICT".to_string(),
                })
            }
            Ok(_) => None,
            Err(error) => {
                state.log(
                    "pcgamingwiki",
                    "pcgw.error",
                    &format!("cross_check_status={}", status_name(error.status())),
                );
                None
            }
        }
    } else {
        None
    };

    state.log(
        "pcgamingwiki",
        "pcgw.resolve.success",
        &format!("page={}", primary.game_ref.page_title),
    );
    let technical = fetch_technical_data(
        state,
        &client,
        &primary.game_ref,
        cached.as_ref().and_then(|value| value.etag.as_deref()),
        cached
            .as_ref()
            .and_then(|value| value.last_modified.as_deref()),
    )
    .await;

    let (capabilities, etag, last_modified, page_identifier, not_modified) = match technical {
        Ok(TechnicalDataResult::NotModified) => {
            let Some(cached_result) = cached else {
                return Ok(empty_response(
                    PcgamingwikiResolutionStatus::ParseFailure,
                    Some("304 without cache".to_string()),
                ));
            };
            (
                cached_result.capabilities,
                cached_result.etag,
                cached_result.last_modified,
                cached_result.game_ref.page_id,
                true,
            )
        }
        Ok(TechnicalDataResult::Fetched {
            capabilities,
            etag,
            last_modified,
            page_identifier,
        }) => (capabilities, etag, last_modified, page_identifier, false),
        Err(error) => {
            if let Some(cached_result) = cached {
                state.log(
                    "pcgamingwiki",
                    "pcgw.error",
                    &format!(
                        "fetch_status={} fallback=stale_cache",
                        status_name(error.status())
                    ),
                );
                return Ok(response_from_cache(cached_result, true, conflict));
            }
            return Ok(empty_response(error.status(), Some(error.to_string())));
        }
    };
    if let Some(page_id) = page_identifier {
        primary.game_ref.page_id = Some(page_id);
    }

    if not_modified {
        state.log("pcgamingwiki", "pcgw.fetch.not_modified", "status=304");
    } else {
        state.log(
            "pcgamingwiki",
            "pcgw.fetch.start",
            "source=mediawiki_wikitext",
        );
    }
    let response = PcgamingwikiCapabilitiesResponse {
        status: PcgamingwikiResolutionStatus::Resolved,
        game_ref: Some(primary.game_ref.clone()),
        capabilities: Some(capabilities.clone()),
        source: PCGAMINGWIKI_SOURCE.to_string(),
        provider_version: PCGAMINGWIKI_PROVIDER_VERSION,
        stale: false,
        conflict,
        error: None,
    };
    persist_result(
        state,
        game_id,
        &primary.game_ref,
        &capabilities,
        identity_checked_at,
        etag,
        last_modified,
    )
    .map_err(|error| error.to_string())?;
    Ok(response)
}

fn build_client() -> Result<Client, ProviderError> {
    Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(20))
        .redirect(Policy::none())
        .user_agent(provider_user_agent())
        .build()
        .map_err(|_| ProviderError::RequestSetup)
}

fn provider_user_agent() -> String {
    format!(
        "LumaDeck/{} PCGamingWikiProvider/{} (https://github.com/rmndjsdz/LumaDeck)",
        env!("CARGO_PKG_VERSION"),
        PCGAMINGWIKI_PROVIDER_VERSION
    )
}

async fn resolve_by_steam(
    state: &DatabaseState,
    client: &Client,
    app_id: i64,
) -> Result<IdentityResolution, ProviderError> {
    resolve_by_external_id(state, client, ExternalIdentity::Steam(app_id.to_string())).await
}

async fn resolve_by_gog(
    state: &DatabaseState,
    client: &Client,
    product_id: &str,
) -> Result<IdentityResolution, ProviderError> {
    resolve_by_external_id(state, client, ExternalIdentity::Gog(product_id.to_string())).await
}

#[derive(Debug, Clone)]
enum ExternalIdentity {
    Steam(String),
    Gog(String),
}

impl ExternalIdentity {
    fn field_name(&self) -> &'static str {
        match self {
            Self::Steam(_) => "Steam_AppID",
            Self::Gog(_) => "GOGcom_ID",
        }
    }

    fn value(&self) -> &str {
        match self {
            Self::Steam(value) | Self::Gog(value) => value,
        }
    }

    fn resolved_via(&self) -> PcgamingwikiResolvedVia {
        match self {
            Self::Steam(_) => PcgamingwikiResolvedVia::MediaWikiSteamId,
            Self::Gog(_) => PcgamingwikiResolvedVia::MediaWikiGogId,
        }
    }
}

async fn resolve_by_external_id(
    state: &DatabaseState,
    client: &Client,
    identity: ExternalIdentity,
) -> Result<IdentityResolution, ProviderError> {
    let rows = cargo_identity_query(state, client, &identity, None).await?;
    if let Some(row) = rows.into_iter().find(|row| row.matches_identity(&identity)) {
        return identity_from_cargo_row(row, &identity);
    }

    if matches!(identity, ExternalIdentity::Gog(_)) {
        let mut offset = 0usize;
        while offset < MAX_GOG_CARGO_SCAN_ROWS {
            let rows = cargo_identity_query(state, client, &identity, Some(offset)).await?;
            if rows.is_empty() {
                return Err(ProviderError::NotFound);
            }
            if let Some(row) = rows.iter().find(|row| row.matches_identity(&identity)) {
                return identity_from_cargo_row(row.clone(), &identity);
            }
            if rows.len() < CARGO_PAGE_SIZE {
                return Err(ProviderError::NotFound);
            }
            offset += CARGO_PAGE_SIZE;
        }
        return Err(ProviderError::CargoUnavailable(
            "GOG Cargo scan limit reached".to_string(),
        ));
    }

    Err(ProviderError::NotFound)
}

async fn cargo_identity_query(
    state: &DatabaseState,
    client: &Client,
    identity: &ExternalIdentity,
    offset: Option<usize>,
) -> Result<Vec<CargoIdentityRow>, ProviderError> {
    let mut url = Url::parse(MEDIAWIKI_ENDPOINT).map_err(|_| ProviderError::RequestSetup)?;
    url.query_pairs_mut()
        .append_pair("action", "cargoquery")
        .append_pair("tables", "Infobox_game")
        .append_pair(
            "fields",
            "Infobox_game._pageName=Page,Infobox_game._pageID=PageID,Infobox_game.Steam_AppID,Infobox_game.GOGcom_ID",
        )
        .append_pair("format", "json")
        .append_pair(
            "limit",
            &offset
                .map(|_| CARGO_PAGE_SIZE)
                .unwrap_or(1)
                .to_string(),
        );
    if let Some(offset) = offset {
        url.query_pairs_mut()
            .append_pair("offset", &offset.to_string());
        url.query_pairs_mut()
            .append_pair("where", "Infobox_game.GOGcom_ID HOLDS LIKE \"%\"");
    } else {
        url.query_pairs_mut().append_pair(
            "where",
            &format!(
                "Infobox_game.{} HOLDS \"{}\"",
                identity.field_name(),
                identity.value()
            ),
        );
    }
    let response = request(state, client, url, MEDIAWIKI_ACCEPT, None, None).await?;
    validate_status(response.status)?;
    parse_cargo_identity_response(&response.body)
}

fn parse_cargo_identity_response(body: &[u8]) -> Result<Vec<CargoIdentityRow>, ProviderError> {
    let parsed: CargoIdentityResponse =
        serde_json::from_slice(body).map_err(|_| ProviderError::Parse)?;
    if let Some(error) = parsed.error {
        return Err(ProviderError::CargoUnavailable(format_mediawiki_error(
            &error,
        )));
    }
    Ok(parsed
        .cargoquery
        .unwrap_or_default()
        .into_iter()
        .map(|row| row.title)
        .collect())
}

fn format_mediawiki_error(error: &MediaWikiApiError) -> String {
    match (&error.code, &error.info) {
        (Some(code), Some(info)) => format!("{code}: {info}"),
        (Some(code), None) => code.clone(),
        (None, Some(info)) => info.clone(),
        (None, None) => "unknown MediaWiki error".to_string(),
    }
}

fn identity_from_cargo_row(
    row: CargoIdentityRow,
    identity: &ExternalIdentity,
) -> Result<IdentityResolution, ProviderError> {
    let page_title = row.page.ok_or(ProviderError::Parse)?;
    let page_id = row.page_id.ok_or(ProviderError::Parse)?;
    Ok(IdentityResolution {
        game_ref: PcgamingwikiGameRef {
            page_title: page_title.clone(),
            page_id: Some(page_id),
            canonical_url: canonical_page_url_from_title(&page_title),
            steam_app_id: match identity {
                ExternalIdentity::Steam(value) => value.parse().ok(),
                ExternalIdentity::Gog(_) => None,
            },
            gog_product_id: match identity {
                ExternalIdentity::Steam(_) => None,
                ExternalIdentity::Gog(value) => Some(value.clone()),
            },
            resolved_via: identity.resolved_via(),
            resolved_at: now_string(),
            redirect_chain: Vec::new(),
        },
        identity_checked_at: now_seconds(),
    })
}

async fn fetch_technical_data(
    state: &DatabaseState,
    client: &Client,
    game_ref: &PcgamingwikiGameRef,
    etag: Option<&str>,
    last_modified: Option<&str>,
) -> Result<TechnicalDataResult, ProviderError> {
    let mut url = Url::parse(MEDIAWIKI_ENDPOINT).map_err(|_| ProviderError::RequestSetup)?;
    url.query_pairs_mut()
        .append_pair("action", "parse")
        .append_pair("page", &game_ref.page_title)
        .append_pair("redirects", "1")
        .append_pair("prop", "wikitext")
        .append_pair("format", "json")
        .append_pair("formatversion", "2");
    let response = request(state, client, url, MEDIAWIKI_ACCEPT, etag, last_modified).await?;
    if response.status == StatusCode::NOT_MODIFIED {
        return Ok(TechnicalDataResult::NotModified);
    }
    validate_status(response.status)?;
    let parsed: MediaWikiParseResponse =
        serde_json::from_slice(&response.body).map_err(|_| ProviderError::Parse)?;
    if let Some(error) = parsed.error {
        return Err(ProviderError::MediaWikiApi(format_mediawiki_error(&error)));
    }
    let page = parsed.parse.ok_or(ProviderError::Parse)?;
    let wikitext = page
        .wikitext
        .and_then(MediaWikiWikitext::text)
        .ok_or(ProviderError::Parse)?;
    let fields = parse_video_fields(&wikitext)?;
    let source_page = page.title.unwrap_or_else(|| game_ref.page_title.clone());
    let capabilities = normalize_capabilities(&fields, &source_page);
    Ok(TechnicalDataResult::Fetched {
        capabilities,
        etag: response.etag,
        last_modified: response.last_modified,
        page_identifier: page.pageid.map(|value| value.to_string()),
    })
}

enum TechnicalDataResult {
    NotModified,
    Fetched {
        capabilities: PcgamingwikiCapabilities,
        etag: Option<String>,
        last_modified: Option<String>,
        page_identifier: Option<String>,
    },
}

async fn request(
    state: &DatabaseState,
    client: &Client,
    mut url: Url,
    accept: &str,
    etag: Option<&str>,
    last_modified: Option<&str>,
) -> Result<HttpResponse, ProviderError> {
    validate_host(&url)?;
    let mut redirect_chain = Vec::new();
    for attempt in 0..=MAX_RETRIES {
        let mut request = client.get(url.clone());
        request = request.header(header::ACCEPT, accept);
        if let Some(value) = etag {
            request = request.header(header::IF_NONE_MATCH, value);
        }
        if let Some(value) = last_modified {
            request = request.header(header::IF_MODIFIED_SINCE, value);
        }
        let response = match request.send().await {
            Ok(response) => response,
            Err(error) if error.is_timeout() => return Err(ProviderError::Timeout),
            Err(_) => return Err(ProviderError::Network),
        };
        let status = response.status();
        let response_url = response.url().clone();
        let headers = response.headers().clone();
        let etag_value = response
            .headers()
            .get(header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        let last_modified_value = response
            .headers()
            .get(header::LAST_MODIFIED)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        let location = headers
            .get(header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);
        let content_length = response.content_length().unwrap_or(0) as usize;
        if content_length > MAX_RESPONSE_BYTES {
            return Err(ProviderError::Parse);
        }
        let body = response.bytes().await.map_err(|_| ProviderError::Network)?;
        if body.len() > MAX_RESPONSE_BYTES {
            return Err(ProviderError::Parse);
        }
        log_http_response(
            state,
            &url,
            accept,
            &response_url,
            status,
            &headers,
            &body,
            &redirect_chain,
            location.as_deref(),
        );
        if status.is_redirection() {
            let location = location.ok_or(ProviderError::InvalidRedirect)?;
            let next = url
                .join(&location)
                .map_err(|_| ProviderError::InvalidRedirect)?;
            validate_host(&next)?;
            redirect_chain.push(next.to_string());
            if redirect_chain.len() > MAX_REDIRECTS {
                return Err(ProviderError::InvalidRedirect);
            }
            url = next;
            continue;
        }
        if (status == StatusCode::REQUEST_TIMEOUT || status.is_server_error())
            && attempt < MAX_RETRIES
        {
            continue;
        }
        return Ok(HttpResponse {
            status,
            body: body.to_vec(),
            etag: etag_value,
            last_modified: last_modified_value,
        });
    }
    Err(ProviderError::Temporary(503))
}

fn log_http_response(
    state: &DatabaseState,
    endpoint: &Url,
    accept: &str,
    response_url: &Url,
    status: StatusCode,
    headers: &header::HeaderMap,
    body: &[u8],
    redirect_chain: &[String],
    location: Option<&str>,
) {
    let content_type = header_value(headers, header::CONTENT_TYPE);
    let server = header_value(headers, header::SERVER);
    let retry_after = header_value(headers, header::RETRY_AFTER);
    let chain = if redirect_chain.is_empty() {
        "<none>".to_string()
    } else {
        redirect_chain.join(" -> ")
    };
    state.log(
        "pcgamingwiki",
        "pcgw.http",
        &format!(
            "endpoint={endpoint}; method=GET; user_agent={}; accept={accept}; redirect_enabled=false; status={}; final_url={response_url}; redirect_chain={chain}; content_type={content_type}; server={server}; retry_after={retry_after}; location={}; body_size={}; body_preview={}",
            provider_user_agent(),
            status.as_u16(),
            location.unwrap_or("<none>"),
            body.len(),
            sanitize_body_preview(body),
        ),
    );
}

fn header_value(headers: &header::HeaderMap, name: header::HeaderName) -> String {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("<none>")
        .to_string()
}

fn sanitize_body_preview(body: &[u8]) -> String {
    let text = String::from_utf8_lossy(body);
    let lower = text.to_ascii_lowercase();
    let sensitive_markers = [
        "authorization",
        "cookie",
        "password",
        "api_key",
        "apikey",
        "set-cookie",
        "token",
    ];
    if sensitive_markers
        .iter()
        .any(|marker| lower.contains(marker))
    {
        return "<redacted>".to_string();
    }
    text.chars()
        .take(300)
        .map(|character| match character {
            '\r' | '\n' | '\t' => ' ',
            _ => character,
        })
        .collect()
}

fn validate_status(status: StatusCode) -> Result<(), ProviderError> {
    if status == StatusCode::NOT_FOUND {
        return Err(ProviderError::NotFound);
    }
    if status == StatusCode::FORBIDDEN {
        return Err(ProviderError::Forbidden);
    }
    if status == StatusCode::TOO_MANY_REQUESTS {
        return Err(ProviderError::RateLimited);
    }
    if status.is_server_error() || status == StatusCode::REQUEST_TIMEOUT {
        return Err(ProviderError::Temporary(status.as_u16()));
    }
    if !status.is_success() {
        return Err(ProviderError::Temporary(status.as_u16()));
    }
    Ok(())
}

fn validate_host(url: &Url) -> Result<(), ProviderError> {
    match url.host_str() {
        Some(host)
            if host.eq_ignore_ascii_case(PCGAMINGWIKI_HOST)
                || host.eq_ignore_ascii_case(PCGAMINGWIKI_HOST_NO_WWW) =>
        {
            Ok(())
        }
        _ => Err(ProviderError::InvalidRedirect),
    }
}

fn valid_steam_id(value: Option<i64>) -> Option<i64> {
    value.filter(|id| *id > 0)
}

fn valid_gog_id(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()
            && value.len() <= 64
            && value.chars().all(|character| character.is_ascii_digit()))
        .then_some(value)
    })
}

fn canonical_page_url_from_title(title: &str) -> String {
    let mut url = Url::parse(&format!("https://{PCGAMINGWIKI_HOST}/"))
        .expect("PCGamingWiki base URL is valid");
    url.path_segments_mut()
        .expect("PCGamingWiki URL supports path segments")
        .push("wiki")
        .push(title);
    url.to_string()
}

#[cfg(test)]
fn title_from_url(url: &Url) -> Option<String> {
    let path = url.path().trim_matches('/');
    let (_, title) = path.split_once("wiki/")?;
    let title = percent_decode(title).replace('_', " ");
    (!title.is_empty()).then_some(title)
}

#[cfg(test)]
fn percent_decode(value: &str) -> String {
    let mut bytes = Vec::with_capacity(value.len());
    let mut chars = value.as_bytes().iter().copied();
    while let Some(value) = chars.next() {
        if value == b'%' {
            let high = chars.next().and_then(hex_value);
            let low = chars.next().and_then(hex_value);
            if let (Some(high), Some(low)) = (high, low) {
                bytes.push(high * 16 + low);
                continue;
            }
        }
        bytes.push(value);
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

#[cfg(test)]
fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn same_identity(left: &PcgamingwikiGameRef, right: &PcgamingwikiGameRef) -> bool {
    match (&left.page_id, &right.page_id) {
        (Some(left), Some(right)) => left == right,
        _ => left.page_title.eq_ignore_ascii_case(&right.page_title),
    }
}

fn parse_video_fields(source: &str) -> Result<VideoFields, ProviderError> {
    let lower = source.to_ascii_lowercase();
    let Some(start) = lower.find("{{video") else {
        return Ok(VideoFields::default());
    };
    let end = matching_template_end(source, start).ok_or(ProviderError::Parse)?;
    let template = &source[start..end.saturating_sub(2)];
    let mut values = HashMap::new();
    for part in split_top_level(template, b'|') {
        let Some((key, value)) = split_assignment(&part) else {
            continue;
        };
        values.insert(
            key.trim().to_ascii_lowercase(),
            clean_wikitext(value.trim()),
        );
    }
    Ok(VideoFields {
        hdr: values.get("hdr").cloned(),
        hdr_notes: first_video_value(&values, &["hdr notes", "hdr note"]),
        upscaling: values.get("upscaling").cloned(),
        upscaling_technologies: values
            .get("upscaling tech")
            .or_else(|| values.get("upscaling technologies"))
            .cloned(),
        upscaling_notes: first_video_value(&values, &["upscaling notes", "upscaling note"]),
        frame_generation: values
            .get("framegen")
            .or_else(|| values.get("frame generation"))
            .cloned(),
        frame_generation_technologies: values
            .get("framegen tech")
            .or_else(|| values.get("frame generation tech"))
            .cloned(),
        frame_generation_notes: first_video_value(
            &values,
            &[
                "framegen notes",
                "framegen note",
                "frame generation notes",
                "frame generation note",
            ],
        ),
    })
}

fn first_video_value(values: &HashMap<String, String>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| values.get(*key).cloned())
}

fn matching_template_end(source: &str, start: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut index = start;
    while index + 1 < bytes.len() {
        if bytes[index] == b'{' && bytes[index + 1] == b'{' {
            depth += 1;
            index += 2;
            continue;
        }
        if bytes[index] == b'}' && bytes[index + 1] == b'}' {
            depth = depth.checked_sub(1)?;
            index += 2;
            if depth == 0 {
                return Some(index);
            }
            continue;
        }
        index += 1;
    }
    None
}

fn split_top_level(source: &str, separator: u8) -> Vec<String> {
    let bytes = source.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    let mut index = 0;
    while index < bytes.len() {
        if index + 1 < bytes.len() && bytes[index] == b'{' && bytes[index + 1] == b'{' {
            depth += 1;
            index += 2;
        } else if index + 1 < bytes.len() && bytes[index] == b'}' && bytes[index + 1] == b'}' {
            depth = depth.saturating_sub(1);
            index += 2;
        } else if bytes[index] == separator && depth == 1 {
            parts.push(source[start..index].to_string());
            index += 1;
            start = index;
        } else {
            index += 1;
        }
    }
    parts.push(source[start..].to_string());
    parts
}

fn split_assignment(source: &str) -> Option<(String, String)> {
    let bytes = source.as_bytes();
    let mut depth = 0usize;
    let mut index = 0;
    while index < bytes.len() {
        if index + 1 < bytes.len() && bytes[index] == b'{' && bytes[index + 1] == b'{' {
            depth += 1;
            index += 2;
        } else if index + 1 < bytes.len() && bytes[index] == b'}' && bytes[index + 1] == b'}' {
            depth = depth.saturating_sub(1);
            index += 2;
        } else if bytes[index] == b'=' && depth == 0 {
            return Some((source[..index].to_string(), source[index + 1..].to_string()));
        } else {
            index += 1;
        }
    }
    None
}

fn clean_wikitext(value: &str) -> String {
    let value = value
        .replace("<br />", ", ")
        .replace("<br/>", ", ")
        .replace("<br>", ", ");
    let mut result = String::with_capacity(value.len());
    let mut in_tag = false;
    for character in value.chars() {
        match character {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(character),
            _ => {}
        }
    }
    result.replace("{{!}}", "|").trim().to_string()
}

fn normalize_capabilities(fields: &VideoFields, source_page: &str) -> PcgamingwikiCapabilities {
    let hdr = normalize_hdr(fields);
    let upscaling_note = note_or_default(
        fields.upscaling.as_deref(),
        fields.upscaling_notes.as_deref(),
        "See the glossary page for potential workarounds.",
    );
    let frame_generation_note = note_or_default(
        fields.frame_generation.as_deref(),
        fields.frame_generation_notes.as_deref(),
        "See the glossary page for potential workarounds.",
    );
    let upscaling = normalize_technology_field(
        fields.upscaling.as_deref(),
        fields.upscaling_technologies.as_deref(),
        upscaling_note.as_deref(),
    );
    let frame_generation = normalize_technology_field(
        fields.frame_generation.as_deref(),
        fields.frame_generation_technologies.as_deref(),
        frame_generation_note.as_deref(),
    );
    PcgamingwikiCapabilities {
        native_hdr: evidence(
            PcgamingwikiCapability::NativeHdr,
            hdr,
            source_page,
            "High dynamic range display (HDR)",
        ),
        high_fidelity_upscaling: evidence(
            PcgamingwikiCapability::HighFidelityUpscaling,
            upscaling,
            source_page,
            "High-fidelity upscaling",
        ),
        frame_generation: evidence(
            PcgamingwikiCapability::FrameGeneration,
            frame_generation,
            source_page,
            "Frame generation",
        ),
    }
}

fn normalize_hdr(fields: &VideoFields) -> NormalizedField {
    let direct = fields.hdr.as_deref().unwrap_or_default();
    let source_note = note_or_default(
        Some(direct),
        fields.hdr_notes.as_deref(),
        "See the engine page to force native HDR output, or the glossary page for other alternatives.",
    );
    let source_value = match (&fields.hdr, &source_note) {
        (Some(value), Some(notes)) if !notes.is_empty() => Some(format!("{value} — {notes}")),
        (Some(value), _) => Some(value.clone()),
        (None, Some(notes)) if !notes.is_empty() => Some(notes.clone()),
        _ => None,
    };
    if explicit_yes(direct) {
        return NormalizedField {
            value: PcgamingwikiNormalizedValue::Yes,
            source_value,
            alternative_available: alternative_available(source_note.as_deref()),
            source_note: source_note.clone(),
            technologies: Vec::new(),
            confidence: PcgamingwikiConfidence::High,
        };
    }
    if explicit_no(direct) || (direct.trim().is_empty() && workaround_only(source_value.as_deref()))
    {
        return NormalizedField {
            value: PcgamingwikiNormalizedValue::No,
            source_value,
            alternative_available: alternative_available(source_note.as_deref()),
            source_note: source_note.clone(),
            technologies: Vec::new(),
            confidence: PcgamingwikiConfidence::High,
        };
    }
    NormalizedField {
        value: PcgamingwikiNormalizedValue::Unknown,
        source_value,
        alternative_available: alternative_available(source_note.as_deref()),
        source_note,
        technologies: Vec::new(),
        confidence: PcgamingwikiConfidence::Low,
    }
}

fn normalize_technology_field(
    value: Option<&str>,
    technologies: Option<&str>,
    note: Option<&str>,
) -> NormalizedField {
    let source_value = match (value, technologies) {
        (Some(value), Some(technologies)) if !technologies.is_empty() => {
            Some(format!("{value} — {technologies}"))
        }
        (Some(value), _) if !value.is_empty() => Some(value.to_string()),
        (None, Some(technologies)) if !technologies.is_empty() => Some(technologies.to_string()),
        _ => None,
    };
    let source = value.unwrap_or_default();
    let techs = technologies.map(extract_technologies).unwrap_or_default();
    if explicit_yes(source) || (!techs.is_empty() && !explicit_no(source)) {
        return NormalizedField {
            value: PcgamingwikiNormalizedValue::Yes,
            source_value,
            alternative_available: alternative_available(note),
            source_note: note.map(ToOwned::to_owned),
            technologies: techs,
            confidence: PcgamingwikiConfidence::High,
        };
    }
    if explicit_no(source) || workaround_only(source_value.as_deref()) {
        return NormalizedField {
            value: PcgamingwikiNormalizedValue::No,
            source_value,
            alternative_available: alternative_available(note),
            source_note: note.map(ToOwned::to_owned),
            technologies: techs,
            confidence: PcgamingwikiConfidence::High,
        };
    }
    NormalizedField {
        value: PcgamingwikiNormalizedValue::Unknown,
        source_value,
        alternative_available: alternative_available(note),
        source_note: note.map(ToOwned::to_owned),
        technologies: techs,
        confidence: PcgamingwikiConfidence::Low,
    }
}

fn alternative_available(note: Option<&str>) -> PcgamingwikiNormalizedValue {
    let Some(note) = note.map(str::trim).filter(|value| !value.is_empty()) else {
        return PcgamingwikiNormalizedValue::Unknown;
    };
    let normalized = note.to_ascii_lowercase();
    if normalized.contains("no workaround")
        || normalized.contains("no alternative")
        || normalized.contains("without workaround")
    {
        return PcgamingwikiNormalizedValue::No;
    }
    if normalized.contains("workaround")
        || normalized.contains("alternative")
        || normalized.contains("force native")
        || normalized.contains("glossary")
        || normalized.contains("engine page")
    {
        return PcgamingwikiNormalizedValue::Yes;
    }
    PcgamingwikiNormalizedValue::Unknown
}

fn note_or_default(value: Option<&str>, note: Option<&str>, default_note: &str) -> Option<String> {
    note.map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| explicit_no(value.unwrap_or_default()).then(|| default_note.to_string()))
}

fn explicit_yes(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    if explicit_no(value) {
        return false;
    }
    matches!(
        normalized.as_str(),
        "true" | "yes" | "y" | "available" | "supported" | "native" | "1"
    ) || normalized.contains("supported")
}

fn explicit_no(value: &str) -> bool {
    let normalized = value.trim().to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "false" | "no" | "n" | "none" | "unavailable" | "unsupported" | "0"
    ) || normalized.contains("not supported")
}

fn workaround_only(value: Option<&str>) -> bool {
    let value = value.unwrap_or_default().to_ascii_lowercase();
    (value.contains("workaround")
        || value.contains("mod")
        || value.contains("reshade")
        || value.contains("special k")
        || value.contains("auto hdr")
        || value.contains("rtx hdr")
        || value.contains("external"))
        && (value.contains("only")
            || value.contains("via")
            || value.contains("requires")
            || value.contains("alternative")
            || value.contains("see the glossary"))
}

fn extract_technologies(value: &str) -> Vec<String> {
    let mut seen = HashSet::new();
    value
        .split(|character| character == ',' || character == ';' || character == '\n')
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "-")
        .filter_map(|value| {
            let value = value.to_string();
            seen.insert(value.to_ascii_lowercase()).then_some(value)
        })
        .collect()
}

fn evidence(
    capability: PcgamingwikiCapability,
    field: NormalizedField,
    source_page: &str,
    source_field: &str,
) -> PcgamingwikiCapabilityEvidence {
    PcgamingwikiCapabilityEvidence {
        capability,
        normalized_value: field.value,
        source_value: field.source_value,
        alternative_available: field.alternative_available,
        source_note: field.source_note,
        technologies: field.technologies,
        source: PCGAMINGWIKI_SOURCE.to_string(),
        source_page: source_page.to_string(),
        source_field: source_field.to_string(),
        confidence: field.confidence,
        observed_at: now_string(),
        provider_version: PCGAMINGWIKI_PROVIDER_VERSION,
        stale: false,
    }
}

fn status_name(status: PcgamingwikiResolutionStatus) -> &'static str {
    match status {
        PcgamingwikiResolutionStatus::Resolved => "RESOLVED",
        PcgamingwikiResolutionStatus::NotFound => "NOT_FOUND",
        PcgamingwikiResolutionStatus::Forbidden => "PCGW_FORBIDDEN",
        PcgamingwikiResolutionStatus::IdentityUnavailable => "IDENTITY_UNAVAILABLE",
        PcgamingwikiResolutionStatus::RateLimited => "RATE_LIMITED",
        PcgamingwikiResolutionStatus::NetworkError => "NETWORK_ERROR",
        PcgamingwikiResolutionStatus::Timeout => "TIMEOUT",
        PcgamingwikiResolutionStatus::TemporaryFailure => "TEMPORARY_FAILURE",
        PcgamingwikiResolutionStatus::InvalidRedirect => "INVALID_REDIRECT",
        PcgamingwikiResolutionStatus::ParseFailure => "PARSE_FAILURE",
    }
}

fn empty_response(
    status: PcgamingwikiResolutionStatus,
    error: Option<String>,
) -> PcgamingwikiCapabilitiesResponse {
    PcgamingwikiCapabilitiesResponse {
        status,
        game_ref: None,
        capabilities: None,
        source: PCGAMINGWIKI_SOURCE.to_string(),
        provider_version: PCGAMINGWIKI_PROVIDER_VERSION,
        stale: false,
        conflict: None,
        error,
    }
}

fn response_from_cache(
    cached: CachedResult,
    stale: bool,
    conflict: Option<PcgamingwikiIdentityConflict>,
) -> PcgamingwikiCapabilitiesResponse {
    let mut capabilities = cached.capabilities;
    if stale {
        capabilities.native_hdr.stale = true;
        capabilities.high_fidelity_upscaling.stale = true;
        capabilities.frame_generation.stale = true;
    }
    PcgamingwikiCapabilitiesResponse {
        status: PcgamingwikiResolutionStatus::Resolved,
        game_ref: Some(cached.game_ref),
        capabilities: Some(capabilities),
        source: PCGAMINGWIKI_SOURCE.to_string(),
        provider_version: PCGAMINGWIKI_PROVIDER_VERSION,
        stale,
        conflict,
        error: None,
    }
}

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or_default()
}

fn now_string() -> String {
    now_seconds().to_string()
}

fn load_cached_result(
    state: &DatabaseState,
    game_id: &str,
) -> Result<Option<CachedResult>, rusqlite::Error> {
    let connection = state
        .connection
        .lock()
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    let mapping = connection.query_row(
        "SELECT page_title, page_identifier, canonical_url, steam_app_id, gog_product_id, resolved_via, resolved_at, last_checked_at, etag, last_modified, redirect_chain_json, identity_checked_at FROM pcgamingwiki_game_mapping WHERE game_id = ?1",
        [game_id],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, String>(2)?, row.get::<_, Option<i64>>(3)?, row.get::<_, Option<String>>(4)?, row.get::<_, String>(5)?, row.get::<_, String>(6)?, row.get::<_, i64>(7)?, row.get::<_, Option<String>>(8)?, row.get::<_, Option<String>>(9)?, row.get::<_, String>(10)?, row.get::<_, i64>(11)?)),
    ).optional()?;
    let Some((
        page_title,
        page_identifier,
        canonical_url,
        steam_app_id,
        gog_product_id,
        resolved_via,
        resolved_at,
        checked_at,
        etag,
        last_modified,
        redirect_chain_json,
        identity_checked_at,
    )) = mapping
    else {
        return Ok(None);
    };
    let resolved_via = match resolved_via.as_str() {
        "STEAM_APP_ID" | "MEDIAWIKI_STEAM_ID" => PcgamingwikiResolvedVia::MediaWikiSteamId,
        "GOG_PRODUCT_ID" | "MEDIAWIKI_GOG_ID" => PcgamingwikiResolvedVia::MediaWikiGogId,
        _ => return Ok(None),
    };
    let mut statement = connection.prepare("SELECT capability, normalized_value, source_value, alternative_available, source_note, technologies_json, source, source_page, source_field, confidence, observed_at, provider_version, stale FROM pcgamingwiki_capability_evidence WHERE game_id = ?1")?;
    let rows = statement.query_map([game_id], |row| {
        let capability: String = row.get(0)?;
        let normalized: String = row.get(1)?;
        let technologies_json: String = row.get(5)?;
        Ok(PcgamingwikiCapabilityEvidence {
            capability: parse_capability(&capability),
            normalized_value: parse_normalized(&normalized),
            source_value: row.get(2)?,
            alternative_available: parse_normalized(&row.get::<_, String>(3)?),
            source_note: row.get(4)?,
            technologies: serde_json::from_str(&technologies_json).unwrap_or_default(),
            source: row.get(6)?,
            source_page: row.get(7)?,
            source_field: row.get(8)?,
            confidence: parse_confidence(&row.get::<_, String>(9)?),
            observed_at: row.get(10)?,
            provider_version: row.get(11)?,
            stale: row.get::<_, i64>(12)? != 0,
        })
    })?;
    let mut evidence = HashMap::new();
    for row in rows {
        let value = row?;
        evidence.insert(capability_name(&value.capability), value);
    }
    let (Some(native_hdr), Some(high_fidelity_upscaling), Some(frame_generation)) = (
        evidence.remove("NATIVE_HDR"),
        evidence.remove("HIGH_FIDELITY_UPSCALING"),
        evidence.remove("FRAME_GENERATION"),
    ) else {
        return Ok(None);
    };
    Ok(Some(CachedResult {
        game_ref: PcgamingwikiGameRef {
            page_title,
            page_id: page_identifier,
            canonical_url,
            steam_app_id,
            gog_product_id,
            resolved_via,
            resolved_at,
            redirect_chain: serde_json::from_str(&redirect_chain_json).unwrap_or_default(),
        },
        capabilities: PcgamingwikiCapabilities {
            native_hdr,
            high_fidelity_upscaling,
            frame_generation,
        },
        checked_at,
        identity_checked_at,
        etag,
        last_modified,
    }))
}

fn persist_result(
    state: &DatabaseState,
    game_id: &str,
    game_ref: &PcgamingwikiGameRef,
    capabilities: &PcgamingwikiCapabilities,
    identity_checked_at: i64,
    etag: Option<String>,
    last_modified: Option<String>,
) -> Result<(), rusqlite::Error> {
    let connection = state
        .connection
        .lock()
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    let transaction = connection.unchecked_transaction()?;
    transaction.execute("INSERT INTO pcgamingwiki_game_mapping(game_id, page_identifier, page_title, canonical_url, steam_app_id, gog_product_id, resolved_via, resolved_at, last_checked_at, etag, last_modified, provider_version, redirect_chain_json, identity_checked_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14) ON CONFLICT(game_id) DO UPDATE SET page_identifier=excluded.page_identifier, page_title=excluded.page_title, canonical_url=excluded.canonical_url, steam_app_id=excluded.steam_app_id, gog_product_id=excluded.gog_product_id, resolved_via=excluded.resolved_via, resolved_at=excluded.resolved_at, last_checked_at=excluded.last_checked_at, etag=excluded.etag, last_modified=excluded.last_modified, provider_version=excluded.provider_version, redirect_chain_json=excluded.redirect_chain_json, identity_checked_at=excluded.identity_checked_at", rusqlite::params![game_id, game_ref.page_id, game_ref.page_title, game_ref.canonical_url, game_ref.steam_app_id, game_ref.gog_product_id, resolved_via_name(&game_ref.resolved_via), game_ref.resolved_at, now_seconds(), etag, last_modified, PCGAMINGWIKI_PROVIDER_VERSION, serde_json::to_string(&game_ref.redirect_chain).unwrap_or_else(|_| "[]".to_string()), identity_checked_at])?;
    for value in [
        &capabilities.native_hdr,
        &capabilities.high_fidelity_upscaling,
        &capabilities.frame_generation,
    ] {
        transaction.execute("INSERT INTO pcgamingwiki_capability_evidence(game_id, capability, normalized_value, source_value, alternative_available, source_note, technologies_json, source, source_page, source_field, confidence, observed_at, provider_version, stale) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 0) ON CONFLICT(game_id, capability) DO UPDATE SET normalized_value=excluded.normalized_value, source_value=excluded.source_value, alternative_available=excluded.alternative_available, source_note=excluded.source_note, technologies_json=excluded.technologies_json, source=excluded.source, source_page=excluded.source_page, source_field=excluded.source_field, confidence=excluded.confidence, observed_at=excluded.observed_at, provider_version=excluded.provider_version, stale=0", rusqlite::params![game_id, capability_name(&value.capability), normalized_name(&value.normalized_value), value.source_value, normalized_name(&value.alternative_available), value.source_note, serde_json::to_string(&value.technologies).unwrap_or_else(|_| "[]".to_string()), value.source, value.source_page, value.source_field, confidence_name(&value.confidence), value.observed_at, value.provider_version])?;
    }
    transaction.commit()
}

fn capability_name(value: &PcgamingwikiCapability) -> &'static str {
    match value {
        PcgamingwikiCapability::NativeHdr => "NATIVE_HDR",
        PcgamingwikiCapability::HighFidelityUpscaling => "HIGH_FIDELITY_UPSCALING",
        PcgamingwikiCapability::FrameGeneration => "FRAME_GENERATION",
    }
}
fn same_external_ids(
    cached: &CachedResult,
    steam_app_id: Option<i64>,
    gog_product_id: Option<&str>,
) -> bool {
    cached.game_ref.steam_app_id == steam_app_id
        && cached.game_ref.gog_product_id.as_deref() == gog_product_id
}
fn normalized_name(value: &PcgamingwikiNormalizedValue) -> &'static str {
    match value {
        PcgamingwikiNormalizedValue::Yes => "YES",
        PcgamingwikiNormalizedValue::No => "NO",
        PcgamingwikiNormalizedValue::Unknown => "UNKNOWN",
    }
}
fn confidence_name(value: &PcgamingwikiConfidence) -> &'static str {
    match value {
        PcgamingwikiConfidence::High => "HIGH",
        PcgamingwikiConfidence::Medium => "MEDIUM",
        PcgamingwikiConfidence::Low => "LOW",
    }
}
fn resolved_via_name(value: &PcgamingwikiResolvedVia) -> &'static str {
    match value {
        PcgamingwikiResolvedVia::SteamAppId | PcgamingwikiResolvedVia::MediaWikiSteamId => {
            "STEAM_APP_ID"
        }
        PcgamingwikiResolvedVia::GogProductId | PcgamingwikiResolvedVia::MediaWikiGogId => {
            "GOG_PRODUCT_ID"
        }
    }
}
fn parse_capability(value: &str) -> PcgamingwikiCapability {
    match value {
        "HIGH_FIDELITY_UPSCALING" => PcgamingwikiCapability::HighFidelityUpscaling,
        "FRAME_GENERATION" => PcgamingwikiCapability::FrameGeneration,
        _ => PcgamingwikiCapability::NativeHdr,
    }
}
fn parse_normalized(value: &str) -> PcgamingwikiNormalizedValue {
    match value {
        "YES" => PcgamingwikiNormalizedValue::Yes,
        "NO" => PcgamingwikiNormalizedValue::No,
        _ => PcgamingwikiNormalizedValue::Unknown,
    }
}
fn parse_confidence(value: &str) -> PcgamingwikiConfidence {
    match value {
        "HIGH" => PcgamingwikiConfidence::High,
        "MEDIUM" => PcgamingwikiConfidence::Medium,
        _ => PcgamingwikiConfidence::Low,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_supported_video_fields_and_technologies() {
        let source = "{{Video\n|hdr=true\n|upscaling=true\n|upscaling tech=DLSS 4, FSR 3.1\n|framegen=true\n|framegen tech=DLSS Frame Generation\n}}";
        let fields = parse_video_fields(source).expect("fields");
        let result = normalize_capabilities(&fields, "Example");
        assert!(matches!(
            result.native_hdr.normalized_value,
            PcgamingwikiNormalizedValue::Yes
        ));
        assert_eq!(
            result.high_fidelity_upscaling.technologies,
            ["DLSS 4", "FSR 3.1"]
        );
        assert_eq!(
            result.frame_generation.technologies,
            ["DLSS Frame Generation"]
        );
    }

    #[test]
    fn keeps_missing_and_ambiguous_values_unknown() {
        let fields =
            parse_video_fields("{{Video|hdr=maybe|upscaling=|framegen=}} ").expect("fields");
        let result = normalize_capabilities(&fields, "Example");
        assert!(matches!(
            result.native_hdr.normalized_value,
            PcgamingwikiNormalizedValue::Unknown
        ));
        assert!(matches!(
            result.high_fidelity_upscaling.normalized_value,
            PcgamingwikiNormalizedValue::Unknown
        ));
        assert!(matches!(
            result.frame_generation.normalized_value,
            PcgamingwikiNormalizedValue::Unknown
        ));
    }

    #[test]
    fn preserves_explicit_negative_upscaling_and_frame_generation() {
        let fields =
            parse_video_fields("{{Video|upscaling=not supported|framegen=false}}").expect("fields");
        let result = normalize_capabilities(&fields, "Example");
        assert!(matches!(
            result.high_fidelity_upscaling.normalized_value,
            PcgamingwikiNormalizedValue::No
        ));
        assert!(matches!(
            result.frame_generation.normalized_value,
            PcgamingwikiNormalizedValue::No
        ));
    }

    #[test]
    fn does_not_promote_hdr_workaround_to_native_support() {
        let fields = parse_video_fields("{{Video|hdr=|hdr notes=Only via Auto HDR workaround}} ")
            .expect("fields");
        let result = normalize_capabilities(&fields, "Example");
        assert!(matches!(
            result.native_hdr.normalized_value,
            PcgamingwikiNormalizedValue::No
        ));
        assert!(result
            .native_hdr
            .source_value
            .as_deref()
            .is_some_and(|value| value.contains("Auto HDR")));
        assert_eq!(
            result.native_hdr.alternative_available,
            PcgamingwikiNormalizedValue::Yes
        );
        assert_eq!(
            result.native_hdr.source_note.as_deref(),
            Some("Only via Auto HDR workaround")
        );
    }

    #[test]
    fn preserves_native_no_and_alternative_notes_separately() {
        let parsed: MediaWikiParseResponse = serde_json::from_slice(include_bytes!(
            "../test-fixtures/pcgamingwiki/parse-marvel.json"
        ))
        .expect("Marvel parse fixture");
        let page = parsed.parse.expect("page");
        let text = page
            .wikitext
            .and_then(MediaWikiWikitext::text)
            .expect("wikitext");
        let fields = parse_video_fields(&text).expect("Video fields");
        let capabilities = normalize_capabilities(&fields, "Marvel Tōkon: Fighting Souls");

        assert_eq!(
            capabilities.native_hdr.normalized_value,
            PcgamingwikiNormalizedValue::No
        );
        assert_eq!(
            capabilities.native_hdr.alternative_available,
            PcgamingwikiNormalizedValue::Yes
        );
        assert_eq!(
            capabilities.native_hdr.source_note.as_deref(),
            Some("See the engine page to force native HDR output, or the glossary page for other alternatives.")
        );
        assert_eq!(
            capabilities.frame_generation.normalized_value,
            PcgamingwikiNormalizedValue::No
        );
        assert_eq!(
            capabilities.frame_generation.alternative_available,
            PcgamingwikiNormalizedValue::Yes
        );
        assert_eq!(
            capabilities.frame_generation.source_note.as_deref(),
            Some("See the glossary page for potential workarounds.")
        );
    }

    #[test]
    fn rejects_external_redirect_host() {
        let url = Url::parse("https://example.com/wiki/Game").expect("url");
        assert!(matches!(
            validate_host(&url),
            Err(ProviderError::InvalidRedirect)
        ));
    }

    #[test]
    fn classifies_forbidden_separately_from_rate_limiting() {
        assert!(matches!(
            validate_status(StatusCode::FORBIDDEN),
            Err(ProviderError::Forbidden)
        ));
        assert!(matches!(
            validate_status(StatusCode::TOO_MANY_REQUESTS),
            Err(ProviderError::RateLimited)
        ));
        assert_eq!(
            status_name(PcgamingwikiResolutionStatus::Forbidden),
            "PCGW_FORBIDDEN"
        );
    }

    #[test]
    fn redacts_sensitive_http_body_previews() {
        assert_eq!(sanitize_body_preview(b"denied token=secret"), "<redacted>");
        assert_eq!(
            sanitize_body_preview(b"<html>Access denied</html>"),
            "<html>Access denied</html>"
        );
    }

    #[test]
    fn resolves_steam_identity_from_cargo_fixture() {
        let rows = parse_cargo_identity_response(include_bytes!(
            "../test-fixtures/pcgamingwiki/cargo-steam.json"
        ))
        .expect("Cargo fixture");
        let identity = ExternalIdentity::Steam("3787240".to_string());
        let row = rows
            .into_iter()
            .find(|row| row.matches_identity(&identity))
            .expect("Steam identity");
        let resolved = identity_from_cargo_row(row, &identity).expect("resolved identity");
        assert_eq!(resolved.game_ref.page_title, "Marvel Tōkon: Fighting Souls");
        assert_eq!(resolved.game_ref.page_id.as_deref(), Some("205571"));
        assert!(matches!(
            resolved.game_ref.resolved_via,
            PcgamingwikiResolvedVia::MediaWikiSteamId
        ));
    }

    #[test]
    fn resolves_gog_identity_from_cargo_fixture() {
        let rows = parse_cargo_identity_response(include_bytes!(
            "../test-fixtures/pcgamingwiki/cargo-gog-scan.json"
        ))
        .expect("Cargo fixture");
        let identity = ExternalIdentity::Gog("1785384169".to_string());
        let row = rows
            .into_iter()
            .find(|row| row.matches_identity(&identity))
            .expect("GOG identity");
        let resolved = identity_from_cargo_row(row, &identity).expect("resolved identity");
        assert_eq!(resolved.game_ref.page_title, "Carrion");
        assert_eq!(resolved.game_ref.page_id.as_deref(), Some("139686"));
        assert!(matches!(
            resolved.game_ref.resolved_via,
            PcgamingwikiResolvedVia::MediaWikiGogId
        ));
    }

    #[test]
    fn distinguishes_identity_not_found_and_malformed_cargo() {
        let rows = parse_cargo_identity_response(
            br#"{"cargoquery":[{"title":{"Page":"Example","PageID":"1","Steam AppID":"10"}}]}"#,
        )
        .expect("Cargo response");
        assert!(!rows[0].matches_identity(&ExternalIdentity::Steam("11".to_string())));
        assert!(matches!(
            parse_cargo_identity_response(br#"{"cargoquery":[}"#),
            Err(ProviderError::Parse)
        ));
    }

    #[test]
    fn keeps_same_page_and_detects_identity_conflict_by_page_id() {
        let steam = PcgamingwikiGameRef {
            page_title: "Carrion".to_string(),
            page_id: Some("139686".to_string()),
            canonical_url: canonical_page_url_from_title("Carrion"),
            steam_app_id: Some(953490),
            gog_product_id: None,
            resolved_via: PcgamingwikiResolvedVia::MediaWikiSteamId,
            resolved_at: now_string(),
            redirect_chain: Vec::new(),
        };
        let gog = PcgamingwikiGameRef {
            resolved_via: PcgamingwikiResolvedVia::MediaWikiGogId,
            ..steam.clone()
        };
        assert!(same_identity(&steam, &gog));
        let conflict = PcgamingwikiGameRef {
            page_id: Some("205571".to_string()),
            ..gog
        };
        assert!(!same_identity(&steam, &conflict));
    }

    #[test]
    fn parses_capability_fixtures_without_hardcoding_values() {
        for fixture in [
            &include_bytes!("../test-fixtures/pcgamingwiki/parse-marvel.json")[..],
            &include_bytes!("../test-fixtures/pcgamingwiki/parse-carrion.json")[..],
        ] {
            let parsed: MediaWikiParseResponse = serde_json::from_slice(fixture).expect("parse");
            let page = parsed.parse.expect("page");
            let text = page
                .wikitext
                .and_then(MediaWikiWikitext::text)
                .expect("wikitext");
            let fields = parse_video_fields(&text).expect("video fields");
            let capabilities = normalize_capabilities(&fields, "fixture");
            assert!(capabilities.native_hdr.source_value.is_some());
        }
    }

    #[test]
    #[ignore = "real PCGamingWiki MediaWiki QA; requires network access"]
    fn real_mediawiki_identity_and_capabilities_qa() {
        tauri::async_runtime::block_on(async {
            let root = std::env::temp_dir()
                .join(format!("lumadeck-pcgamingwiki-v12-{}", std::process::id()));
            let state = crate::settings::DatabaseState::open(
                crate::data_directory::DataDirectoryResolver::for_app_data(&root),
            )
            .expect("temporary database");

            let steam = get_capabilities(
                &state,
                PcgamingwikiCapabilitiesRequest {
                    game_id: "pcgw-qa-steam-3787240".to_string(),
                    steam_app_id: Some(3_787_240),
                    gog_product_id: None,
                    force_refresh: true,
                    cross_check_identities: false,
                },
            )
            .await
            .expect("Steam provider response");
            assert!(matches!(
                steam.status,
                PcgamingwikiResolutionStatus::Resolved
            ));
            let steam_ref = steam.game_ref.expect("Steam identity");
            assert_eq!(steam_ref.page_id.as_deref(), Some("205571"));
            assert_eq!(steam_ref.page_title, "Marvel Tōkon: Fighting Souls");
            let steam_capabilities = steam.capabilities.expect("Steam capabilities");
            assert!(matches!(
                steam_capabilities.native_hdr.normalized_value,
                PcgamingwikiNormalizedValue::No
            ));
            assert!(matches!(
                steam_capabilities.high_fidelity_upscaling.normalized_value,
                PcgamingwikiNormalizedValue::Yes
            ));
            assert!(matches!(
                steam_capabilities.frame_generation.normalized_value,
                PcgamingwikiNormalizedValue::No
            ));
            println!("STEAM_CAPABILITIES={steam_capabilities:?}");

            let gog = get_capabilities(
                &state,
                PcgamingwikiCapabilitiesRequest {
                    game_id: "pcgw-qa-gog-1785384169".to_string(),
                    steam_app_id: None,
                    gog_product_id: Some("1785384169".to_string()),
                    force_refresh: true,
                    cross_check_identities: false,
                },
            )
            .await
            .expect("GOG provider response");
            assert!(matches!(gog.status, PcgamingwikiResolutionStatus::Resolved));
            let gog_ref = gog.game_ref.expect("GOG identity");
            assert_eq!(gog_ref.page_id.as_deref(), Some("139686"));
            assert_eq!(gog_ref.page_title, "Carrion");
            let gog_capabilities = gog.capabilities.expect("GOG capabilities");
            assert_eq!(gog_capabilities.native_hdr.source_page, "Carrion");
            assert_eq!(
                gog_capabilities.high_fidelity_upscaling.source_page,
                "Carrion"
            );
            assert_eq!(gog_capabilities.frame_generation.source_page, "Carrion");
            println!("GOG_CAPABILITIES={gog_capabilities:?}");

            let log_path = state.logs_directory().join("settings-runtime.log");
            let cold_request_count = std::fs::read_to_string(&log_path)
                .expect("provider diagnostics")
                .matches("checkpoint=pcgw.http")
                .count();
            assert!(cold_request_count >= 4);

            let steam_warm = get_capabilities(
                &state,
                PcgamingwikiCapabilitiesRequest {
                    game_id: "pcgw-qa-steam-3787240".to_string(),
                    steam_app_id: Some(3_787_240),
                    gog_product_id: None,
                    force_refresh: false,
                    cross_check_identities: false,
                },
            )
            .await
            .expect("warm Steam provider response");
            assert!(matches!(
                steam_warm.status,
                PcgamingwikiResolutionStatus::Resolved
            ));
            let gog_warm = get_capabilities(
                &state,
                PcgamingwikiCapabilitiesRequest {
                    game_id: "pcgw-qa-gog-1785384169".to_string(),
                    steam_app_id: None,
                    gog_product_id: Some("1785384169".to_string()),
                    force_refresh: false,
                    cross_check_identities: false,
                },
            )
            .await
            .expect("warm GOG provider response");
            assert!(matches!(
                gog_warm.status,
                PcgamingwikiResolutionStatus::Resolved
            ));
            let warm_request_count = std::fs::read_to_string(&log_path)
                .expect("provider diagnostics after warm cache")
                .matches("checkpoint=pcgw.http")
                .count();
            println!(
                "REQUEST_COUNT cold={cold_request_count} warm_additional={}",
                warm_request_count - cold_request_count
            );
            assert_eq!(warm_request_count, cold_request_count);

            drop(state);
            std::fs::remove_dir_all(&root).expect("temporary QA database cleanup");
        })
    }

    #[test]
    fn decodes_page_titles_without_using_url_as_only_identity() {
        let url =
            Url::parse("https://www.pcgamingwiki.com/wiki/Marvel_T%C5%8Dkon%3A_Fighting_Souls")
                .expect("url");
        assert_eq!(
            title_from_url(&url).as_deref(),
            Some("Marvel Tōkon: Fighting Souls")
        );
    }
}
