use crate::{
    news::{NewsCategory, NewsItem, NewsRepository, NewsSyncState},
    settings::{self, DatabaseError, DatabaseState},
};
use chrono::{Local, TimeZone};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{
    future::Future,
    pin::Pin,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

pub const STEAM_NEWS_PROVIDER_ID: &str = "steam";
pub const STEAM_NEWS_REQUEST_LANGUAGE: &str = "english";
pub const DEFAULT_NEWS_COUNT: u32 = 20;
pub const DEFAULT_NEWS_MAX_LENGTH: u32 = 8_000;
const STEAM_NEWS_BASE_URL: &str = "https://api.steampowered.com";
const NEWS_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const NEWS_REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_NEWS_RESPONSE_BYTES: usize = 2 * 1024 * 1024;

#[derive(Debug, Clone, Error)]
pub enum SteamNewsError {
    #[error("Steam News request is invalid")]
    InvalidRequest,
    #[error("Steam News is unreachable")]
    Offline,
    #[error("Steam News request timed out")]
    Timeout,
    #[error("Steam News returned HTTP status {0}")]
    HttpStatus(u16),
    #[error("Steam News returned an invalid response")]
    InvalidResponse,
    #[error("Steam News response exceeded the size limit")]
    ResponseTooLarge,
    #[error("Steam News request could not be created")]
    RequestSetup,
}

impl SteamNewsError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidRequest => "STEAM_NEWS_INVALID_REQUEST",
            Self::Offline => "STEAM_NEWS_OFFLINE",
            Self::Timeout => "STEAM_NEWS_TIMEOUT",
            Self::HttpStatus(_) => "STEAM_NEWS_API_ERROR",
            Self::InvalidResponse => "STEAM_NEWS_INVALID_RESPONSE",
            Self::ResponseTooLarge => "STEAM_NEWS_RESPONSE_TOO_LARGE",
            Self::RequestSetup => "STEAM_NEWS_REQUEST_ERROR",
        }
    }
}

#[derive(Debug, Error)]
pub enum NewsSyncError {
    #[error("Steam metadata is unavailable for this game")]
    SteamMetadataUnavailable,
    #[error(transparent)]
    Provider(#[from] SteamNewsError),
    #[error(transparent)]
    Database(#[from] DatabaseError),
}

impl NewsSyncError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::SteamMetadataUnavailable => "STEAM_METADATA_NOT_AVAILABLE",
            Self::Provider(error) => error.code(),
            Self::Database(_) => "DATABASE_ERROR",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamNewsRequest {
    pub app_id: i64,
    pub count: u32,
    pub max_length: u32,
    pub end_date: Option<i64>,
    pub language: String,
    pub feeds: Option<String>,
}

impl SteamNewsRequest {
    pub fn conservative(app_id: i64, count: Option<u32>, max_length: Option<u32>) -> Self {
        Self {
            app_id,
            count: count.unwrap_or(DEFAULT_NEWS_COUNT).clamp(1, 50),
            max_length: max_length
                .unwrap_or(DEFAULT_NEWS_MAX_LENGTH)
                .clamp(256, 20_000),
            end_date: None,
            language: STEAM_NEWS_REQUEST_LANGUAGE.to_string(),
            feeds: None,
        }
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct SteamNewsEntry {
    #[serde(default)]
    pub gid: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    #[serde(rename = "is_external_url")]
    pub is_external_url: Option<bool>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub contents: Option<String>,
    #[serde(default)]
    pub feedlabel: Option<String>,
    #[serde(default)]
    pub feedname: Option<String>,
    #[serde(default)]
    pub date: Option<i64>,
    #[serde(default)]
    #[serde(rename = "feed_type")]
    pub feed_type: Option<i64>,
    #[serde(default)]
    pub appid: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SteamNewsPage {
    pub app_id: i64,
    pub items: Vec<SteamNewsEntry>,
}

#[derive(Debug, Deserialize)]
struct SteamNewsEnvelope {
    #[serde(default)]
    appnews: Option<SteamNewsPayload>,
}

#[derive(Debug, Deserialize)]
struct SteamNewsPayload {
    #[serde(default)]
    appid: Option<i64>,
    #[serde(default)]
    newsitems: Vec<SteamNewsEntry>,
}

pub trait SteamNewsSource: Send + Sync {
    fn fetch<'a>(
        &'a self,
        request: SteamNewsRequest,
    ) -> Pin<Box<dyn Future<Output = Result<SteamNewsPage, SteamNewsError>> + Send + 'a>>;
}

pub struct SteamNewsProvider {
    http: Client,
    base_url: String,
}

impl SteamNewsProvider {
    pub fn new() -> Result<Self, SteamNewsError> {
        Self::with_base_url(STEAM_NEWS_BASE_URL)
    }

    pub fn with_base_url(base_url: impl Into<String>) -> Result<Self, SteamNewsError> {
        Self::with_options(base_url, NEWS_CONNECT_TIMEOUT, NEWS_REQUEST_TIMEOUT)
    }

    pub fn with_options(
        base_url: impl Into<String>,
        connect_timeout: Duration,
        request_timeout: Duration,
    ) -> Result<Self, SteamNewsError> {
        let http = Client::builder()
            .connect_timeout(connect_timeout)
            .timeout(request_timeout)
            .user_agent("LumaDeck/SteamNewsV1")
            .build()
            .map_err(|_| SteamNewsError::RequestSetup)?;
        Ok(Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
        })
    }

    async fn fetch_page(&self, request: SteamNewsRequest) -> Result<SteamNewsPage, SteamNewsError> {
        if request.app_id <= 0 || request.count == 0 || request.max_length == 0 {
            return Err(SteamNewsError::InvalidRequest);
        }
        let url = format!(
            "{}/ISteamNews/GetNewsForApp/v2/",
            self.base_url.trim_end_matches('/')
        );
        let mut query = vec![
            ("appid", request.app_id.to_string()),
            ("count", request.count.to_string()),
            ("maxlength", request.max_length.to_string()),
            ("l", request.language.clone()),
            ("format", "json".to_string()),
        ];
        if let Some(end_date) = request.end_date {
            query.push(("enddate", end_date.to_string()));
        }
        if let Some(feeds) = request.feeds {
            if !feeds.trim().is_empty() {
                query.push(("feeds", feeds));
            }
        }
        let response = self
            .http
            .get(url)
            .query(&query)
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    SteamNewsError::Timeout
                } else {
                    SteamNewsError::Offline
                }
            })?;
        let status = response.status();
        if !status.is_success() {
            return Err(SteamNewsError::HttpStatus(status.as_u16()));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_NEWS_RESPONSE_BYTES as u64)
        {
            return Err(SteamNewsError::ResponseTooLarge);
        }
        let body = response
            .bytes()
            .await
            .map_err(|_| SteamNewsError::Offline)?;
        if body.len() > MAX_NEWS_RESPONSE_BYTES {
            return Err(SteamNewsError::ResponseTooLarge);
        }
        let envelope: SteamNewsEnvelope =
            serde_json::from_slice(&body).map_err(|_| SteamNewsError::InvalidResponse)?;
        let payload = envelope.appnews.ok_or(SteamNewsError::InvalidResponse)?;
        Ok(SteamNewsPage {
            app_id: payload.appid.unwrap_or(request.app_id),
            items: payload.newsitems,
        })
    }
}

