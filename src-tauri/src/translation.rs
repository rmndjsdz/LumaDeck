use crate::{
    news::{
        NewsCategory, NewsContentFormat, NewsFeedItem, NewsItem, NewsRepository, NewsSyncState,
        NewsTranslation, TranslationStatus,
    },
    news_steam::detect_source_language,
    settings::{self, DatabaseError, DatabaseState},
};
use futures_util::future::BoxFuture;
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use thiserror::Error;

pub const DEFAULT_TARGET_LANGUAGE: &str = "es-419";
pub const GOOGLE_PUBLIC_TRANSLATION_PROVIDER_ID: &str = "google-public";
pub const GOOGLE_PUBLIC_TRANSLATION_PROVIDER_VERSION: &str = "public-v3-mymemory-fallback";
pub const GOOGLE_TRANSLATION_PROVIDER_ID: &str = "google-cloud-translation";
pub const GOOGLE_TRANSLATION_PROVIDER_VERSION: &str = "v2-basic-nmt-content";
const GOOGLE_PUBLIC_ENDPOINT: &str = "https://translate.googleapis.com/translate_a/single";
const MYMEMORY_ENDPOINT: &str = "https://api.mymemory.translated.net/get";
const PUBLIC_MAX_RESPONSE_BYTES: usize = 512 * 1024;
const PUBLIC_MAX_FRAGMENT_CHARACTERS: usize = 450;
const PUBLIC_MAX_BATCH_ITEMS: usize = 8;
const PUBLIC_MAX_BATCH_CHARACTERS: usize = 4_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationErrorKind {
    CredentialsMissing,
    AuthenticationFailed,
    QuotaExceeded,
    RateLimited,
    Timeout,
    ProviderUnavailable,
    InvalidResponse,
    UnsupportedLanguage,
    RequestTooLarge,
    PartialFailure,
    Unknown,
}

impl TranslationErrorKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CredentialsMissing => "credentials_missing",
            Self::AuthenticationFailed => "authentication_failed",
            Self::QuotaExceeded => "quota_exceeded",
            Self::RateLimited => "rate_limited",
            Self::Timeout => "timeout",
            Self::ProviderUnavailable => "provider_unavailable",
            Self::InvalidResponse => "invalid_response",
            Self::UnsupportedLanguage => "unsupported_language",
            Self::RequestTooLarge => "request_too_large",
            Self::PartialFailure => "partial_failure",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationCapabilities {
    pub provider_id: String,
    pub provider_version: String,
    pub supported_languages: Vec<String>,
    pub max_batch_items: usize,
    pub max_batch_characters: usize,
    pub supports_glossary: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationProviderDescriptor {
    pub provider_id: String,
    pub provider_version: String,
    pub display_name: String,
    pub available: bool,
    pub configured: bool,
    pub credentials_required: bool,
    pub credentials_configured: bool,
    pub official: bool,
    pub best_effort: bool,
    pub supports_glossary: bool,
    pub stability: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveTranslationProvider {
    pub active_provider_id: String,
    pub explicit_selection: bool,
    pub provider: TranslationProviderDescriptor,
}

#[derive(Debug, Clone)]
pub struct TranslationRequest {
    pub request_id: String,
    #[allow(dead_code)]
    pub news_item_id: String,
    pub source_language: String,
    pub target_language: String,
    pub title: String,
    pub summary: Option<String>,
    pub content: Option<String>,
    #[allow(dead_code)]
    pub glossary_version: Option<String>,
}

impl TranslationRequest {
    pub fn character_count(&self) -> usize {
        self.title.chars().count()
            + self
                .summary
                .as_deref()
                .map(|summary| summary.chars().count())
                .unwrap_or_default()
            + self
                .content
                .as_deref()
                .map(|content| content.chars().count())
                .unwrap_or_default()
    }
}

#[derive(Debug, Clone)]
pub struct TranslationResult {
    pub request_id: String,
    pub translated_title: Option<String>,
    pub translated_summary: Option<String>,
    pub translated_content: Option<String>,
    pub error: Option<TranslationErrorKind>,
}

#[derive(Debug, Clone, Default)]
pub struct TranslationBatchResult {
    pub results: Vec<TranslationResult>,
}

pub trait TranslationProvider: Send + Sync {
    fn capabilities(&self) -> TranslationCapabilities;
    fn translate_batch(
        &self,
        requests: Vec<TranslationRequest>,
    ) -> BoxFuture<'_, Result<TranslationBatchResult, TranslationProviderError>>;
}

#[derive(Debug, Clone)]
pub enum TranslationProviderInstance {
    GoogleCloud(GoogleTranslationProvider),
    GooglePublic(GooglePublicTranslationProvider),
}

impl TranslationProvider for TranslationProviderInstance {
    fn capabilities(&self) -> TranslationCapabilities {
        match self {
            Self::GoogleCloud(provider) => provider.capabilities(),
            Self::GooglePublic(provider) => provider.capabilities(),
        }
    }

    fn translate_batch(
        &self,
        requests: Vec<TranslationRequest>,
    ) -> BoxFuture<'_, Result<TranslationBatchResult, TranslationProviderError>> {
        match self {
            Self::GoogleCloud(provider) => provider.translate_batch(requests),
            Self::GooglePublic(provider) => provider.translate_batch(requests),
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TranslationProviderError {
    #[error("translation credentials are missing")]
    CredentialsMissing,
    #[error("translation authentication failed")]
    AuthenticationFailed,
    #[allow(dead_code)]
    #[error("translation quota exceeded")]
    QuotaExceeded,
    #[error("translation provider rate limited the request")]
    RateLimited,
    #[error("translation request timed out")]
    Timeout,
    #[error("translation provider is unavailable")]
    ProviderUnavailable,
    #[error("translation provider returned an invalid response")]
    InvalidResponse,
    #[error("translation language is unsupported")]
    UnsupportedLanguage,
    #[error("translation request is too large")]
    RequestTooLarge,
    #[error("translation provider returned an unknown error")]
    Unknown,
}

impl TranslationProviderError {
    fn kind(&self) -> TranslationErrorKind {
        match self {
            Self::CredentialsMissing => TranslationErrorKind::CredentialsMissing,
            Self::AuthenticationFailed => TranslationErrorKind::AuthenticationFailed,
            Self::QuotaExceeded => TranslationErrorKind::QuotaExceeded,
            Self::RateLimited => TranslationErrorKind::RateLimited,
            Self::Timeout => TranslationErrorKind::Timeout,
            Self::ProviderUnavailable => TranslationErrorKind::ProviderUnavailable,
            Self::InvalidResponse => TranslationErrorKind::InvalidResponse,
            Self::UnsupportedLanguage => TranslationErrorKind::UnsupportedLanguage,
            Self::RequestTooLarge => TranslationErrorKind::RequestTooLarge,
            Self::Unknown => TranslationErrorKind::Unknown,
        }
    }
}

#[derive(Debug, Clone)]
pub struct GoogleTranslationProvider {
    api_key: String,
    client: reqwest::Client,
    endpoint: String,
}

#[derive(Debug, Clone, Copy)]
enum TranslationField {
    Title,
    Summary,
    Content,
}

impl GoogleTranslationProvider {
    pub fn new(api_key: String) -> Result<Self, TranslationProviderError> {
        if api_key.trim().is_empty() {
            return Err(TranslationProviderError::CredentialsMissing);
        }
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(20))
            .user_agent("LumaDeck/0.1 news-translation")
            .build()
            .map_err(|_| TranslationProviderError::ProviderUnavailable)?;
        Ok(Self {
            api_key,
            client,
            endpoint: "https://translation.googleapis.com/language/translate/v2".to_string(),
        })
    }

    #[cfg(test)]
    fn with_endpoint(api_key: &str, endpoint: &str) -> Self {
        Self {
            api_key: api_key.to_string(),
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(1))
                .timeout(Duration::from_secs(2))
                .build()
                .expect("test HTTP client"),
            endpoint: endpoint.to_string(),
        }
    }

    fn translate_request(
        &self,
        requests: Vec<TranslationRequest>,
    ) -> BoxFuture<'_, Result<TranslationBatchResult, TranslationProviderError>> {
        Box::pin(async move {
            if requests.is_empty() {
                return Ok(TranslationBatchResult::default());
            }
            if requests.len() > 64
                || requests
                    .iter()
                    .map(TranslationRequest::character_count)
                    .sum::<usize>()
                    > 30_000
            {
                return Err(TranslationProviderError::RequestTooLarge);
            }
            if requests.iter().any(|request| {
                request.source_language != "en"
                    || !matches!(request.target_language.as_str(), "es-419" | "es")
            }) {
                return Err(TranslationProviderError::UnsupportedLanguage);
            }

            let mut fields = Vec::new();
            let mut field_map = Vec::new();
            for request in &requests {
                fields.push(request.title.clone());
                field_map.push((request.request_id.clone(), TranslationField::Title));
                if let Some(summary) = &request.summary {
                    fields.push(summary.clone());
                    field_map.push((request.request_id.clone(), TranslationField::Summary));
                }
                if let Some(content) = &request.content {
                    fields.push(content.clone());
                    field_map.push((request.request_id.clone(), TranslationField::Content));
                }
            }
            if fields.len() > 128 {
                return Err(TranslationProviderError::RequestTooLarge);
            }

            let body = json!({
                "q": fields,
                "source": "en",
                "target": requests[0].target_language,
                "format": "text"
            });
            let response = self
                .client
                .post(&self.endpoint)
                .query(&[("key", self.api_key.as_str())])
                .json(&body)
                .send()
                .await
                .map_err(|error| {
                    if error.is_timeout() {
                        TranslationProviderError::Timeout
                    } else {
                        TranslationProviderError::ProviderUnavailable
                    }
                })?;
            let status = response.status();
            if !status.is_success() {
                return Err(match status {
                    StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
                        TranslationProviderError::AuthenticationFailed
                    }
                    StatusCode::TOO_MANY_REQUESTS => TranslationProviderError::RateLimited,
                    StatusCode::REQUEST_TIMEOUT => TranslationProviderError::Timeout,
                    status if status.is_server_error() => {
                        TranslationProviderError::ProviderUnavailable
                    }
                    _ => TranslationProviderError::Unknown,
                });
            }
            let payload = response
                .json::<GoogleTranslateResponse>()
                .await
                .map_err(|_| TranslationProviderError::InvalidResponse)?;
            let translations = payload
                .data
                .translations
                .ok_or(TranslationProviderError::InvalidResponse)?;
            if translations.len() != field_map.len() {
                return Err(TranslationProviderError::InvalidResponse);
            }

            let mut results = requests
                .iter()
                .map(|request| TranslationResult {
                    request_id: request.request_id.clone(),
                    translated_title: None,
                    translated_summary: None,
                    translated_content: None,
                    error: None,
                })
                .collect::<Vec<_>>();
            for ((request_id, is_title), translation) in field_map.into_iter().zip(translations) {
                let result = results
                    .iter_mut()
                    .find(|result| result.request_id == request_id)
                    .ok_or(TranslationProviderError::InvalidResponse)?;
                match is_title {
                    TranslationField::Title => {
                        result.translated_title = Some(translation.translated_text)
                    }
                    TranslationField::Summary => {
                        result.translated_summary = Some(translation.translated_text)
                    }
                    TranslationField::Content => {
                        result.translated_content = Some(translation.translated_text)
                    }
                }
            }
            Ok(TranslationBatchResult { results })
        })
    }
}

