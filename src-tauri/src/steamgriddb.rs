use reqwest::header::{ACCEPT, AUTHORIZATION};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

const API_BASE_URL: &str = "https://www.steamgriddb.com/api/v2";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_RESULTS_PER_QUERY: usize = 200;
const QUERY_TTL_SECS: u64 = 10 * 60;
const MAX_CACHED_QUERIES: usize = 20;
static QUERY_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum ArtworkKind {
    Grid,
    Hero,
    Logo,
    Icon,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum ArtworkSlot {
    #[serde(rename = "grid_horizontal")]
    GridHorizontal,
    #[serde(rename = "grid_vertical")]
    GridVertical,
    #[serde(rename = "grid_square")]
    GridSquare,
    #[serde(rename = "hero")]
    Hero,
    #[serde(rename = "logo")]
    Logo,
    #[serde(rename = "icon")]
    Icon,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(transparent)]
pub struct GridStyle(String);

impl GridStyle {
    pub const NO_LOGO: &str = "no_logo";
    pub const ALTERNATE: &str = "alternate";
    pub const BLURRED: &str = "blurred";
    pub const MATERIAL: &str = "material";
    pub const WHITE_LOGO: &str = "white_logo";

    pub fn new(value: impl Into<String>) -> Option<Self> {
        let value = value.into().trim().to_lowercase();
        (!value.is_empty()).then_some(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkTarget {
    kind: ArtworkKind,
    slot: ArtworkSlot,
    style: Option<GridStyle>,
}

impl ArtworkTarget {
    pub fn try_new(kind: ArtworkKind, slot: ArtworkSlot, style: Option<GridStyle>) -> Option<Self> {
        let grid_slot = matches!(
            slot,
            ArtworkSlot::GridHorizontal | ArtworkSlot::GridVertical | ArtworkSlot::GridSquare
        );
        let kind_matches = match kind {
            ArtworkKind::Grid => grid_slot,
            ArtworkKind::Hero => slot == ArtworkSlot::Hero,
            ArtworkKind::Logo => slot == ArtworkSlot::Logo,
            ArtworkKind::Icon => slot == ArtworkSlot::Icon,
        };
        if !kind_matches || (!grid_slot && style.is_some()) {
            return None;
        }
        Some(Self { kind, slot, style })
    }

    pub fn kind(&self) -> ArtworkKind {
        self.kind
    }

    pub fn slot(&self) -> ArtworkSlot {
        self.slot
    }

    pub fn style(&self) -> Option<&GridStyle> {
        self.style.as_ref()
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkSize {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AssetSizePolicy {
    pub preferred_size: Option<ArtworkSize>,
    pub fallback_size: Option<ArtworkSize>,
    pub allow_smaller_assets: bool,
    pub aspect_ratio_priority: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ImageCachePolicy {
    Disabled,
    Enabled,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CompressionPolicy {
    Disabled,
    PreserveSource,
    PreferWebp,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtworkFilterKind {
    All,
    NoLogo,
    Other,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkSearchRequest {
    pub game_id: String,
    pub slot: ArtworkSlot,
    pub style_filter: ArtworkFilterKind,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkSearchResult {
    pub query_id: String,
    pub game_id: String,
    pub slot: ArtworkSlot,
    pub style_filter: ArtworkFilterKind,
    pub identity: SteamGridDbGameIdentity,
    pub candidates: Vec<ArtworkPreviewCandidate>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamGridDbGameIdentity {
    pub local_game_id: String,
    pub title: String,
    pub steam_app_id: Option<i64>,
    pub steamgriddb_game_id: Option<i64>,
    pub source: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct LocalGameIdentity {
    pub local_game_id: String,
    pub title: String,
    pub steam_app_id: Option<i64>,
    pub platform: String,
    pub source: String,
    pub title_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkPreviewCandidate {
    pub candidate_id: String,
    pub external_asset_id: i64,
    pub external_game_id: i64,
    pub kind: ArtworkKind,
    pub slot: ArtworkSlot,
    pub grid_style: Option<GridStyle>,
    pub width: u32,
    pub height: u32,
    pub aspect_ratio: f32,
    pub thumbnail_url: String,
    pub mime_type: Option<String>,
    pub score: Option<f32>,
    pub upvotes: Option<i64>,
    pub downvotes: Option<i64>,
    pub nsfw: bool,
    pub locked: bool,
    pub author_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SteamGridDbRemoteAsset {
    pub external_asset_id: i64,
    pub external_game_id: i64,
    pub kind: ArtworkKind,
    pub grid_style: Option<GridStyle>,
    pub width: u32,
    pub height: u32,
    pub aspect_ratio: f32,
    pub source_url: String,
    pub thumbnail_url: String,
    pub mime_type: Option<String>,
    pub score: Option<f32>,
    pub upvotes: Option<i64>,
    pub downvotes: Option<i64>,
    pub nsfw: bool,
    pub locked: bool,
    pub ephemeral: bool,
    pub author_name: Option<String>,
    pub author_steam64: Option<String>,
}

#[derive(Debug, Error)]
pub enum SteamGridDbError {
    #[error("SteamGridDB credential is unavailable")]
    CredentialUnavailable,
    #[error("SteamGridDB request is invalid")]
    InvalidRequest,
    #[error("SteamGridDB is unreachable")]
    Offline,
    #[error("SteamGridDB request timed out")]
    Timeout,
    #[error("SteamGridDB returned HTTP status {0}")]
    Api(u16),
    #[error("SteamGridDB rate limit exceeded")]
    RateLimited,
    #[error("SteamGridDB returned an invalid response")]
    InvalidResponse,
    #[error("SteamGridDB game could not be resolved")]
    GameUnresolved,
    #[error("SteamGridDB game resolution is ambiguous")]
    GameAmbiguous,
    #[error("SteamGridDB candidate expired")]
    CandidateExpired,
    #[error("SteamGridDB candidate does not belong to this query")]
    CandidateContextMismatch,
}

#[derive(Debug, Deserialize)]
struct ApiEnvelope<T> {
    #[serde(default)]
    success: bool,
    data: Option<T>,
}

#[derive(Debug, Deserialize)]
struct RawGame {
    id: i64,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawAsset {
    id: i64,
    #[serde(default)]
    game: Option<i64>,
    #[serde(default)]
    style: Option<String>,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
    url: String,
    thumb: String,
    #[serde(default)]
    mime: Option<String>,
    #[serde(default)]
    score: Option<f32>,
    #[serde(default)]
    upvotes: Option<i64>,
    #[serde(default)]
    downvotes: Option<i64>,
    #[serde(default)]
    nsfw: bool,
    #[serde(default)]
    locked: bool,
    #[serde(default)]
    ephemeral: bool,
    #[serde(default)]
    author: Option<RawAuthor>,
}

#[derive(Debug, Deserialize)]
struct RawAuthor {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    steam64: Option<String>,
}

pub struct SteamGridDbClient {
    http: reqwest::Client,
    api_key: String,
}

impl SteamGridDbClient {
    pub fn new(api_key: String) -> Result<Self, SteamGridDbError> {
        if api_key.trim().is_empty() {
            return Err(SteamGridDbError::CredentialUnavailable);
        }
        let http = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(REQUEST_TIMEOUT)
            .user_agent("LumaDeck/SteamGridDBV1")
            .build()
            .map_err(|_| SteamGridDbError::Offline)?;
        Ok(Self { http, api_key })
    }

    pub async fn resolve_steam_game(&self, steam_app_id: i64) -> Result<i64, SteamGridDbError> {
        if steam_app_id <= 0 {
            return Err(SteamGridDbError::InvalidRequest);
        }
        let data: RawGame = self
            .get_json(&format!("/games/steam/{steam_app_id}"), &Vec::new())
            .await?;
        Ok(data.id)
    }

    pub async fn resolve_title(&self, title: &str) -> Result<i64, SteamGridDbError> {
        let title = title.trim();
        if title.is_empty() {
            return Err(SteamGridDbError::InvalidRequest);
        }
        let path = format!("/search/autocomplete/{}", encode_path_segment(title));
        let data: Vec<RawGame> = self.get_json(&path, &Vec::new()).await?;
        let matches = data
            .into_iter()
            .filter(|game| {
                game.name
                    .as_deref()
                    .is_some_and(|name| normalize_title(name) == normalize_title(title))
            })
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [game] => Ok(game.id),
            [] => Err(SteamGridDbError::GameUnresolved),
            _ => Err(SteamGridDbError::GameAmbiguous),
        }
    }

    pub async fn get_assets(
        &self,
        external_game_id: i64,
        slot: ArtworkSlot,
        style_filter: ArtworkFilterKind,
    ) -> Result<Vec<SteamGridDbRemoteAsset>, SteamGridDbError> {
        let endpoint = match slot {
            ArtworkSlot::GridHorizontal | ArtworkSlot::GridVertical | ArtworkSlot::GridSquare => {
                "grids"
            }
            ArtworkSlot::Hero => "heroes",
            ArtworkSlot::Logo => "logos",
            ArtworkSlot::Icon => "icons",
        };
        let query = build_asset_query(slot, style_filter);
        let data: Vec<RawAsset> = self
            .get_json(&format!("/{endpoint}/game/{external_game_id}"), &query)
            .await?;
        data.into_iter()
            .filter_map(|asset| map_remote_asset(asset, external_game_id, slot))
            .filter(|asset| match style_filter {
                ArtworkFilterKind::All => true,
                ArtworkFilterKind::NoLogo => asset
                    .grid_style
                    .as_ref()
                    .is_some_and(|style| style.as_str() == GridStyle::NO_LOGO),
                ArtworkFilterKind::Other => asset
                    .grid_style
                    .as_ref()
                    .is_some_and(|style| style.as_str() != GridStyle::NO_LOGO),
            })
            .take(MAX_RESULTS_PER_QUERY)
            .collect::<Vec<_>>()
            .pipe(Ok)
    }

    pub async fn get_assets_for_enrichment(
        &self,
        external_game_id: i64,
        slot: ArtworkSlot,
    ) -> Result<Vec<SteamGridDbRemoteAsset>, SteamGridDbError> {
        let endpoint = match slot {
            ArtworkSlot::GridHorizontal | ArtworkSlot::GridVertical | ArtworkSlot::GridSquare => {
                "grids"
            }
            ArtworkSlot::Hero => "heroes",
            ArtworkSlot::Logo => "logos",
            ArtworkSlot::Icon => "icons",
        };
        let data: Vec<RawAsset> = self
            .get_json(&format!("/{endpoint}/game/{external_game_id}"), &Vec::new())
            .await?;
        Ok(data
            .into_iter()
            .filter_map(|asset| map_remote_asset(asset, external_game_id, slot))
            .take(MAX_RESULTS_PER_QUERY)
            .collect())
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(String, String)],
    ) -> Result<T, SteamGridDbError> {
        let url = format!("{}{}", API_BASE_URL, path);
        let response = self
            .http
            .get(url)
            .header(AUTHORIZATION, format!("Bearer {}", self.api_key))
            .header(ACCEPT, "application/json")
            .query(query)
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    SteamGridDbError::Timeout
                } else {
                    SteamGridDbError::Offline
                }
            })?;
        let status = response.status();
        if status.as_u16() == 429 {
            return Err(SteamGridDbError::RateLimited);
        }
        if !status.is_success() {
            return Err(SteamGridDbError::Api(status.as_u16()));
        }
        let envelope = response
            .json::<ApiEnvelope<T>>()
            .await
            .map_err(|_| SteamGridDbError::InvalidResponse)?;
        if !envelope.success {
            return Err(SteamGridDbError::InvalidResponse);
        }
        envelope.data.ok_or(SteamGridDbError::InvalidResponse)
    }
}

pub fn select_best_asset(
    slot: ArtworkSlot,
    assets: &[SteamGridDbRemoteAsset],
) -> Option<SteamGridDbRemoteAsset> {
    assets
        .iter()
        .filter(|asset| asset_matches_slot(asset, slot))
        .filter(|asset| !asset.nsfw)
        .max_by(|left, right| {
            let left_key = asset_quality_key(left);
            let right_key = asset_quality_key(right);
            left_key.cmp(&right_key)
        })
        .cloned()
}

fn asset_matches_slot(asset: &SteamGridDbRemoteAsset, slot: ArtworkSlot) -> bool {
    let ratio = asset.width as f32 / asset.height as f32;
    match slot {
        ArtworkSlot::GridHorizontal => ratio >= 1.45,
        ArtworkSlot::GridVertical => ratio <= 0.9,
        ArtworkSlot::GridSquare => (0.9..=1.1).contains(&ratio),
        ArtworkSlot::Hero => ratio >= 2.0,
        ArtworkSlot::Logo => true,
        ArtworkSlot::Icon => true,
    }
}

fn asset_quality_key(asset: &SteamGridDbRemoteAsset) -> (u64, i64, i64, i64) {
    let pixels = u64::from(asset.width) * u64::from(asset.height);
    let score = asset.score.map(|value| (value * 100.0) as i64).unwrap_or(0);
    let upvotes = asset.upvotes.unwrap_or(0) - asset.downvotes.unwrap_or(0);
    let transparency = if asset.kind == ArtworkKind::Logo
        && asset
            .mime_type
            .as_deref()
            .is_some_and(|mime| mime.eq_ignore_ascii_case("image/png"))
    {
        1
    } else {
        0
    };
    (pixels, transparency, score, upvotes)
}

fn build_asset_query(slot: ArtworkSlot, style_filter: ArtworkFilterKind) -> Vec<(String, String)> {
    let mut query = Vec::new();
    if matches!(slot, ArtworkSlot::GridHorizontal) {
        query.push(("dimensions".to_string(), "920x430".to_string()));
    } else if matches!(slot, ArtworkSlot::GridVertical) {
        query.push(("dimensions".to_string(), "600x900".to_string()));
    } else if matches!(slot, ArtworkSlot::GridSquare) {
        query.push(("dimensions".to_string(), "1024x1024,512x512".to_string()));
    } else if matches!(slot, ArtworkSlot::Hero) {
        query.push(("dimensions".to_string(), "3840x1240,1920x620".to_string()));
    }
    let is_grid_slot = matches!(
        slot,
        ArtworkSlot::GridHorizontal | ArtworkSlot::GridVertical | ArtworkSlot::GridSquare
    );
    if is_grid_slot && matches!(style_filter, ArtworkFilterKind::NoLogo) {
        query.push(("styles".to_string(), GridStyle::NO_LOGO.to_string()));
    }
    query
}

fn map_remote_asset(
    asset: RawAsset,
    external_game_id: i64,
    slot: ArtworkSlot,
) -> Option<SteamGridDbRemoteAsset> {
    let width = asset.width?;
    let height = asset.height?;
    if width == 0 || height == 0 || !is_https_url(&asset.url) || !is_https_url(&asset.thumb) {
        return None;
    }
    let grid_style = asset.style.and_then(GridStyle::new);
    let kind = match slot {
        ArtworkSlot::GridHorizontal | ArtworkSlot::GridVertical | ArtworkSlot::GridSquare => {
            ArtworkKind::Grid
        }
        ArtworkSlot::Hero => ArtworkKind::Hero,
        ArtworkSlot::Logo => ArtworkKind::Logo,
        ArtworkSlot::Icon => ArtworkKind::Icon,
    };
    Some(SteamGridDbRemoteAsset {
        external_asset_id: asset.id,
        external_game_id: asset.game.unwrap_or(external_game_id),
        kind,
        grid_style,
        width,
        height,
        aspect_ratio: width as f32 / height as f32,
        source_url: asset.url,
        thumbnail_url: asset.thumb,
        mime_type: asset.mime,
        score: asset.score,
        upvotes: asset.upvotes,
        downvotes: asset.downvotes,
        nsfw: asset.nsfw,
        locked: asset.locked,
        ephemeral: asset.ephemeral,
        author_name: asset.author.as_ref().and_then(|author| author.name.clone()),
        author_steam64: asset.author.and_then(|author| author.steam64),
    })
}

fn is_https_url(value: &str) -> bool {
    value
        .strip_prefix("https://")
        .is_some_and(|rest| !rest.is_empty() && !rest.starts_with('/'))
}

fn encode_path_segment(value: &str) -> String {
    value.bytes().fold(String::new(), |mut encoded, byte| {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push('%');
            encoded.push_str(&format!("{byte:02X}"));
        }
        encoded
    })
}

fn normalize_title(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric() || character.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

#[derive(Debug, Clone)]
struct CachedCandidate {
    query_id: String,
    game_id: String,
    slot: ArtworkSlot,
    style_filter: ArtworkFilterKind,
    asset: SteamGridDbRemoteAsset,
}

#[derive(Debug)]
pub struct SteamGridDbQueryCache {
    queries: VecDeque<(String, u64)>,
    candidates: HashMap<String, CachedCandidate>,
}

impl Default for SteamGridDbQueryCache {
    fn default() -> Self {
        Self {
            queries: VecDeque::new(),
            candidates: HashMap::new(),
        }
    }
}

impl SteamGridDbQueryCache {
    pub fn insert(
        &mut self,
        game_id: String,
        slot: ArtworkSlot,
        style_filter: ArtworkFilterKind,
        assets: Vec<SteamGridDbRemoteAsset>,
    ) -> (String, Vec<ArtworkPreviewCandidate>) {
        self.prune_expired();
        let query_id = format!(
            "q-{:x}-{:x}",
            unix_timestamp_millis(),
            QUERY_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        );
        let expires_at = unix_timestamp_secs().saturating_add(QUERY_TTL_SECS);
        self.queries.push_back((query_id.clone(), expires_at));
        let candidates = assets
            .into_iter()
            .take(MAX_RESULTS_PER_QUERY)
            .map(|asset| {
                let candidate_id = format!("{query_id}-{}", asset.external_asset_id);
                self.candidates.insert(
                    candidate_id.clone(),
                    CachedCandidate {
                        query_id: query_id.clone(),
                        game_id: game_id.clone(),
                        slot,
                        style_filter,
                        asset: asset.clone(),
                    },
                );
                ArtworkPreviewCandidate {
                    candidate_id,
                    external_asset_id: asset.external_asset_id,
                    external_game_id: asset.external_game_id,
                    kind: asset.kind,
                    slot,
                    grid_style: asset.grid_style.clone(),
                    width: asset.width,
                    height: asset.height,
                    aspect_ratio: asset.aspect_ratio,
                    thumbnail_url: asset.thumbnail_url,
                    mime_type: asset.mime_type,
                    score: asset.score,
                    upvotes: asset.upvotes,
                    downvotes: asset.downvotes,
                    nsfw: asset.nsfw,
                    locked: asset.locked,
                    author_name: asset.author_name,
                }
            })
            .collect::<Vec<_>>();
        while self.queries.len() > MAX_CACHED_QUERIES {
            if let Some((old_query_id, _)) = self.queries.pop_front() {
                self.candidates
                    .retain(|_, value| value.query_id != old_query_id);
            }
        }
        (query_id, candidates)
    }

    pub fn get(
        &mut self,
        candidate_id: &str,
        expected_game_id: &str,
        expected_slot: ArtworkSlot,
        expected_style_filter: ArtworkFilterKind,
    ) -> Result<SteamGridDbRemoteAsset, SteamGridDbError> {
        self.prune_expired();
        let candidate = self
            .candidates
            .get(candidate_id)
            .ok_or(SteamGridDbError::CandidateExpired)?;
        if candidate.game_id != expected_game_id
            || candidate.slot != expected_slot
            || candidate.style_filter != expected_style_filter
        {
            return Err(SteamGridDbError::CandidateContextMismatch);
        }
        Ok(candidate.asset.clone())
    }

    fn prune_expired(&mut self) {
        let now = unix_timestamp_secs();
        let mut expired = Vec::new();
        self.queries.retain(|(query_id, expires_at)| {
            let keep = *expires_at > now;
            if !keep {
                expired.push(query_id.clone());
            }
            keep
        });
        self.candidates
            .retain(|_, candidate| !expired.iter().any(|id| id == &candidate.query_id));
    }
}

fn unix_timestamp_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn unix_timestamp_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

trait Pipe: Sized {
    fn pipe<T>(self, function: impl FnOnce(Self) -> T) -> T {
        function(self)
    }
}

impl<T> Pipe for T {}

#[cfg(test)]
mod tests {
    use super::{
        build_asset_query, is_https_url, select_best_asset, ArtworkFilterKind, ArtworkKind,
        ArtworkSlot, ArtworkTarget, GridStyle, SteamGridDbQueryCache, SteamGridDbRemoteAsset,
    };

    fn asset(width: u32, height: u32, kind: ArtworkKind) -> SteamGridDbRemoteAsset {
        SteamGridDbRemoteAsset {
            external_asset_id: i64::from(width),
            external_game_id: 7,
            kind,
            grid_style: None,
            width,
            height,
            aspect_ratio: width as f32 / height as f32,
            source_url: "https://images.steamgriddb.com/original.png".to_string(),
            thumbnail_url: "https://images.steamgriddb.com/thumb.png".to_string(),
            mime_type: Some("image/png".to_string()),
            score: Some(8.0),
            upvotes: Some(10),
            downvotes: Some(1),
            nsfw: false,
            locked: false,
            ephemeral: false,
            author_name: None,
            author_steam64: None,
        }
    }

    #[test]
    fn validates_artwork_target_combinations() {
        let style = GridStyle::new(GridStyle::NO_LOGO).expect("style");
        assert!(ArtworkTarget::try_new(
            ArtworkKind::Grid,
            ArtworkSlot::GridVertical,
            Some(style.clone())
        )
        .is_some());
        assert!(ArtworkTarget::try_new(ArtworkKind::Hero, ArtworkSlot::Hero, None).is_some());
        assert!(
            ArtworkTarget::try_new(ArtworkKind::Hero, ArtworkSlot::Hero, Some(style)).is_none()
        );
        assert!(ArtworkTarget::try_new(ArtworkKind::Logo, ArtworkSlot::GridSquare, None).is_none());
    }

    #[test]
    fn asset_query_keeps_logo_and_icon_requests_unfiltered_by_unsupported_options() {
        assert!(build_asset_query(ArtworkSlot::Logo, ArtworkFilterKind::All).is_empty());
        assert!(build_asset_query(ArtworkSlot::Icon, ArtworkFilterKind::All).is_empty());
        assert!(build_asset_query(ArtworkSlot::Logo, ArtworkFilterKind::NoLogo).is_empty());
        assert_eq!(
            build_asset_query(ArtworkSlot::GridHorizontal, ArtworkFilterKind::NoLogo),
            vec![
                ("dimensions".to_string(), "920x430".to_string()),
                ("styles".to_string(), "no_logo".to_string())
            ]
        );
    }

    #[test]
    fn asset_query_only_adds_dimensions_to_supported_aspect_slots() {
        assert_eq!(
            build_asset_query(ArtworkSlot::GridHorizontal, ArtworkFilterKind::All),
            vec![("dimensions".to_string(), "920x430".to_string())]
        );
        assert_eq!(
            build_asset_query(ArtworkSlot::Hero, ArtworkFilterKind::All),
            vec![("dimensions".to_string(), "3840x1240,1920x620".to_string())]
        );
        assert_eq!(
            build_asset_query(ArtworkSlot::GridSquare, ArtworkFilterKind::All),
            vec![("dimensions".to_string(), "1024x1024,512x512".to_string())]
        );
    }

    #[test]
    fn cache_requires_the_original_query_context() {
        let asset = SteamGridDbRemoteAsset {
            external_asset_id: 42,
            external_game_id: 7,
            kind: ArtworkKind::Grid,
            grid_style: GridStyle::new(GridStyle::NO_LOGO),
            width: 600,
            height: 900,
            aspect_ratio: 600.0 / 900.0,
            source_url: "https://images.steamgriddb.com/original.png".to_string(),
            thumbnail_url: "https://images.steamgriddb.com/thumb.png".to_string(),
            mime_type: Some("image/png".to_string()),
            score: Some(8.5),
            upvotes: Some(10),
            downvotes: Some(1),
            nsfw: false,
            locked: false,
            ephemeral: false,
            author_name: None,
            author_steam64: None,
        };
        let mut cache = SteamGridDbQueryCache::default();
        let (_, candidates) = cache.insert(
            "game-1".to_string(),
            ArtworkSlot::GridVertical,
            ArtworkFilterKind::NoLogo,
            vec![asset],
        );
        let candidate = &candidates[0];
        assert!(cache
            .get(
                &candidate.candidate_id,
                "game-1",
                ArtworkSlot::GridVertical,
                ArtworkFilterKind::NoLogo,
            )
            .is_ok());
        assert!(matches!(
            cache.get(
                &candidate.candidate_id,
                "game-2",
                ArtworkSlot::GridVertical,
                ArtworkFilterKind::NoLogo,
            ),
            Err(super::SteamGridDbError::CandidateContextMismatch)
        ));
        let public_json = serde_json::to_string(candidate).expect("preview serialization");
        assert!(!public_json.contains("original.png"));
    }

    #[test]
    fn only_https_urls_are_accepted_for_remote_assets() {
        assert!(is_https_url("https://images.steamgriddb.com/grid.png"));
        assert!(!is_https_url("http://images.steamgriddb.com/grid.png"));
        assert!(!is_https_url("data:image/png;base64,abc"));
        assert!(!is_https_url("https:///grid.png"));
    }

    #[test]
    fn selects_highest_valid_density_without_mixing_aspect_types() {
        let assets = vec![
            asset(920, 430, ArtworkKind::Grid),
            asset(460, 215, ArtworkKind::Grid),
            asset(600, 900, ArtworkKind::Grid),
            asset(2048, 2048, ArtworkKind::Grid),
        ];
        assert_eq!(
            select_best_asset(ArtworkSlot::GridHorizontal, &assets)
                .expect("horizontal")
                .width,
            920
        );
        assert_eq!(
            select_best_asset(ArtworkSlot::GridVertical, &assets)
                .expect("vertical")
                .height,
            900
        );
        assert_eq!(
            select_best_asset(ArtworkSlot::GridSquare, &assets)
                .expect("square")
                .width,
            2048
        );
    }

    #[test]
    fn a_larger_wrong_type_never_wins() {
        let assets = vec![
            asset(4000, 4000, ArtworkKind::Grid),
            asset(920, 430, ArtworkKind::Grid),
        ];
        assert_eq!(
            select_best_asset(ArtworkSlot::GridHorizontal, &assets)
                .expect("horizontal")
                .width,
            920
        );
    }

    #[test]
    fn logo_selection_prefers_transparent_png_when_density_is_equal() {
        let mut png = asset(1024, 512, ArtworkKind::Logo);
        let mut webp = asset(1024, 512, ArtworkKind::Logo);
        webp.mime_type = Some("image/webp".to_string());
        png.upvotes = Some(1);
        webp.upvotes = Some(100);
        assert_eq!(
            select_best_asset(ArtworkSlot::Logo, &[webp, png])
                .expect("logo")
                .mime_type
                .as_deref(),
            Some("image/png")
        );
    }
}