impl SteamNewsSource for SteamNewsProvider {
    fn fetch<'a>(
        &'a self,
        request: SteamNewsRequest,
    ) -> Pin<Box<dyn Future<Output = Result<SteamNewsPage, SteamNewsError>> + Send + 'a>> {
        Box::pin(self.fetch_page(request))
    }
}

pub struct SteamNewsNormalizer;

impl SteamNewsNormalizer {
    pub fn normalize(
        entry: SteamNewsEntry,
        game_id: &str,
        app_id: i64,
        source_language: &str,
    ) -> Option<NewsItem> {
        let title = entry.title?.trim().to_string();
        let source_url = entry.url?.trim().to_string();
        let published_at = entry.date.filter(|value| *value > 0)?.to_string();
        if title.is_empty() || source_url.is_empty() {
            return None;
        }
        let canonical_url = canonical_url(&source_url);
        let raw_contents = entry.contents.clone();
        let external_id = entry
            .gid
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .or_else(|| canonical_url.as_ref().map(|value| format!("url:{value}")))
            .unwrap_or_else(|| format!("url:{source_url}"));
        let original_content = entry
            .contents
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let original_summary = original_content
            .as_deref()
            .map(news_summary)
            .filter(|value| !value.is_empty());
        let detected_source_language =
            detect_source_language(&title, original_content.as_deref(), source_language);
        let mut metadata = Map::new();
        insert_metadata(&mut metadata, "feedname", entry.feedname);
        insert_metadata(&mut metadata, "feedlabel", entry.feedlabel);
        insert_metadata(&mut metadata, "author", entry.author);
        if let Some(value) = entry.is_external_url {
            metadata.insert("isExternalUrl".to_string(), Value::Bool(value));
        }
        if let Some(value) = entry.feed_type {
            metadata.insert("feedType".to_string(), Value::from(value));
        }
        if let Some(value) = entry.appid {
            metadata.insert("responseAppId".to_string(), Value::from(value));
        }
        if let Some(image_url) = extract_first_image_url(raw_contents.as_deref()) {
            metadata.insert("imageUrl".to_string(), Value::String(image_url));
            metadata.insert("imageSource".to_string(), Value::String("news".to_string()));
        }
        metadata.insert(
            "sourceLanguageBasis".to_string(),
            Value::String(
                if detected_source_language == source_language {
                    "requested"
                } else {
                    "detected-cyrillic"
                }
                .to_string(),
            ),
        );
        let mut item = NewsItem {
            id: NewsItem::stable_id(
                STEAM_NEWS_PROVIDER_ID,
                Some(&external_id),
                canonical_url.as_deref(),
            ),
            provider_id: STEAM_NEWS_PROVIDER_ID.to_string(),
            external_id,
            game_id: game_id.to_string(),
            external_game_id: Some(app_id.to_string()),
            category: classify_steam_news(
                metadata
                    .get("feedname")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                &title,
            ),
            source_url,
            canonical_url,
            published_at,
            updated_at: None,
            first_seen_at: unix_timestamp(),
            source_language: detected_source_language,
            original_title: title,
            original_summary,
            original_content,
            content_format: content_format(
                metadata
                    .get("feedname")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                raw_contents.as_deref(),
            ),
            source_content_hash: String::new(),
            provider_metadata: Some(Value::Object(metadata)),
            created_at: unix_timestamp(),
            persisted_updated_at: unix_timestamp(),
        };
        item.refresh_source_content_hash();
        Some(item)
    }