impl TranslationProvider for GoogleTranslationProvider {
    fn capabilities(&self) -> TranslationCapabilities {
        TranslationCapabilities {
            provider_id: GOOGLE_TRANSLATION_PROVIDER_ID.to_string(),
            provider_version: GOOGLE_TRANSLATION_PROVIDER_VERSION.to_string(),
            supported_languages: vec!["en".to_string(), "es".to_string(), "es-419".to_string()],
            max_batch_items: 64,
            max_batch_characters: 30_000,
            supports_glossary: false,
        }
    }

    fn translate_batch(
        &self,
        requests: Vec<TranslationRequest>,
    ) -> BoxFuture<'_, Result<TranslationBatchResult, TranslationProviderError>> {
        self.translate_request(requests)
    }
}

#[derive(Debug, Clone)]
pub struct GooglePublicTranslationProvider {
    client: reqwest::Client,
    endpoint: String,
    fallback_endpoint: String,
    fallback_active: Arc<AtomicBool>,
}

impl GooglePublicTranslationProvider {
    pub fn new() -> Result<Self, TranslationProviderError> {
        let client = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(4))
            .timeout(Duration::from_secs(15))
            .user_agent("LumaDeck/0.1 news-translation")
            .build()
            .map_err(|_| TranslationProviderError::ProviderUnavailable)?;
        Ok(Self {
            client,
            endpoint: GOOGLE_PUBLIC_ENDPOINT.to_string(),
            fallback_endpoint: MYMEMORY_ENDPOINT.to_string(),
            fallback_active: Arc::new(AtomicBool::new(false)),
        })
    }

    #[cfg(test)]
    fn with_endpoint(endpoint: &str) -> Self {
        Self::with_endpoints(endpoint, "http://127.0.0.1:1")
    }

    #[cfg(test)]
    fn with_endpoints(endpoint: &str, fallback_endpoint: &str) -> Self {
        Self {
            client: reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(1))
                .timeout(Duration::from_secs(2))
                .build()
                .expect("test HTTP client"),
            endpoint: endpoint.to_string(),
            fallback_endpoint: fallback_endpoint.to_string(),
            fallback_active: Arc::new(AtomicBool::new(false)),
        }
    }

    async fn translate_text(
        &self,
        text: &str,
        source_language: &str,
        target_language: &str,
    ) -> Result<String, TranslationProviderError> {
        if text.trim().is_empty() {
            return Err(TranslationProviderError::InvalidResponse);
        }
        if self.fallback_active.load(Ordering::Relaxed) {
            return self
                .translate_mymemory_text(text, source_language, target_language)
                .await;
        }
        match self
            .translate_google_text(text, source_language, target_language)
            .await
        {
            Ok(translated) => Ok(translated),
            Err(
                error @ (TranslationProviderError::RateLimited
                | TranslationProviderError::InvalidResponse),
            ) => {
                self.fallback_active.store(true, Ordering::Relaxed);
                match self
                    .translate_mymemory_text(text, source_language, target_language)
                    .await
                {
                    Ok(translated) => Ok(translated),
                    Err(_) => Err(error),
                }
            }
            Err(error) => Err(error),
        }
    }

    async fn translate_google_text(
        &self,
        text: &str,
        source_language: &str,
        target_language: &str,
    ) -> Result<String, TranslationProviderError> {
        let source = if source_language == "en" {
            "en"
        } else {
            "auto"
        };
        let target = map_public_language(target_language)?;
        let mut attempts = 0;
        loop {
            let response = self
                .client
                .post(&self.endpoint)
                .form(&[
                    ("client", "gtx"),
                    ("sl", source),
                    ("tl", target),
                    ("dt", "t"),
                    ("q", text),
                ])
                .send()
                .await;
            let response = match response {
                Ok(response) => response,
                Err(error) if attempts == 0 && !error.is_timeout() => {
                    attempts += 1;
                    wait_before_retry(Duration::from_millis(100)).await;
                    continue;
                }
                Err(error) => {
                    return Err(if error.is_timeout() {
                        TranslationProviderError::Timeout
                    } else {
                        TranslationProviderError::ProviderUnavailable
                    });
                }
            };
            let status = response.status();
            if status == StatusCode::TOO_MANY_REQUESTS {
                return Err(TranslationProviderError::RateLimited);
            }
            if status == StatusCode::REQUEST_TIMEOUT {
                return Err(TranslationProviderError::Timeout);
            }
            if status.is_server_error() && attempts == 0 {
                attempts += 1;
                let delay = response
                    .headers()
                    .get("retry-after")
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse::<u64>().ok())
                    .map(|seconds| seconds.min(2) * 1_000)
                    .unwrap_or(100);
                wait_before_retry(Duration::from_millis(delay)).await;
                continue;
            }
            if status.is_server_error() {
                return Err(TranslationProviderError::ProviderUnavailable);
            }
            if !status.is_success() {
                return Err(TranslationProviderError::Unknown);
            }
            let body = read_limited_response(response, PUBLIC_MAX_RESPONSE_BYTES).await?;
            return parse_public_translation_response(&body);
        }
    }

    async fn translate_mymemory_text(
        &self,
        text: &str,
        source_language: &str,
        target_language: &str,
    ) -> Result<String, TranslationProviderError> {
        let source = if source_language == "en" {
            "en"
        } else {
            source_language
        };
        let target = map_public_language(target_language)?;
        let langpair = format!("{source}|{target}");
        let response = self
            .client
            .get(&self.fallback_endpoint)
            .query(&[("q", text), ("langpair", langpair.as_str())])
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    TranslationProviderError::Timeout
                } else {
                    TranslationProviderError::ProviderUnavailable
                }
            })?;
        if !response.status().is_success() {
            return Err(match response.status() {
                StatusCode::TOO_MANY_REQUESTS => TranslationProviderError::RateLimited,
                StatusCode::REQUEST_TIMEOUT => TranslationProviderError::Timeout,
                status if status.is_server_error() => TranslationProviderError::ProviderUnavailable,
                _ => TranslationProviderError::Unknown,
            });
        }
        let body = read_limited_response(response, PUBLIC_MAX_RESPONSE_BYTES).await?;
        parse_mymemory_translation_response(&body)
    }

    async fn translate_fragmented(
        &self,
        text: &str,
        source_language: &str,
        target_language: &str,
    ) -> Result<String, TranslationProviderError> {
        let fragments = split_translation_text(text, PUBLIC_MAX_FRAGMENT_CHARACTERS);
        let mut translated = String::new();
        for fragment in fragments {
            translated.push_str(
                &self
                    .translate_text(&fragment, source_language, target_language)
                    .await?,
            );
        }
        if translated.trim().is_empty() {
            return Err(TranslationProviderError::InvalidResponse);
        }
        Ok(translated)
    }
}

impl TranslationProvider for GooglePublicTranslationProvider {
    fn capabilities(&self) -> TranslationCapabilities {
        TranslationCapabilities {
            provider_id: GOOGLE_PUBLIC_TRANSLATION_PROVIDER_ID.to_string(),
            provider_version: GOOGLE_PUBLIC_TRANSLATION_PROVIDER_VERSION.to_string(),
            supported_languages: vec!["auto".to_string(), "en".to_string(), "es-419".to_string()],
            max_batch_items: PUBLIC_MAX_BATCH_ITEMS,
            max_batch_characters: PUBLIC_MAX_BATCH_CHARACTERS,
            supports_glossary: false,
        }
    }

    fn translate_batch(
        &self,
        requests: Vec<TranslationRequest>,
    ) -> BoxFuture<'_, Result<TranslationBatchResult, TranslationProviderError>> {
        Box::pin(async move {
            if requests.len() > PUBLIC_MAX_BATCH_ITEMS
                || requests
                    .iter()
                    .map(TranslationRequest::character_count)
                    .sum::<usize>()
                    > PUBLIC_MAX_BATCH_CHARACTERS
            {
                return Err(TranslationProviderError::RequestTooLarge);
            }
            let mut results = Vec::with_capacity(requests.len());
            for request in requests {
                let translated_title = self
                    .translate_fragmented(
                        &request.title,
                        &request.source_language,
                        &request.target_language,
                    )
                    .await?;
                let translated_summary = match request.summary.as_deref() {
                    Some(summary) => Some(
                        self.translate_fragmented(
                            summary,
                            &request.source_language,
                            &request.target_language,
                        )
                        .await?,
                    ),
                    None => None,
                };
                let translated_content = match request.content.as_deref() {
                    Some(content) => Some(
                        self.translate_fragmented(
                            content,
                            &request.source_language,
                            &request.target_language,
                        )
                        .await?,
                    ),
                    None => None,
                };
                results.push(TranslationResult {
                    request_id: request.request_id,
                    translated_title: Some(translated_title),
                    translated_summary,
                    translated_content,
                    error: None,
                });
            }
            Ok(TranslationBatchResult { results })
        })
    }
}