    pub fn normalize_batch(
        entries: Vec<SteamNewsEntry>,
        game_id: &str,
        app_id: i64,
        source_language: &str,
    ) -> (Vec<NewsItem>, usize) {
        let mut discarded = 0;
        let items = entries
            .into_iter()
            .filter_map(|entry| {
                let item = Self::normalize(entry, game_id, app_id, source_language);
                if item.is_none() {
                    discarded += 1;
                }
                item
            })
            .collect();
        (items, discarded)
    }
}

fn news_summary(value: &str) -> String {
    let plain = value
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .split('<')
        .enumerate()
        .map(|(index, part)| {
            if index == 0 {
                part.to_string()
            } else {
                part.split_once('>')
                    .map(|(_, text)| text)
                    .unwrap_or("")
                    .to_string()
            }
        })
        .collect::<String>()
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let mut summary = plain.chars().take(180).collect::<String>();
    if plain.chars().count() > 180 {
        summary.push('…');
    }
    summary
}

pub(crate) fn detect_source_language(
    title: &str,
    content: Option<&str>,
    requested: &str,
) -> String {
    let cyrillic_count = title
        .chars()
        .chain(content.unwrap_or_default().chars())
        .filter(|character| matches!(*character, '\u{0400}'..='\u{04ff}'))
        .count();
    if cyrillic_count >= 4 {
        "ru".to_string()
    } else {
        requested.to_string()
    }
}

pub fn classify_steam_news(feedname: &str, title: &str) -> NewsCategory {
    let feed = feedname.trim().to_ascii_lowercase();
    let title = title.trim().to_ascii_lowercase();
    let signal = if feed.is_empty() { &title } else { &feed };
    if signal.contains("dlc") || signal.contains("downloadable") {
        NewsCategory::Dlc
    } else if signal.contains("maintenance") || signal.contains("server") {
        NewsCategory::Maintenance
    } else if signal.contains("event") || signal.contains("tournament") {
        NewsCategory::Event
    } else if signal.contains("update")
        || signal.contains("patch")
        || signal.contains("changelog")
        || signal.contains("release")
    {
        NewsCategory::Update
    } else if signal.contains("community") {
        NewsCategory::Community
    } else if signal.contains("media") || signal.contains("video") {
        NewsCategory::Media
    } else if signal.contains("official")
        || signal.contains("announcement")
        || signal.contains("news")
    {
        NewsCategory::Official
    } else {
        NewsCategory::Other
    }
}

pub fn deduplicate_steam_news(mut items: Vec<NewsItem>) -> (Vec<NewsItem>, usize) {
    let mut deduplicated = Vec::with_capacity(items.len());
    let mut removed = 0;
    for item in items.drain(..) {
        let duplicate_index = deduplicated.iter().position(|existing: &NewsItem| {
            existing.provider_id == item.provider_id
                && (existing.external_id == item.external_id
                    || (existing.canonical_url.is_some()
                        && existing.canonical_url == item.canonical_url))
        });
        if let Some(index) = duplicate_index {
            let replacement = prefer_complete_news_item(&deduplicated[index], &item);
            deduplicated[index] = replacement;
            removed += 1;
        } else {
            deduplicated.push(item);
        }
    }
    (deduplicated, removed)
}

fn prefer_complete_news_item(left: &NewsItem, right: &NewsItem) -> NewsItem {
    let left_score = completeness_score(left);
    let right_score = completeness_score(right);
    if right_score > left_score || (right_score == left_score && right.id < left.id) {
        right.clone()
    } else {
        left.clone()
    }
}

fn completeness_score(item: &NewsItem) -> (usize, usize, usize, usize, usize) {
    (
        item.original_content.as_deref().map_or(0, str::len),
        item.original_summary.as_deref().map_or(0, str::len),
        item.original_title.len(),
        usize::from(item.canonical_url.is_some()),
        usize::from(item.provider_metadata.is_some()),
    )
}

fn canonical_url(value: &str) -> Option<String> {
    let lower = value.to_ascii_lowercase();
    (lower.starts_with("https://") || lower.starts_with("http://")).then(|| value.to_string())
}

fn content_format(feedname: &str, content: Option<&str>) -> crate::news::NewsContentFormat {
    if content.is_none() {
        return crate::news::NewsContentFormat::Unknown;
    }
    let _ = feedname;
    if content.is_some_and(|value| value.contains('<') && value.contains('>')) {
        crate::news::NewsContentFormat::Html
    } else {
        crate::news::NewsContentFormat::PlainText
    }
}

fn insert_metadata(metadata: &mut Map<String, Value>, key: &str, value: Option<String>) {
    if let Some(value) = value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        metadata.insert(key.to_string(), Value::String(value));
    }
}

fn extract_first_image_url(content: Option<&str>) -> Option<String> {
    let content = content?;
    let lower = content.to_ascii_lowercase();
    let image_start = lower.find("<img")?;
    let tag_end = lower[image_start..].find('>')? + image_start;
    let tag = &content[image_start..tag_end];
    let tag_lower = tag.to_ascii_lowercase();
    for attribute in ["src", "data-src"] {
        let Some(attribute_position) = tag_lower.find(attribute) else {
            continue;
        };
        let attribute_start = attribute_position + attribute.len();
        let value = tag[attribute_start..].trim_start();
        let value = value.strip_prefix('=')?.trim_start();
        let quote = value.chars().next()?;
        if !matches!(quote, '\'' | '"') {
            continue;
        }
        let value = value[quote.len_utf8()..].split(quote).next()?.trim();
        if value.starts_with("https://") {
            return Some(value.to_string());
        }
    }
    None
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsSyncResult {
    pub provider_id: String,
    pub game_id: String,
    pub steam_app_id: i64,
    pub fetched_count: usize,
    pub accepted_count: usize,
    pub discarded_count: usize,
    pub deduplicated_count: usize,
    pub inserted_count: usize,
    pub updated_count: usize,
    pub unchanged_count: usize,
    pub skipped_due_to_freshness: bool,
    pub last_successful_sync_at: Option<String>,
    pub warnings: Vec<String>,
}

pub struct RefreshGameNewsUseCase<'a, P> {
    state: &'a DatabaseState,
    provider: P,
}

impl<'a> RefreshGameNewsUseCase<'a, SteamNewsProvider> {
    pub fn new(state: &'a DatabaseState) -> Result<Self, SteamNewsError> {
        Ok(Self {
            state,
            provider: SteamNewsProvider::new()?,
        })
    }
}