#[derive(Debug, Error)]
pub enum TranslationServiceError {
    #[error("translation database operation failed")]
    Database(#[from] DatabaseError),
    #[error("translation request is invalid")]
    InvalidRequest,
    #[error("translation target language is unsupported")]
    UnsupportedLanguage,
    #[error("translation provider failed")]
    Provider(#[from] TranslationProviderError),
}

impl TranslationServiceError {
    pub fn kind(&self) -> TranslationErrorKind {
        match self {
            Self::Database(DatabaseError::AccountNotConfigured) => {
                TranslationErrorKind::CredentialsMissing
            }
            Self::Database(_) => TranslationErrorKind::Unknown,
            Self::InvalidRequest => TranslationErrorKind::Unknown,
            Self::UnsupportedLanguage => TranslationErrorKind::UnsupportedLanguage,
            Self::Provider(error) => error.kind(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranslationBatchSummary {
    pub target_language: String,
    pub requested_count: usize,
    pub cache_hit_count: usize,
    pub translated_count: usize,
    pub failed_count: usize,
    pub partial_failure: bool,
}

pub struct TranslationService<'a, P> {
    repository: NewsRepository<'a>,
    provider: P,
}

impl<'a> TranslationService<'a, TranslationProviderInstance> {
    pub fn from_settings(state: &'a DatabaseState) -> Result<Self, TranslationServiceError> {
        let selected_provider = settings::get_translation_provider_selection(state)?
            .unwrap_or_else(|| GOOGLE_PUBLIC_TRANSLATION_PROVIDER_ID.to_string());
        let provider = match selected_provider.as_str() {
            GOOGLE_PUBLIC_TRANSLATION_PROVIDER_ID => {
                TranslationProviderInstance::GooglePublic(GooglePublicTranslationProvider::new()?)
            }
            GOOGLE_TRANSLATION_PROVIDER_ID => {
                let api_key = settings::get_translation_api_key(state)?;
                TranslationProviderInstance::GoogleCloud(GoogleTranslationProvider::new(api_key)?)
            }
            _ => return Err(TranslationServiceError::InvalidRequest),
        };
        Ok(Self::new(state, provider))
    }
}

impl<'a, P> TranslationService<'a, P>
where
    P: TranslationProvider,
{
    pub fn new(state: &'a DatabaseState, provider: P) -> Self {
        Self {
            repository: NewsRepository::new(state),
            provider,
        }
    }

    pub async fn translate_news_items(
        &self,
        news_item_ids: &[String],
        target_language: &str,
        force_retranslate: bool,
        glossary_version: Option<String>,
    ) -> Result<TranslationBatchSummary, TranslationServiceError> {
        let target_language = normalize_target_language(target_language)?;
        if news_item_ids.is_empty() {
            return Ok(TranslationBatchSummary {
                target_language,
                requested_count: 0,
                cache_hit_count: 0,
                translated_count: 0,
                failed_count: 0,
                partial_failure: false,
            });
        }
        let capabilities = self.provider.capabilities();
        if capabilities.max_batch_items == 0 || capabilities.max_batch_characters == 0 {
            return Err(TranslationServiceError::InvalidRequest);
        }

        let mut requests = Vec::new();
        let mut cache_hit_count = 0;
        let mut failed_count = 0;
        for id in news_item_ids {
            let stored_item = self
                .repository
                .get_news_item_by_id(id)?
                .ok_or(TranslationServiceError::InvalidRequest)?;
            let detected_source_language = detect_source_language(
                &stored_item.original_title,
                stored_item.original_content.as_deref(),
                &stored_item.source_language,
            );
            let mut item = stored_item;
            if item.source_language != detected_source_language {
                item.source_language = detected_source_language;
                item.refresh_source_content_hash();
                self.repository
                    .upsert_news_items(std::slice::from_ref(&item))?;
            }
            if !force_retranslate
                && self
                    .repository
                    .get_reusable_translation_for_provider(
                        &item.id,
                        &target_language,
                        &item.source_content_hash,
                        &capabilities.provider_id,
                        &capabilities.provider_version,
                        glossary_version.as_deref(),
                    )?
                    .is_some()
            {
                cache_hit_count += 1;
                continue;
            }
            if item.source_language != "en"
                && capabilities.provider_id != GOOGLE_PUBLIC_TRANSLATION_PROVIDER_ID
            {
                self.persist_failure(
                    &item,
                    &target_language,
                    glossary_version.clone(),
                    TranslationErrorKind::UnsupportedLanguage,
                )?;
                failed_count += 1;
                continue;
            }
            let request = TranslationRequest {
                request_id: format!("translation-request-{}", item.id),
                news_item_id: item.id.clone(),
                source_language: item.source_language.clone(),
                target_language: target_language.clone(),
                title: item.original_title.clone(),
                summary: item.original_summary.clone(),
                content: item
                    .original_content
                    .as_deref()
                    .map(|content| plain_translation_text(content, item.content_format)),
                glossary_version: glossary_version.clone(),
            };
            if request.character_count() > capabilities.max_batch_characters {
                self.persist_failure(
                    &item,
                    &target_language,
                    glossary_version.clone(),
                    TranslationErrorKind::RequestTooLarge,
                )?;
                failed_count += 1;
                continue;
            }
            self.persist_status(
                &item,
                &target_language,
                glossary_version.clone(),
                TranslationStatus::Translating,
                None,
                None,
                None,
                None,
            )?;
            requests.push((item, request));
        }

        let mut translated_count = 0;
        for chunk in request_chunks(
            requests,
            capabilities.max_batch_items,
            capabilities.max_batch_characters,
        ) {
            let provider_requests = chunk
                .iter()
                .map(|(_, request)| request.clone())
                .collect::<Vec<_>>();
            let response = match self.provider.translate_batch(provider_requests).await {
                Ok(response) => response,
                Err(error) => {
                    for (item, _) in &chunk {
                        self.persist_failure(
                            item,
                            &target_language,
                            glossary_version.clone(),
                            error.kind(),
                        )?;
                        failed_count += 1;
                    }
                    continue;
                }
            };
            for (item, request) in chunk {
                let result = response
                    .results
                    .iter()
                    .find(|result| result.request_id == request.request_id);
                match result {
                    Some(result) if result.error.is_none() && result.translated_title.is_some() => {
                        self.persist_status(
                            &item,
                            &target_language,
                            glossary_version.clone(),
                            TranslationStatus::Translated,
                            result.translated_title.clone(),
                            result.translated_summary.clone(),
                            result.translated_content.clone(),
                            None,
                        )?;
                        translated_count += 1;
                    }
                    Some(result) => {
                        self.persist_failure_with_values(
                            &item,
                            &target_language,
                            glossary_version.clone(),
                            result.translated_title.clone(),
                            result.translated_summary.clone(),
                            result.translated_content.clone(),
                            result
                                .error
                                .clone()
                                .unwrap_or(TranslationErrorKind::PartialFailure),
                        )?;
                        failed_count += 1;
                    }
                    None => {
                        self.persist_failure(
                            &item,
                            &target_language,
                            glossary_version.clone(),
                            TranslationErrorKind::InvalidResponse,
                        )?;
                        failed_count += 1;
                    }
                }
            }
        }

        Ok(TranslationBatchSummary {
            target_language,
            requested_count: news_item_ids.len(),
            cache_hit_count,
            translated_count,
            failed_count,
            partial_failure: failed_count > 0 && (translated_count > 0 || cache_hit_count > 0),
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn persist_status(
        &self,
        item: &NewsItem,
        target_language: &str,
        glossary_version: Option<String>,
        status: TranslationStatus,
        translated_title: Option<String>,
        translated_summary: Option<String>,
        translated_content: Option<String>,
        error_code: Option<TranslationErrorKind>,
    ) -> Result<(), TranslationServiceError> {
        let now = timestamp();
        let translated_content_hash = (status == TranslationStatus::Translated).then(|| {
            translated_content_hash(
                translated_title.as_deref(),
                translated_summary.as_deref(),
                translated_content.as_deref(),
            )
        });
        self.repository.save_translation(&NewsTranslation {
            id: String::new(),
            news_item_id: item.id.clone(),
            source_language: item.source_language.clone(),
            target_language: target_language.to_string(),
            translated_title,
            translated_summary,
            translated_content,
            status,
            provider_id: self.provider.capabilities().provider_id,
            provider_version: Some(self.provider.capabilities().provider_version),
            glossary_version,
            source_content_hash: item.source_content_hash.clone(),
            translated_content_hash,
            translated_at: (status == TranslationStatus::Translated).then_some(now.clone()),
            last_attempt_at: Some(now.clone()),
            error_code: error_code.map(|error| error.as_str().to_string()),
            created_at: String::new(),
            updated_at: now,
        })?;
        Ok(())
    }

    fn persist_failure(
        &self,
        item: &NewsItem,
        target_language: &str,
        glossary_version: Option<String>,
        error: TranslationErrorKind,
    ) -> Result<(), TranslationServiceError> {
        self.persist_failure_with_values(
            item,
            target_language,
            glossary_version,
            None,
            None,
            None,
            error,
        )
    }

    fn persist_failure_with_values(
        &self,
        item: &NewsItem,
        target_language: &str,
        glossary_version: Option<String>,
        translated_title: Option<String>,
        translated_summary: Option<String>,
        translated_content: Option<String>,
        error: TranslationErrorKind,
    ) -> Result<(), TranslationServiceError> {
        self.persist_status(
            item,
            target_language,
            glossary_version,
            TranslationStatus::Failed,
            translated_title,
            translated_summary,
            translated_content,
            Some(error),
        )
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsDisplayItem {
    pub news_item_id: String,
    pub category: NewsCategory,
    pub source_url: String,
    pub published_at: String,
    pub display_title: String,
    pub display_summary: Option<String>,
    pub display_content: Option<String>,
    pub display_language: String,
    pub original_title: String,
    pub original_summary: Option<String>,
    pub original_content: Option<String>,
    pub content_format: NewsContentFormat,
    pub image_url: Option<String>,
    pub thumbnail_url: Option<String>,
    pub image_source: Option<String>,
    pub comment_count: Option<u32>,
    pub translation_status: Option<TranslationStatus>,
    pub has_translation: bool,
    pub source_language: String,
    pub target_language: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsFeedViewModel {
    pub game_id: String,
    pub sync_state: Option<NewsSyncState>,
    pub is_stale: bool,
    pub total_count: usize,
    pub active_provider: String,
    pub target_language: Option<String>,
    pub hero: Option<NewsDisplayItem>,
    pub items: Vec<NewsDisplayItem>,
    pub secondary_items: Vec<NewsDisplayItem>,
    pub available_categories: Vec<NewsCategory>,
    pub warnings: Vec<String>,
}

pub fn get_news_display_feed(
    state: &DatabaseState,
    game_id: &str,
    categories: Vec<crate::news::NewsCategory>,
    limit: Option<u32>,
    target_language: Option<String>,
) -> Result<Vec<NewsDisplayItem>, DatabaseError> {
    let target_language = target_language
        .as_deref()
        .map(normalize_target_language)
        .transpose()
        .map_err(|_| rusqlite::Error::InvalidQuery)?;
    let active_provider = get_active_translation_provider(state)?;
    let entries = NewsRepository::new(state).get_news_feed(&crate::news::NewsFeedQuery {
        game_id: Some(game_id.to_string()),
        categories,
        limit: limit.unwrap_or(50) as usize,
        target_language,
        translation_provider_id: Some(active_provider.provider.provider_id),
        translation_provider_version: Some(active_provider.provider.provider_version),
        translation_glossary_version: None,
    })?;
    Ok(entries.into_iter().map(display_item).collect())
}

pub fn get_news_feed_view_model(
    state: &DatabaseState,
    game_id: &str,
    categories: Vec<NewsCategory>,
    limit: Option<u32>,
    target_language: Option<String>,
) -> Result<NewsFeedViewModel, DatabaseError> {
    let target_language = target_language.or_else(|| Some(DEFAULT_TARGET_LANGUAGE.to_string()));
    let active_provider = get_active_translation_provider(state)?;
    let mut entries = get_news_display_feed(
        state,
        game_id,
        categories,
        Some(limit.unwrap_or(5).clamp(1, 50)),
        target_language.clone(),
    )?;
    let hero = entries.first().cloned();
    if hero.is_some() {
        entries.remove(0);
    }
    let secondary_items = if entries.len() > 4 {
        entries.split_off(4)
    } else {
        Vec::new()
    };
    let sync_state = NewsRepository::new(state).get_sync_state("steam", game_id)?;
    let is_stale = sync_state.as_ref().is_some_and(|value| value.is_stale);
    let mut warnings = Vec::new();
    if sync_state
        .as_ref()
        .and_then(|value| value.last_error_code.as_deref())
        .is_some()
    {
        warnings.push("NEWS_SYNC_WARNING".to_string());
    }
    Ok(NewsFeedViewModel {
        game_id: game_id.to_string(),
        sync_state,
        is_stale,
        total_count: entries.len() + secondary_items.len() + usize::from(hero.is_some()),
        active_provider: active_provider.active_provider_id,
        target_language,
        hero,
        items: entries,
        secondary_items,
        available_categories: vec![
            NewsCategory::Official,
            NewsCategory::Update,
            NewsCategory::Event,
            NewsCategory::Dlc,
            NewsCategory::Maintenance,
            NewsCategory::Community,
        ],
        warnings,
    })
}

pub fn get_translation_providers(
    state: &DatabaseState,
) -> Result<Vec<TranslationProviderDescriptor>, DatabaseError> {
    let cloud = settings::get_translation_configuration(state)?;
    Ok(vec![
        TranslationProviderDescriptor {
            provider_id: GOOGLE_PUBLIC_TRANSLATION_PROVIDER_ID.to_string(),
            provider_version: GOOGLE_PUBLIC_TRANSLATION_PROVIDER_VERSION.to_string(),
            display_name: "Public Translation (Google + MyMemory)".to_string(),
            available: true,
            configured: true,
            credentials_required: false,
            credentials_configured: false,
            official: false,
            best_effort: true,
            supports_glossary: false,
            stability: "experimental".to_string(),
        },
        TranslationProviderDescriptor {
            provider_id: GOOGLE_TRANSLATION_PROVIDER_ID.to_string(),
            provider_version: GOOGLE_TRANSLATION_PROVIDER_VERSION.to_string(),
            display_name: "Google Cloud Translation".to_string(),
            available: true,
            configured: cloud.api_key_configured && cloud.credential_available,
            credentials_required: true,
            credentials_configured: cloud.api_key_configured && cloud.credential_available,
            official: true,
            best_effort: false,
            supports_glossary: false,
            stability: "official".to_string(),
        },
    ])
}

pub fn get_active_translation_provider(
    state: &DatabaseState,
) -> Result<ActiveTranslationProvider, DatabaseError> {
    let selection = settings::get_translation_provider_selection(state)?;
    let active_provider_id = selection
        .clone()
        .unwrap_or_else(|| GOOGLE_PUBLIC_TRANSLATION_PROVIDER_ID.to_string());
    let provider = get_translation_providers(state)?
        .into_iter()
        .find(|provider| provider.provider_id == active_provider_id)
        .ok_or(DatabaseError::UnsupportedProvider)?;
    Ok(ActiveTranslationProvider {
        active_provider_id,
        explicit_selection: selection.is_some(),
        provider,
    })
}

pub fn set_active_translation_provider(
    state: &DatabaseState,
    provider_id: &str,
) -> Result<ActiveTranslationProvider, DatabaseError> {
    settings::set_translation_provider_selection(state, provider_id)?;
    get_active_translation_provider(state)
}

fn display_item(entry: NewsFeedItem) -> NewsDisplayItem {
    let image_url = metadata_string(&entry.item, "imageUrl");
    let image_source = metadata_string(&entry.item, "imageSource");
    let translation_status = entry
        .translation
        .as_ref()
        .map(|translation| translation.status);
    let translation_target = entry
        .translation
        .as_ref()
        .map(|translation| translation.target_language.clone());
    let valid_translation = entry.translation.filter(|translation| {
        translation.status == TranslationStatus::Translated
            && translation.source_content_hash == entry.item.source_content_hash
            && translation.translated_title.is_some()
            && (entry.item.original_content.is_none() || translation.translated_content.is_some())
    });
    let has_translation = valid_translation.is_some();
    let (display_title, display_summary, display_content, display_language, status, target) =
        if let Some(translation) = valid_translation {
            (
                translation
                    .translated_title
                    .unwrap_or_else(|| entry.item.original_title.clone()),
                translation.translated_summary,
                translation.translated_content,
                translation.target_language.clone(),
                Some(TranslationStatus::Translated),
                Some(translation.target_language),
            )
        } else {
            (
                entry.item.original_title.clone(),
                entry.item.original_summary.clone(),
                None,
                entry.item.source_language.clone(),
                translation_status,
                translation_target,
            )
        };
    NewsDisplayItem {
        news_item_id: entry.item.id,
        category: entry.item.category,
        source_url: entry.item.source_url,
        published_at: entry.item.published_at,
        display_title,
        display_summary,
        display_content,
        display_language,
        original_title: entry.item.original_title,
        original_summary: entry.item.original_summary,
        original_content: entry.item.original_content,
        content_format: entry.item.content_format,
        image_url,
        thumbnail_url: None,
        image_source,
        comment_count: None,
        translation_status: status,
        has_translation,
        source_language: entry.item.source_language,
        target_language: target,
    }
}

fn metadata_string(item: &NewsItem, key: &str) -> Option<String> {
    item.provider_metadata
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|metadata| metadata.get(key))
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .filter(|value| !value.trim().is_empty())
}

fn normalize_target_language(language: &str) -> Result<String, TranslationServiceError> {
    let normalized = language.trim().to_ascii_lowercase();
    matches!(normalized.as_str(), "es-419" | "es")
        .then_some(normalized)
        .ok_or(TranslationServiceError::UnsupportedLanguage)
}

fn request_chunks(
    requests: Vec<(NewsItem, TranslationRequest)>,
    max_items: usize,
    max_characters: usize,
) -> Vec<Vec<(NewsItem, TranslationRequest)>> {
    let mut chunks = Vec::new();
    let mut current = Vec::new();
    let mut current_characters = 0;
    for request in requests {
        let characters = request.1.character_count();
        if !current.is_empty()
            && (current.len() >= max_items || current_characters + characters > max_characters)
        {
            chunks.push(std::mem::take(&mut current));
            current_characters = 0;
        }
        current_characters += characters;
        current.push(request);
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

fn timestamp() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

#[derive(Debug, Deserialize)]
struct GoogleTranslateResponse {
    data: GoogleTranslateData,
}

#[derive(Debug, Deserialize)]
struct GoogleTranslateData {
    translations: Option<Vec<GoogleTranslation>>,
}

#[derive(Debug, Deserialize)]
struct GoogleTranslation {
    #[serde(rename = "translatedText")]
    translated_text: String,
}

fn map_public_language(language: &str) -> Result<&'static str, TranslationProviderError> {
    match language.trim().to_ascii_lowercase().as_str() {
        "es-419" | "es" => Ok("es"),
        _ => Err(TranslationProviderError::UnsupportedLanguage),
    }
}

async fn read_limited_response(
    response: reqwest::Response,
    max_bytes: usize,
) -> Result<Vec<u8>, TranslationProviderError> {
    if response
        .content_length()
        .is_some_and(|length| length as usize > max_bytes)
    {
        return Err(TranslationProviderError::InvalidResponse);
    }
    let body = response.bytes().await.map_err(|error| {
        if error.is_timeout() {
            TranslationProviderError::Timeout
        } else {
            TranslationProviderError::ProviderUnavailable
        }
    })?;
    if body.len() > max_bytes {
        return Err(TranslationProviderError::InvalidResponse);
    }
    Ok(body.to_vec())
}

async fn wait_before_retry(delay: Duration) {
    let _ = tauri::async_runtime::spawn_blocking(move || std::thread::sleep(delay)).await;
}

fn parse_public_translation_response(body: &[u8]) -> Result<String, TranslationProviderError> {
    let text = std::str::from_utf8(body).map_err(|_| TranslationProviderError::InvalidResponse)?;
    let normalized = text.trim().to_ascii_lowercase();
    if normalized.is_empty()
        || normalized.contains("<html")
        || normalized.contains("<!doctype")
        || normalized.contains("captcha")
        || normalized.contains("unusual traffic")
    {
        return Err(TranslationProviderError::InvalidResponse);
    }
    let value: Value =
        serde_json::from_str(text).map_err(|_| TranslationProviderError::InvalidResponse)?;
    let segments = value
        .as_array()
        .and_then(|root| root.first())
        .and_then(Value::as_array)
        .ok_or(TranslationProviderError::InvalidResponse)?;
    let mut translated = String::new();
    for segment in segments {
        let Some(value) = segment
            .as_array()
            .and_then(|segment| segment.first())
            .and_then(Value::as_str)
        else {
            continue;
        };
        translated.push_str(value);
    }
    if translated.trim().is_empty() {
        return Err(TranslationProviderError::InvalidResponse);
    }
    Ok(translated)
}

#[derive(Debug, Deserialize)]
struct MyMemoryTranslationResponse {
    #[serde(rename = "responseData")]
    response_data: Option<MyMemoryResponseData>,
    #[serde(rename = "responseStatus")]
    response_status: Option<u16>,
    #[serde(rename = "quotaFinished", default)]
    quota_finished: bool,
}

#[derive(Debug, Deserialize)]
struct MyMemoryResponseData {
    #[serde(rename = "translatedText")]
    translated_text: String,
}

fn parse_mymemory_translation_response(body: &[u8]) -> Result<String, TranslationProviderError> {
    let response = serde_json::from_slice::<MyMemoryTranslationResponse>(body)
        .map_err(|_| TranslationProviderError::InvalidResponse)?;
    if response.quota_finished || response.response_status != Some(200) {
        return Err(TranslationProviderError::RateLimited);
    }
    let translated = response
        .response_data
        .ok_or(TranslationProviderError::InvalidResponse)?
        .translated_text;
    if translated.trim().is_empty() {
        return Err(TranslationProviderError::InvalidResponse);
    }
    Ok(translated)
}

fn split_translation_text(text: &str, max_characters: usize) -> Vec<String> {
    if text.chars().count() <= max_characters {
        return vec![text.to_string()];
    }
    let mut fragments = Vec::new();
    let mut start = 0;
    while start < text.len() {
        let remaining = &text[start..];
        if remaining.chars().count() <= max_characters {
            fragments.push(remaining.to_string());
            break;
        }
        let hard_end = remaining
            .char_indices()
            .nth(max_characters)
            .map(|(index, _)| index)
            .unwrap_or(remaining.len());
        let candidate = &remaining[..hard_end];
        let paragraph_end = candidate.rfind("\n\n").map(|index| index + 2);
        let sentence_end = [". ", "? ", "! ", ".\n", "?\n", "!\n"]
            .iter()
            .filter_map(|separator| {
                candidate
                    .rfind(separator)
                    .map(|index| index + separator.len())
            })
            .max();
        let end = paragraph_end.or(sentence_end).unwrap_or(hard_end);
        let end = if end == 0 { hard_end } else { end };
        fragments.push(remaining[..end].to_string());
        start += end;
    }
    fragments
}

fn plain_translation_text(content: &str, format: NewsContentFormat) -> String {
    if format != NewsContentFormat::Html {
        return content.to_string();
    }
    let mut text = String::with_capacity(content.len());
    let mut in_tag = false;
    for character in content.chars() {
        match character {
            '<' => in_tag = true,
            '>' if in_tag => {
                in_tag = false;
                text.push(' ');
            }
            _ if !in_tag => text.push(character),
            _ => {}
        }
    }
    text.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn translated_content_hash(
    title: Option<&str>,
    summary: Option<&str>,
    content: Option<&str>,
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(title.unwrap_or_default().as_bytes());
    hasher.update([0x1f]);
    hasher.update(summary.unwrap_or_default().as_bytes());
    hasher.update([0x1f]);
    hasher.update(content.unwrap_or_default().as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{data_directory::DataDirectoryResolver, news::NewsCategory};
    use std::{
        fs,
        path::PathBuf,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
    };

    fn root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "lumadeck-translation-{name}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }

    fn item(id: &str, title: &str, summary: Option<&str>) -> NewsItem {
        let mut item = NewsItem {
            id: format!("news-{id}"),
            provider_id: "test".to_string(),
            external_id: id.to_string(),
            game_id: "game-001".to_string(),
            external_game_id: None,
            category: NewsCategory::Official,
            source_url: format!("https://example.test/{id}"),
            canonical_url: Some(format!("https://example.test/{id}")),
            published_at: "1".to_string(),
            updated_at: None,
            first_seen_at: "1".to_string(),
            source_language: "en".to_string(),
            original_title: title.to_string(),
            original_summary: summary.map(str::to_string),
            original_content: Some("must never be translated".to_string()),
            content_format: crate::news::NewsContentFormat::PlainText,
            source_content_hash: String::new(),
            provider_metadata: None,
            created_at: "1".to_string(),
            persisted_updated_at: "1".to_string(),
        };
        item.refresh_source_content_hash();
        item
    }

    struct FakeProvider {
        calls: Arc<AtomicUsize>,
        fail: Option<TranslationErrorKind>,
    }

    impl TranslationProvider for FakeProvider {
        fn capabilities(&self) -> TranslationCapabilities {
            TranslationCapabilities {
                provider_id: GOOGLE_PUBLIC_TRANSLATION_PROVIDER_ID.to_string(),
                provider_version: GOOGLE_PUBLIC_TRANSLATION_PROVIDER_VERSION.to_string(),
                supported_languages: vec!["en".to_string(), "es-419".to_string()],
                max_batch_items: 2,
                max_batch_characters: 10_000,
                supports_glossary: false,
            }
        }

        fn translate_batch(
            &self,
            requests: Vec<TranslationRequest>,
        ) -> BoxFuture<'_, Result<TranslationBatchResult, TranslationProviderError>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let fail = self.fail.clone();
            Box::pin(async move {
                if let Some(error) = fail {
                    return Err(match error {
                        TranslationErrorKind::RateLimited => TranslationProviderError::RateLimited,
                        _ => TranslationProviderError::Unknown,
                    });
                }
                Ok(TranslationBatchResult {
                    results: requests
                        .into_iter()
                        .rev()
                        .map(|request| TranslationResult {
                            request_id: request.request_id,
                            translated_title: Some(format!("ES {}", request.title)),
                            translated_summary: request
                                .summary
                                .map(|summary| format!("ES {summary}")),
                            translated_content: request
                                .content
                                .map(|content| format!("ES {content}")),
                            error: None,
                        })
                        .collect(),
                })
            })
        }
    }

    fn with_database<T>(test: impl FnOnce(&DatabaseState, &NewsRepository<'_>) -> T) -> T {
        let path = root("db");
        let state = DatabaseState::open(DataDirectoryResolver::for_app_data(&path)).expect("db");
        state.connection.lock().expect("connection").execute(
            "INSERT INTO games(id,title,sort_title,provider,platform,created_at,updated_at) VALUES ('game-001','Game','game','test','pc','1','1')",
            [],
        ).expect("game");
        let repository = NewsRepository::new(&state);
        let result = test(&state, &repository);
        drop(state);
        let _ = fs::remove_dir_all(path);
        result
    }

    #[test]
    fn exact_es_419_and_title_only_are_supported() {
        assert_eq!(normalize_target_language("es-419").unwrap(), "es-419");
        let request = TranslationRequest {
            request_id: "1".to_string(),
            news_item_id: "news-1".to_string(),
            source_language: "en".to_string(),
            target_language: "es-419".to_string(),
            title: "Title".to_string(),
            summary: None,
            content: None,
            glossary_version: None,
        };
        assert_eq!(request.character_count(), 5);
    }

    #[test]
    fn service_reuses_cache_and_preserves_original_fallback() {
        with_database(|state, repository| {
            repository
                .upsert_news_items(&[item("1", "Title", Some("Summary"))])
                .expect("item");
            let calls = Arc::new(AtomicUsize::new(0));
            let service = TranslationService::new(
                state,
                FakeProvider {
                    calls: calls.clone(),
                    fail: None,
                },
            );
            let ids = vec!["news-1".to_string()];
            tauri::async_runtime::block_on(
                service.translate_news_items(&ids, "es-419", false, None),
            )
            .expect("translate");
            let second = tauri::async_runtime::block_on(
                service.translate_news_items(&ids, "es-419", false, None),
            )
            .expect("cache");
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            assert_eq!(second.cache_hit_count, 1);
            let display = get_news_display_feed(
                state,
                "game-001",
                Vec::new(),
                Some(10),
                Some("es-419".to_string()),
            )
            .expect("display");
            assert_eq!(display[0].display_title, "ES Title");
            assert_eq!(display[0].display_summary.as_deref(), Some("ES Summary"));
            assert_eq!(
                display[0].display_content.as_deref(),
                Some("ES must never be translated")
            );
            assert!(display[0].has_translation);
        });
    }

    #[test]
    fn translation_keeps_missing_summary_empty_and_translates_content() {
        with_database(|state, repository| {
            repository
                .upsert_news_items(&[item("1", "Title", None)])
                .expect("item");
            let calls = Arc::new(AtomicUsize::new(0));
            let service = TranslationService::new(state, FakeProvider { calls, fail: None });
            tauri::async_runtime::block_on(service.translate_news_items(
                &["news-1".to_string()],
                "es-419",
                false,
                None,
            ))
            .expect("translate");
            let translation = repository
                .get_translations_for_news("news-1")
                .expect("translation");
            assert_eq!(translation[0].translated_summary, None);
            assert_eq!(
                translation[0].translated_content.as_deref(),
                Some("ES must never be translated")
            );
        });
    }

    #[test]
    fn repairs_legacy_requested_language_before_translation() {
        with_database(|state, repository| {
            let mut legacy = item(
                "ru",
                "RTX 3060 и системные требования",
                Some("Студия опубликовала системные требования игры."),
            );
            legacy.original_content =
                Some("Студия опубликовала системные требования игры.".to_string());
            legacy.refresh_source_content_hash();
            repository
                .upsert_news_items(&[legacy])
                .expect("legacy item");
            let service = TranslationService::new(
                state,
                FakeProvider {
                    calls: Arc::new(AtomicUsize::new(0)),
                    fail: None,
                },
            );
            tauri::async_runtime::block_on(service.translate_news_items(
                &["news-ru".to_string()],
                "es-419",
                false,
                None,
            ))
            .expect("translate legacy item");
            let repaired = repository
                .get_news_item_by_id("news-ru")
                .expect("item lookup")
                .expect("repaired item");
            assert_eq!(repaired.source_language, "ru");
            let translation = repository
                .get_translations_for_news("news-ru")
                .expect("translation lookup")
                .into_iter()
                .find(|value| value.provider_id == GOOGLE_PUBLIC_TRANSLATION_PROVIDER_ID)
                .expect("public translation");
            assert_eq!(translation.source_language, "ru");
            assert_eq!(translation.status, TranslationStatus::Translated);
            assert!(translation.translated_content.is_some());
        });
    }

    #[test]
    fn force_retranslate_calls_provider_and_source_hash_creates_new_cache_key() {
        with_database(|state, repository| {
            let first = item("1", "Title", Some("Summary"));
            repository
                .upsert_news_items(std::slice::from_ref(&first))
                .expect("first");
            let calls = Arc::new(AtomicUsize::new(0));
            let service = TranslationService::new(
                state,
                FakeProvider {
                    calls: calls.clone(),
                    fail: None,
                },
            );
            let ids = vec!["news-1".to_string()];
            tauri::async_runtime::block_on(
                service.translate_news_items(&ids, "es-419", false, None),
            )
            .expect("translate");
            tauri::async_runtime::block_on(
                service.translate_news_items(&ids, "es-419", true, None),
            )
            .expect("force");
            assert_eq!(calls.load(Ordering::SeqCst), 2);
            let mut changed = first;
            changed.original_title = "Changed".to_string();
            repository.upsert_news_items(&[changed]).expect("changed");
            tauri::async_runtime::block_on(
                service.translate_news_items(&ids, "es-419", false, None),
            )
            .expect("translate changed source");
            let translations = repository
                .get_translations_for_news("news-1")
                .expect("history");
            assert_eq!(translations.len(), 2);
            assert!(translations
                .iter()
                .any(|translation| translation.status == TranslationStatus::Stale));
            assert!(translations
                .iter()
                .any(|translation| translation.status == TranslationStatus::Translated));
        });
    }

    #[test]
    fn provider_failure_is_typed_without_exposing_secret() {
        with_database(|state, repository| {
            repository
                .upsert_news_items(&[item("1", "Title", None)])
                .expect("item");
            let service = TranslationService::new(
                state,
                FakeProvider {
                    calls: Arc::new(AtomicUsize::new(0)),
                    fail: Some(TranslationErrorKind::RateLimited),
                },
            );
            tauri::async_runtime::block_on(service.translate_news_items(
                &["news-1".to_string()],
                "es-419",
                false,
                None,
            ))
            .expect("summary");
            let translation = repository
                .get_translations_for_news("news-1")
                .expect("translation");
            assert_eq!(translation[0].error_code.as_deref(), Some("rate_limited"));
            assert!(!format!("{translation:?}").contains("secret"));
        });
    }

    #[test]
    fn google_response_is_mapped_by_field_order_without_real_network() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            thread,
        };
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let endpoint = format!("http://{}", listener.local_addr().expect("address"));
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("request");
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request).expect("read request");
            let body = r#"{"data":{"translations":[{"translatedText":"Título"},{"translatedText":"Resumen"}]}}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .expect("response");
        });
        let provider = GoogleTranslationProvider::with_endpoint("secret-key", &endpoint);
        let response =
            tauri::async_runtime::block_on(provider.translate_batch(vec![TranslationRequest {
                request_id: "request-1".to_string(),
                news_item_id: "news-1".to_string(),
                source_language: "en".to_string(),
                target_language: "es-419".to_string(),
                title: "Title".to_string(),
                summary: Some("Summary".to_string()),
                content: None,
                glossary_version: None,
            }]))
            .expect("translation response");
        server.join().expect("server");
        assert_eq!(
            response.results[0].translated_title.as_deref(),
            Some("Título")
        );
        assert_eq!(
            response.results[0].translated_summary.as_deref(),
            Some("Resumen")
        );
        assert_eq!(provider.capabilities().max_batch_items, 64);
        assert!(!provider.capabilities().supports_glossary);
    }

    #[test]
    fn translation_credential_lifecycle_is_encrypted_and_masked() {
        with_database(|state, _repository| {
            let initial = settings::get_translation_configuration(state).expect("initial status");
            assert_eq!(initial.status, "not-configured");
            let configured = settings::save_translation_api_key(state, "translation-secret-123")
                .expect("save credential");
            assert_eq!(configured.status, "configured");
            assert!(configured
                .api_key_masked
                .unwrap_or_default()
                .ends_with("-123"));
            let encrypted: Vec<u8> = state
                .connection
                .lock()
                .expect("connection")
                .query_row(
                    "SELECT encrypted_value FROM provider_credentials WHERE provider_account_id = 'google-cloud-translation-default'",
                    [],
                    |row| row.get(0),
                )
                .expect("encrypted value");
            assert!(!String::from_utf8_lossy(&encrypted).contains("translation-secret-123"));
            let disconnected = settings::delete_translation_api_key(state).expect("disconnect");
            assert_eq!(disconnected.status, "not-configured");
            settings::set_translation_provider_selection(state, GOOGLE_TRANSLATION_PROVIDER_ID)
                .expect("select cloud");
            let error = match TranslationService::from_settings(state) {
                Ok(_) => panic!("credential should be missing"),
                Err(error) => error,
            };
            assert_eq!(error.kind(), TranslationErrorKind::CredentialsMissing);
        });
    }

    #[test]
    fn google_public_is_credential_free_and_maps_domain_language() {
        let provider = GooglePublicTranslationProvider::new().expect("public provider");
        let capabilities = provider.capabilities();
        assert_eq!(capabilities.provider_id, "google-public");
        assert_eq!(
            capabilities.provider_version,
            GOOGLE_PUBLIC_TRANSLATION_PROVIDER_VERSION
        );
        assert!(!capabilities.supports_glossary);
        assert_eq!(map_public_language("es-419").expect("mapping"), "es");
        assert_eq!(map_public_language("es").expect("mapping"), "es");
        assert_eq!(
            map_public_language("fr"),
            Err(TranslationProviderError::UnsupportedLanguage)
        );
    }

    #[test]
    fn public_parser_is_defensive_and_preserves_segment_order() {
        let body = br#"[[["Uno"],[" dos"],[" tres"]],null,"en"]"#;
        assert_eq!(
            parse_public_translation_response(body).expect("parsed"),
            "Uno dos tres"
        );
        assert_eq!(
            parse_public_translation_response(
                br#"[]"),
            Err(TranslationProviderError::InvalidResponse)
        );
        assert_eq!(
            parse_public_translation_response(br#"not-json"#
            ),
            Err(TranslationProviderError::InvalidResponse)
        );
        assert_eq!(
            parse_public_translation_response(br#"<html><body>captcha</body></html>"#),
            Err(TranslationProviderError::InvalidResponse)
        );
    }

    #[test]
    fn public_fragmentation_is_deterministic_and_lossless() {
        let text = format!("{}\n\n{}", "A".repeat(700), "B".repeat(700));
        let fragments = split_translation_text(&text, 800);
        assert!(fragments.len() > 1);
        assert!(fragments
            .iter()
            .all(|fragment| fragment.chars().count() <= 800));
        assert_eq!(fragments.concat(), text);
        assert_eq!(
            split_translation_text("Short title", 800),
            vec!["Short title"]
        );
    }

    #[test]
    fn public_transport_uses_post_body_and_maps_title_and_summary() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            thread,
        };
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let endpoint = format!("http://{}", listener.local_addr().expect("address"));
        let server = thread::spawn(move || {
            for _ in 0..3 {
                let (mut stream, _) = listener.accept().expect("request");
                let mut request = [0_u8; 4096];
                let length = stream.read(&mut request).expect("read request");
                let request = String::from_utf8_lossy(&request[..length]);
                let request_line = request.lines().next().unwrap_or_default();
                assert!(!request_line.contains("Title"));
                assert!(request.contains("tl=es"));
                let body = r#"[[["Translated"]],null,"en"]"#;
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .expect("response");
            }
        });
        let provider = GooglePublicTranslationProvider::with_endpoint(&endpoint);
        let result =
            tauri::async_runtime::block_on(provider.translate_batch(vec![TranslationRequest {
                request_id: "public-1".to_string(),
                news_item_id: "news-1".to_string(),
                source_language: "en".to_string(),
                target_language: "es-419".to_string(),
                title: "Title".to_string(),
                summary: Some("Summary".to_string()),
                content: Some("Content".to_string()),
                glossary_version: None,
            }]))
            .expect("public response");
        server.join().expect("server");
        assert_eq!(
            result.results[0].translated_title.as_deref(),
            Some("Translated")
        );
        assert_eq!(
            result.results[0].translated_summary.as_deref(),
            Some("Translated")
        );
        assert_eq!(
            result.results[0].translated_content.as_deref(),
            Some("Translated")
        );
    }

    #[test]
    fn public_http_errors_are_typed_when_fallback_is_unavailable() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            thread,
        };
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener");
        let endpoint = format!("http://{}", listener.local_addr().expect("address"));
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("request");
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request).expect("read request");
            write!(
                stream,
                "HTTP/1.1 429 Too Many Requests\r\nContent-Length: 0\r\n\r\n"
            )
            .expect("response");
        });
        let provider = GooglePublicTranslationProvider::with_endpoint(&endpoint);
        let error =
            tauri::async_runtime::block_on(provider.translate_batch(vec![TranslationRequest {
                request_id: "public-429".to_string(),
                news_item_id: "news-429".to_string(),
                source_language: "en".to_string(),
                target_language: "es-419".to_string(),
                title: "Title".to_string(),
                summary: None,
                content: None,
                glossary_version: None,
            }]))
            .expect_err("429");
        server.join().expect("server");
        assert_eq!(error, TranslationProviderError::RateLimited);
    }

    #[test]
    fn public_provider_falls_back_to_mymemory_after_google_rate_limit() {
        use std::{
            io::{Read, Write},
            net::TcpListener,
            thread,
        };

        let google_listener = TcpListener::bind("127.0.0.1:0").expect("google listener");
        let google_endpoint = format!(
            "http://{}",
            google_listener.local_addr().expect("google address")
        );
        let google_server = thread::spawn(move || {
            let (mut stream, _) = google_listener.accept().expect("google request");
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request).expect("read google request");
            write!(
                stream,
                "HTTP/1.1 429 Too Many Requests\r\nContent-Length: 0\r\n\r\n"
            )
            .expect("google response");
        });

        let fallback_listener = TcpListener::bind("127.0.0.1:0").expect("fallback listener");
        let fallback_endpoint = format!(
            "http://{}",
            fallback_listener.local_addr().expect("fallback address")
        );
        let fallback_server = thread::spawn(move || {
            let (mut stream, _) = fallback_listener.accept().expect("fallback request");
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request).expect("read fallback request");
            let body = r#"{"responseData":{"translatedText":"Pragmata 2 podría convertirse en realidad"},"responseStatus":200,"quotaFinished":false}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
                body.len(),
                body
            )
            .expect("fallback response");
        });

        let provider =
            GooglePublicTranslationProvider::with_endpoints(&google_endpoint, &fallback_endpoint);
        let result =
            tauri::async_runtime::block_on(provider.translate_batch(vec![TranslationRequest {
                request_id: "pragmata-qa".to_string(),
                news_item_id: "news-pragmata-qa".to_string(),
                source_language: "ru".to_string(),
                target_language: "es-419".to_string(),
                title: "Pragmata 2 может стать реальностью".to_string(),
                summary: None,
                content: None,
                glossary_version: None,
            }]))
            .expect("fallback response");
        google_server.join().expect("google server");
        fallback_server.join().expect("fallback server");
        assert_eq!(
            result.results[0].translated_title.as_deref(),
            Some("Pragmata 2 podría convertirse en realidad")
        );
    }

    #[test]
    fn public_is_default_and_explicit_cloud_selection_is_respected() {
        with_database(|state, _repository| {
            let initial = get_active_translation_provider(state).expect("active provider");
            assert_eq!(initial.active_provider_id, "google-public");
            assert!(!initial.explicit_selection);
            let cloud = set_active_translation_provider(state, GOOGLE_TRANSLATION_PROVIDER_ID)
                .expect("cloud selection");
            assert_eq!(cloud.active_provider_id, GOOGLE_TRANSLATION_PROVIDER_ID);
            assert!(cloud.explicit_selection);
            let error = match TranslationService::from_settings(state) {
                Ok(_) => panic!("cloud should require credentials"),
                Err(error) => error,
            };
            assert_eq!(error.kind(), TranslationErrorKind::CredentialsMissing);
            let public =
                set_active_translation_provider(state, GOOGLE_PUBLIC_TRANSLATION_PROVIDER_ID)
                    .expect("public selection");
            assert_eq!(
                public.active_provider_id,
                GOOGLE_PUBLIC_TRANSLATION_PROVIDER_ID
            );
        });
    }

    #[test]
    fn cache_keeps_public_and_cloud_translations_separate() {
        with_database(|_state, repository| {
            let item = item("cache", "Title", Some("Summary"));
            repository
                .upsert_news_items(std::slice::from_ref(&item))
                .expect("item");
            for provider_id in [
                GOOGLE_PUBLIC_TRANSLATION_PROVIDER_ID,
                GOOGLE_TRANSLATION_PROVIDER_ID,
            ] {
                let provider_version = if provider_id == GOOGLE_PUBLIC_TRANSLATION_PROVIDER_ID {
                    GOOGLE_PUBLIC_TRANSLATION_PROVIDER_VERSION
                } else {
                    GOOGLE_TRANSLATION_PROVIDER_VERSION
                };
                repository
                    .save_translation(&NewsTranslation {
                        id: String::new(),
                        news_item_id: item.id.clone(),
                        source_language: "en".to_string(),
                        target_language: "es-419".to_string(),
                        translated_title: Some(provider_id.to_string()),
                        translated_summary: None,
                        translated_content: None,
                        status: TranslationStatus::Translated,
                        provider_id: provider_id.to_string(),
                        provider_version: Some(provider_version.to_string()),
                        glossary_version: None,
                        source_content_hash: item.source_content_hash.clone(),
                        translated_content_hash: None,
                        translated_at: Some("1".to_string()),
                        last_attempt_at: Some("1".to_string()),
                        error_code: None,
                        created_at: "1".to_string(),
                        updated_at: "1".to_string(),
                    })
                    .expect("translation");
            }
            let public = repository
                .get_reusable_translation_for_provider(
                    &item.id,
                    "es-419",
                    &item.source_content_hash,
                    GOOGLE_PUBLIC_TRANSLATION_PROVIDER_ID,
                    GOOGLE_PUBLIC_TRANSLATION_PROVIDER_VERSION,
                    None,
                )
                .expect("public cache")
                .expect("public translation");
            let cloud = repository
                .get_reusable_translation_for_provider(
                    &item.id,
                    "es-419",
                    &item.source_content_hash,
                    GOOGLE_TRANSLATION_PROVIDER_ID,
                    GOOGLE_TRANSLATION_PROVIDER_VERSION,
                    None,
                )
                .expect("cloud cache")
                .expect("cloud translation");
            assert_eq!(public.provider_id, GOOGLE_PUBLIC_TRANSLATION_PROVIDER_ID);
            assert_eq!(cloud.provider_id, GOOGLE_TRANSLATION_PROVIDER_ID);
            assert_eq!(
                repository
                    .get_translations_for_news(&item.id)
                    .expect("history")
                    .len(),
                2
            );
        });
    }
}