impl<'a, P> RefreshGameNewsUseCase<'a, P>
where
    P: SteamNewsSource,
{
    #[cfg(test)]
    pub fn with_provider(state: &'a DatabaseState, provider: P) -> Self {
        Self { state, provider }
    }

    pub async fn refresh(
        &self,
        game_id: &str,
        count: Option<u32>,
        max_length: Option<u32>,
        force_refresh: bool,
    ) -> Result<NewsSyncResult, NewsSyncError> {
        if game_id.trim().is_empty() {
            return Err(NewsSyncError::Provider(SteamNewsError::InvalidRequest));
        }
        let app_id = settings::get_steam_app_id(self.state, game_id)?
            .ok_or(NewsSyncError::SteamMetadataUnavailable)?;
        if app_id <= 0 {
            return Err(NewsSyncError::SteamMetadataUnavailable);
        }
        let repository = NewsRepository::new(self.state);
        let previous = repository.get_sync_state(STEAM_NEWS_PROVIDER_ID, game_id)?;
        if !force_refresh
            && previous
                .as_ref()
                .and_then(|state| state.last_successful_sync_at.as_deref())
                .is_some_and(is_same_local_day)
        {
            let count = repository.get_news_items_by_game(game_id)?.len();
            return Ok(NewsSyncResult {
                provider_id: STEAM_NEWS_PROVIDER_ID.to_string(),
                game_id: game_id.to_string(),
                steam_app_id: app_id,
                fetched_count: 0,
                accepted_count: count,
                discarded_count: 0,
                deduplicated_count: 0,
                inserted_count: 0,
                updated_count: 0,
                unchanged_count: count,
                skipped_due_to_freshness: true,
                last_successful_sync_at: previous.and_then(|state| state.last_successful_sync_at),
                warnings: vec!["NEWS_REFRESH_SKIPPED_FRESH".to_string()],
            });
        }

        let now = unix_timestamp();
        let mut sync_state = previous.unwrap_or_else(|| NewsSyncState {
            provider_id: STEAM_NEWS_PROVIDER_ID.to_string(),
            game_id: game_id.to_string(),
            last_successful_sync_at: None,
            last_attempt_at: None,
            last_error_code: None,
            cursor: None,
            is_stale: false,
            created_at: now.clone(),
            updated_at: now.clone(),
        });
        sync_state.last_attempt_at = Some(now.clone());
        sync_state.last_error_code = None;
        sync_state.updated_at = now.clone();
        repository.save_sync_state(&sync_state)?;

        let request = SteamNewsRequest::conservative(app_id, count, max_length);
        let page = match self.provider.fetch(request).await {
            Ok(page) => page,
            Err(error) => {
                let sync_error = NewsSyncError::from(error);
                sync_state.last_error_code = Some(sync_error.code().to_string());
                sync_state.is_stale = repository
                    .get_news_items_by_game(game_id)
                    .map(|items| !items.is_empty())
                    .unwrap_or(true);
                sync_state.updated_at = unix_timestamp();
                let _ = repository.save_sync_state(&sync_state);
                return Err(sync_error);
            }
        };
        if page.app_id > 0 && page.app_id != app_id {
            return self.fail_sync(
                repository,
                sync_state,
                NewsSyncError::Provider(SteamNewsError::InvalidResponse),
                game_id,
            );
        }

        let fetched_count = page.items.len();
        let (normalized, discarded_count) =
            SteamNewsNormalizer::normalize_batch(page.items, game_id, app_id, "en");
        let (items, deduplicated_count) = deduplicate_steam_news(normalized);
        let accepted_count = items.len();
        let before = items
            .iter()
            .map(|item| repository.get_news_item_by_id(&item.id))
            .collect::<Result<Vec<_>, _>>()?;
        let persisted = match repository.upsert_news_items(&items) {
            Ok(value) => value,
            Err(error) => {
                return self.fail_sync(
                    repository,
                    sync_state,
                    NewsSyncError::Database(error),
                    game_id,
                );
            }
        };
        let mut inserted_count = 0;
        let mut updated_count = 0;
        let mut unchanged_count = 0;
        for ((incoming, existing), saved) in items.iter().zip(before).zip(persisted.iter()) {
            match existing {
                None if saved.id == incoming.id => inserted_count += 1,
                None => updated_count += 1,
                Some(previous) if previous.source_content_hash == incoming.source_content_hash => {
                    unchanged_count += 1
                }
                Some(_) => updated_count += 1,
            }
        }
        let completed_at = unix_timestamp();
        sync_state.last_successful_sync_at = Some(completed_at.clone());
        sync_state.last_attempt_at = Some(completed_at.clone());
        sync_state.last_error_code = None;
        sync_state.is_stale = false;
        sync_state.updated_at = completed_at.clone();
        repository.save_sync_state(&sync_state)?;
        let mut warnings = Vec::new();
        if discarded_count > 0 {
            warnings.push("STEAM_NEWS_INVALID_ITEMS_DISCARDED".to_string());
        }
        Ok(NewsSyncResult {
            provider_id: STEAM_NEWS_PROVIDER_ID.to_string(),
            game_id: game_id.to_string(),
            steam_app_id: app_id,
            fetched_count,
            accepted_count,
            discarded_count,
            deduplicated_count,
            inserted_count,
            updated_count,
            unchanged_count,
            skipped_due_to_freshness: false,
            last_successful_sync_at: Some(completed_at),
            warnings,
        })
    }

    fn fail_sync(
        &self,
        repository: NewsRepository<'a>,
        mut sync_state: NewsSyncState,
        error: NewsSyncError,
        game_id: &str,
    ) -> Result<NewsSyncResult, NewsSyncError> {
        sync_state.last_error_code = Some(error.code().to_string());
        sync_state.is_stale = repository
            .get_news_items_by_game(game_id)
            .map(|items| !items.is_empty())
            .unwrap_or(true);
        sync_state.updated_at = unix_timestamp();
        let _ = repository.save_sync_state(&sync_state);
        Err(error)
    }
}

fn is_same_local_day(value: &str) -> bool {
    let now_timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or_default();
    is_same_local_day_at(value, now_timestamp)
}

fn is_same_local_day_at(value: &str, now_timestamp: i64) -> bool {
    let Ok(previous_timestamp) = value.parse::<i64>() else {
        return false;
    };
    let Some(previous_date) = Local.timestamp_opt(previous_timestamp, 0).single() else {
        return false;
    };
    let Some(now_date) = Local.timestamp_opt(now_timestamp, 0).single() else {
        return false;
    };
    previous_date.date_naive() == now_date.date_naive()
}

fn unix_timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

pub fn get_game_news_sync_state(
    state: &DatabaseState,
    game_id: &str,
) -> Result<Option<NewsSyncState>, DatabaseError> {
    NewsRepository::new(state).get_sync_state(STEAM_NEWS_PROVIDER_ID, game_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        data_directory::DataDirectoryResolver,
        news::{NewsContentFormat, NewsRepository},
    };
    use std::{
        fs,
        io::{Read, Write},
        net::TcpListener,
        path::PathBuf,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex,
        },
        thread::{self, JoinHandle},
    };

    fn entry(
        gid: Option<&str>,
        title: Option<&str>,
        url: Option<&str>,
        date: Option<i64>,
        feedname: Option<&str>,
        contents: Option<&str>,
    ) -> SteamNewsEntry {
        SteamNewsEntry {
            gid: gid.map(ToString::to_string),
            title: title.map(ToString::to_string),
            url: url.map(ToString::to_string),
            is_external_url: Some(false),
            author: Some("Luma Author".to_string()),
            contents: contents.map(ToString::to_string),
            feedlabel: Some("Official News".to_string()),
            feedname: feedname.map(ToString::to_string),
            date,
            feed_type: Some(1),
            appid: Some(440),
        }
    }

    fn test_page(contents: &str) -> SteamNewsPage {
        SteamNewsPage {
            app_id: 440,
            items: vec![entry(
                Some("gid-1"),
                Some("A Steam update"),
                Some("https://example.test/news/1"),
                Some(1_700_000_000),
                Some("patchnotes"),
                Some(contents),
            )],
        }
    }

    #[derive(Clone)]
    struct MockSource {
        result: Arc<Mutex<Result<SteamNewsPage, SteamNewsError>>>,
        calls: Arc<AtomicUsize>,
    }

    impl MockSource {
        fn new(result: Result<SteamNewsPage, SteamNewsError>) -> Self {
            Self {
                result: Arc::new(Mutex::new(result)),
                calls: Arc::new(AtomicUsize::new(0)),
            }
        }

        fn set_result(&self, result: Result<SteamNewsPage, SteamNewsError>) {
            *self.result.lock().expect("mock result lock") = result;
        }

        fn call_count(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    impl SteamNewsSource for MockSource {
        fn fetch<'a>(
            &'a self,
            _request: SteamNewsRequest,
        ) -> Pin<Box<dyn Future<Output = Result<SteamNewsPage, SteamNewsError>> + Send + 'a>>
        {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let result = self.result.lock().expect("mock result lock").clone();
            Box::pin(async move { result })
        }
    }

    fn test_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "lumadeck-steam-news-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }

    fn with_test_state(test: impl FnOnce(&DatabaseState)) {
        let root = test_root("database");
        let state = DatabaseState::open(DataDirectoryResolver::for_app_data(&root))
            .expect("database state");
        state
            .connection
            .lock()
            .expect("database connection")
            .execute_batch(
                "INSERT INTO games(
                   id, title, sort_title, provider, platform, favorite, installed,
                   progress, status, created_at, updated_at
                 ) VALUES ('game-001', 'Test game', 'test game', 'steam', 'Windows', 0, 1,
                           0, 'not-started', '1700000000', '1700000000');
                 INSERT INTO game_details(game_id, steam_app_id, steam_updated_at)
                   VALUES ('game-001', 440, '1700000000');",
            )
            .expect("test game");
        test(&state);
        drop(state);
        fs::remove_dir_all(root).expect("remove database");
    }

    fn spawn_server(status: u16, body: String, delay: Duration) -> (String, JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let address = format!("http://{}", listener.local_addr().expect("server address"));
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut request = [0_u8; 4096];
            let count = stream.read(&mut request).expect("read request");
            thread::sleep(delay);
            let status_text = match status {
                200 => "OK",
                503 => "Service Unavailable",
                _ => "Error",
            };
            let response = format!(
                "HTTP/1.1 {status} {status_text}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            let _ = stream.write_all(response.as_bytes());
            String::from_utf8_lossy(&request[..count]).to_string()
        });
        (address, handle)
    }

    #[test]
    fn deserializes_valid_steam_response_and_optional_fields() {
        let response: SteamNewsEnvelope = serde_json::from_str(
            r#"{"appnews":{"appid":440,"newsitems":[{"gid":"1","title":"Patch","url":"https://example.test/1","date":1700000000,"feedname":"patchnotes","feed_type":1,"is_external_url":false}]}}"#,
        )
        .expect("valid Steam response");
        let payload = response.appnews.expect("appnews");
        assert_eq!(payload.appid, Some(440));
        assert_eq!(payload.newsitems[0].feed_type, Some(1));
        assert_eq!(payload.newsitems[0].is_external_url, Some(false));
        assert!(payload.newsitems[0].contents.is_none());
        let missing: SteamNewsEntry =
            serde_json::from_str(r#"{"title":"Only a title","url":"https://example.test/2"}"#)
                .expect("optional fields");
        assert!(missing.gid.is_none());
        assert!(missing.date.is_none());
    }

    #[test]
    fn rejects_invalid_json_and_normalizes_valid_items() {
        assert!(serde_json::from_str::<SteamNewsEnvelope>("not json").is_err());
        let normalized = SteamNewsNormalizer::normalize(
            entry(
                Some("gid-1"),
                Some("Patch"),
                Some("https://example.test/1"),
                Some(1_700_000_000),
                Some("patchnotes"),
                Some("<p>Patch content</p>"),
            ),
            "game-001",
            440,
            "en",
        )
        .expect("normalized item");
        assert_eq!(normalized.provider_id, STEAM_NEWS_PROVIDER_ID);
        assert_eq!(normalized.external_game_id.as_deref(), Some("440"));
        assert_eq!(normalized.source_language, "en");
        assert_eq!(normalized.content_format, NewsContentFormat::Html);
        assert_eq!(normalized.category, NewsCategory::Update);
    }

    #[test]
    fn extracts_first_https_news_image_from_steam_content() {
        let normalized = SteamNewsNormalizer::normalize(
            entry(
                Some("gid-image"),
                Some("Patch with artwork"),
                Some("https://example.test/image"),
                Some(1_700_000_000),
                Some("patchnotes"),
                Some(
                    r#"<p>Patch</p><img alt="art" src="https://cdn.cloudflare.steamstatic.com/steam/apps/440/header.jpg">"#,
                ),
            ),
            "game-001",
            440,
            "en",
        )
        .expect("normalized item");
        let metadata = normalized
            .provider_metadata
            .as_ref()
            .and_then(Value::as_object)
            .expect("metadata");
        assert_eq!(
            metadata.get("imageUrl").and_then(Value::as_str),
            Some("https://cdn.cloudflare.steamstatic.com/steam/apps/440/header.jpg")
        );
        assert_eq!(
            metadata.get("imageSource").and_then(Value::as_str),
            Some("news")
        );
    }

    #[test]
    fn detects_cyrillic_external_news_even_when_english_was_requested() {
        let normalized = SteamNewsNormalizer::normalize(
            entry(
                Some("gid-ru"),
                Some("RTX 3060 и системные требования"),
                Some("https://example.test/ru"),
                Some(1_700_000_000),
                Some("external"),
                Some("<p>Студия опубликовала системные требования игры.</p>"),
            ),
            "game-001",
            440,
            "en",
        )
        .expect("normalized item");
        assert_eq!(normalized.source_language, "ru");
        assert_eq!(
            normalized
                .provider_metadata
                .as_ref()
                .and_then(|metadata| metadata.get("sourceLanguageBasis"))
                .and_then(Value::as_str),
            Some("detected-cyrillic")
        );
    }

    #[test]
    fn discards_items_without_minimum_fields_without_failing_batch() {
        let (items, discarded) = SteamNewsNormalizer::normalize_batch(
            vec![
                entry(
                    Some("valid"),
                    Some("Valid"),
                    Some("https://example.test/valid"),
                    Some(10),
                    Some("news"),
                    Some("Content"),
                ),
                entry(None, Some("Missing URL"), None, Some(10), None, None),
            ],
            "game-001",
            440,
            "en",
        );
        assert_eq!(items.len(), 1);
        assert_eq!(discarded, 1);
    }

    #[test]
    fn classifies_structured_steam_feeds_and_has_other_fallback() {
        assert_eq!(
            classify_steam_news("patchnotes", "Anything"),
            NewsCategory::Update
        );
        assert_eq!(
            classify_steam_news("event", "Anything"),
            NewsCategory::Event
        );
        assert_eq!(classify_steam_news("dlc", "Anything"), NewsCategory::Dlc);
        assert_eq!(
            classify_steam_news("maintenance", "Anything"),
            NewsCategory::Maintenance
        );
        assert_eq!(
            classify_steam_news("", "Unclassified text"),
            NewsCategory::Other
        );
    }

    #[test]
    fn deduplicates_by_external_id_and_canonical_url_preferring_complete_content() {
        let first = SteamNewsNormalizer::normalize(
            entry(
                Some("same-gid"),
                Some("Title"),
                Some("https://example.test/same"),
                Some(10),
                Some("news"),
                Some("short"),
            ),
            "game-001",
            440,
            "en",
        )
        .expect("first item");
        let second = SteamNewsNormalizer::normalize(
            entry(
                Some("different-gid"),
                Some("Title"),
                Some("https://example.test/same"),
                Some(10),
                Some("news"),
                Some("a much more complete article body"),
            ),
            "game-001",
            440,
            "en",
        )
        .expect("second item");
        let (items, count) = deduplicate_steam_news(vec![first, second]);
        assert_eq!(items.len(), 1);
        assert_eq!(count, 1);
        assert_eq!(
            items[0].original_content.as_deref(),
            Some("a much more complete article body")
        );
    }

    #[test]
    fn provider_requests_english_and_handles_http_errors() {
        let body = r#"{"appnews":{"appid":440,"newsitems":[]}}"#;
        let (address, handle) = spawn_server(200, body.to_string(), Duration::ZERO);
        let provider = SteamNewsProvider::with_base_url(address).expect("provider");
        let page = tauri::async_runtime::block_on(provider.fetch(SteamNewsRequest {
            app_id: 440,
            count: 2,
            max_length: 500,
            end_date: None,
            language: STEAM_NEWS_REQUEST_LANGUAGE.to_string(),
            feeds: None,
        }))
        .expect("Steam response");
        let request = handle.join().expect("server thread");
        assert_eq!(page.app_id, 440);
        assert!(request.contains("appid=440"));
        assert!(request.contains("count=2"));
        assert!(request.contains("l=english"));

        let (address, handle) = spawn_server(503, "{}".to_string(), Duration::ZERO);
        let provider = SteamNewsProvider::with_base_url(address).expect("provider");
        let error = tauri::async_runtime::block_on(
            provider.fetch(SteamNewsRequest::conservative(440, None, None)),
        )
        .expect_err("HTTP error");
        handle.join().expect("server thread");
        assert!(matches!(error, SteamNewsError::HttpStatus(503)));
    }

    #[test]
    fn provider_respects_request_timeout() {
        let (address, handle) = spawn_server(
            200,
            r#"{"appnews":{"appid":440,"newsitems":[]}}"#.to_string(),
            Duration::from_millis(100),
        );
        let provider = SteamNewsProvider::with_options(
            address,
            Duration::from_secs(1),
            Duration::from_millis(10),
        )
        .expect("provider");
        let error = tauri::async_runtime::block_on(
            provider.fetch(SteamNewsRequest::conservative(440, None, None)),
        )
        .expect_err("timeout");
        handle.join().expect("server thread");
        assert!(matches!(error, SteamNewsError::Timeout));
    }

    #[test]
    fn news_refresh_uses_the_local_calendar_day_not_a_rolling_24_hour_window() {
        let late_evening = Local
            .with_ymd_and_hms(2026, 8, 5, 23, 0, 0)
            .single()
            .expect("late evening")
            .timestamp();
        let next_morning = Local
            .with_ymd_and_hms(2026, 8, 6, 6, 0, 0)
            .single()
            .expect("next morning")
            .timestamp();
        let same_day = Local
            .with_ymd_and_hms(2026, 8, 5, 23, 30, 0)
            .single()
            .expect("same day")
            .timestamp();

        assert!(!is_same_local_day_at(
            &late_evening.to_string(),
            next_morning
        ));
        assert!(is_same_local_day_at(&late_evening.to_string(), same_day));
    }

    #[test]
    fn refresh_persists_news_without_duplicates_and_updates_changed_content() {
        with_test_state(|state| {
            let source = MockSource::new(Ok(test_page("Original body")));
            let first = tauri::async_runtime::block_on(
                RefreshGameNewsUseCase::with_provider(state, source.clone()).refresh(
                    "game-001",
                    Some(2),
                    Some(500),
                    false,
                ),
            )
            .expect("first refresh");
            assert_eq!(first.inserted_count, 1);
            assert_eq!(first.discarded_count, 0);

            let second = tauri::async_runtime::block_on(
                RefreshGameNewsUseCase::with_provider(state, source.clone()).refresh(
                    "game-001",
                    Some(2),
                    Some(500),
                    true,
                ),
            )
            .expect("second refresh");
            assert_eq!(second.unchanged_count, 1);
            assert_eq!(
                NewsRepository::new(state)
                    .get_news_items_by_game("game-001")
                    .expect("feed")
                    .len(),
                1
            );

            source.set_result(Ok(test_page("Changed body with more detail")));
            let third = tauri::async_runtime::block_on(
                RefreshGameNewsUseCase::with_provider(state, source)
                    .refresh("game-001", None, None, true),
            )
            .expect("changed refresh");
            assert_eq!(third.updated_count, 1);
            let item = NewsRepository::new(state)
                .get_news_items_by_game("game-001")
                .expect("feed")
                .pop()
                .expect("item");
            assert_eq!(
                item.original_content.as_deref(),
                Some("Changed body with more detail")
            );
        });
    }

    #[test]
    fn refresh_records_success_failure_preserves_feed_and_obeys_freshness() {
        with_test_state(|state| {
            let source = MockSource::new(Ok(test_page("Original body")));
            tauri::async_runtime::block_on(
                RefreshGameNewsUseCase::with_provider(state, source.clone())
                    .refresh("game-001", None, None, false),
            )
            .expect("initial refresh");
            let skipped = tauri::async_runtime::block_on(
                RefreshGameNewsUseCase::with_provider(state, source.clone())
                    .refresh("game-001", None, None, false),
            )
            .expect("fresh refresh");
            assert!(skipped.skipped_due_to_freshness);
            assert_eq!(source.call_count(), 1);

            source.set_result(Err(SteamNewsError::Offline));
            let error = tauri::async_runtime::block_on(
                RefreshGameNewsUseCase::with_provider(state, source)
                    .refresh("game-001", None, None, true),
            )
            .expect_err("failed refresh");
            assert_eq!(error.code(), "STEAM_NEWS_OFFLINE");
            let sync = NewsRepository::new(state)
                .get_sync_state(STEAM_NEWS_PROVIDER_ID, "game-001")
                .expect("sync state")
                .expect("sync state row");
            assert!(sync.is_stale);
            assert_eq!(sync.last_error_code.as_deref(), Some("STEAM_NEWS_OFFLINE"));
            assert_eq!(
                NewsRepository::new(state)
                    .get_news_items_by_game("game-001")
                    .expect("previous feed")
                    .len(),
                1
            );
        });
    }
}
