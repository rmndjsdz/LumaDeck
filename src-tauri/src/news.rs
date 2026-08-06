use crate::settings::{DatabaseError, DatabaseState};
use rusqlite::{params, params_from_iter, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fmt;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NewsCategory {
    Official,
    Update,
    Event,
    Community,
    Media,
    Dlc,
    Maintenance,
    Other,
}

impl NewsCategory {
    fn as_str(self) -> &'static str {
        match self {
            Self::Official => "official",
            Self::Update => "update",
            Self::Event => "event",
            Self::Community => "community",
            Self::Media => "media",
            Self::Dlc => "dlc",
            Self::Maintenance => "maintenance",
            Self::Other => "other",
        }
    }
}

impl FromStr for NewsCategory {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "official" => Ok(Self::Official),
            "update" => Ok(Self::Update),
            "event" => Ok(Self::Event),
            "community" => Ok(Self::Community),
            "media" => Ok(Self::Media),
            "dlc" => Ok(Self::Dlc),
            "maintenance" => Ok(Self::Maintenance),
            "other" => Ok(Self::Other),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NewsContentFormat {
    PlainText,
    Html,
    Markdown,
    Unknown,
}

impl NewsContentFormat {
    fn as_str(self) -> &'static str {
        match self {
            Self::PlainText => "plain_text",
            Self::Html => "html",
            Self::Markdown => "markdown",
            Self::Unknown => "unknown",
        }
    }
}

impl FromStr for NewsContentFormat {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "plain_text" => Ok(Self::PlainText),
            "html" => Ok(Self::Html),
            "markdown" => Ok(Self::Markdown),
            "unknown" => Ok(Self::Unknown),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationStatus {
    Pending,
    Translating,
    Translated,
    Failed,
    Stale,
}

impl TranslationStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Translating => "translating",
            Self::Translated => "translated",
            Self::Failed => "failed",
            Self::Stale => "stale",
        }
    }
}

impl FromStr for TranslationStatus {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "pending" => Ok(Self::Pending),
            "translating" => Ok(Self::Translating),
            "translated" => Ok(Self::Translated),
            "failed" => Ok(Self::Failed),
            "stale" => Ok(Self::Stale),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsItem {
    pub id: String,
    pub provider_id: String,
    pub external_id: String,
    pub game_id: String,
    pub external_game_id: Option<String>,
    pub category: NewsCategory,
    pub source_url: String,
    pub canonical_url: Option<String>,
    pub published_at: String,
    pub updated_at: Option<String>,
    pub first_seen_at: String,
    pub source_language: String,
    pub original_title: String,
    pub original_summary: Option<String>,
    pub original_content: Option<String>,
    pub content_format: NewsContentFormat,
    pub source_content_hash: String,
    pub provider_metadata: Option<Value>,
    pub created_at: String,
    pub persisted_updated_at: String,
}

impl NewsItem {
    pub fn stable_identity_key(
        provider_id: &str,
        external_id: Option<&str>,
        canonical_url: Option<&str>,
    ) -> String {
        let provider = provider_id.trim();
        if let Some(external_id) = external_id.map(str::trim).filter(|value| !value.is_empty()) {
            return format!("provider:{provider}|external:{external_id}");
        }
        if let Some(canonical_url) = canonical_url
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return format!("provider:{provider}|canonical:{canonical_url}");
        }
        format!("provider:{provider}|fallback:unknown")
    }

    pub fn stable_id(
        provider_id: &str,
        external_id: Option<&str>,
        canonical_url: Option<&str>,
    ) -> String {
        format!(
            "news-{}",
            sha256_hex(Self::stable_identity_key(
                provider_id,
                external_id,
                canonical_url,
            ))
        )
    }

    pub fn stable_id_from_source(
        provider_id: &str,
        external_id: Option<&str>,
        canonical_url: Option<&str>,
        source_url: &str,
        title: &str,
        published_at: &str,
    ) -> String {
        let identity = if external_id
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
            || canonical_url
                .map(str::trim)
                .is_some_and(|value| !value.is_empty())
        {
            Self::stable_identity_key(provider_id, external_id, canonical_url)
        } else {
            format!(
                "provider:{}|fallback:{}",
                provider_id.trim(),
                sha256_hex(format!(
                    "{}\u{1f}{}\u{1f}{}",
                    normalize_hash_text(source_url),
                    normalize_hash_text(title),
                    normalize_hash_text(published_at)
                ))
            )
        };
        format!("news-{}", sha256_hex(identity))
    }

    pub fn calculated_source_content_hash(&self) -> String {
        source_content_hash(
            &self.source_language,
            &self.original_title,
            self.original_summary.as_deref(),
            self.original_content.as_deref(),
        )
    }

    pub fn refresh_source_content_hash(&mut self) {
        self.source_content_hash = self.calculated_source_content_hash();
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsTranslation {
    pub id: String,
    pub news_item_id: String,
    pub source_language: String,
    pub target_language: String,
    pub translated_title: Option<String>,
    pub translated_summary: Option<String>,
    pub translated_content: Option<String>,
    pub status: TranslationStatus,
    pub provider_id: String,
    pub provider_version: Option<String>,
    pub glossary_version: Option<String>,
    pub source_content_hash: String,
    pub translated_content_hash: Option<String>,
    pub translated_at: Option<String>,
    pub last_attempt_at: Option<String>,
    pub error_code: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl NewsTranslation {
    pub fn stable_id(&self) -> String {
        let key = format!(
            "news:{}|{}|{}|{}|{}|{}|{}",
            self.news_item_id,
            self.source_language,
            self.target_language,
            self.provider_id,
            self.provider_version.as_deref().unwrap_or_default(),
            self.glossary_version.as_deref().unwrap_or_default(),
            self.source_content_hash,
        );
        format!("translation-{}", sha256_hex(key))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsSyncState {
    pub provider_id: String,
    pub game_id: String,
    pub last_successful_sync_at: Option<String>,
    pub last_attempt_at: Option<String>,
    pub last_error_code: Option<String>,
    pub cursor: Option<String>,
    pub is_stale: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewsFeedItem {
    pub item: NewsItem,
    pub translation: Option<NewsTranslation>,
}

#[derive(Debug, Clone, Default)]
pub struct NewsFeedQuery {
    pub game_id: Option<String>,
    pub categories: Vec<NewsCategory>,
    pub limit: usize,
    pub target_language: Option<String>,
    pub translation_provider_id: Option<String>,
    pub translation_provider_version: Option<String>,
    pub translation_glossary_version: Option<String>,
}

pub struct NewsRepository<'a> {
    state: &'a DatabaseState,
}

impl<'a> NewsRepository<'a> {
    pub fn new(state: &'a DatabaseState) -> Self {
        Self { state }
    }

    pub fn upsert_news_items(&self, items: &[NewsItem]) -> Result<Vec<NewsItem>, DatabaseError> {
        let mut connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.transaction()?;
        let mut persisted = Vec::with_capacity(items.len());

        for source_item in items {
            let mut item = source_item.clone();
            item.refresh_source_content_hash();
            let now = unix_timestamp();
            if item.created_at.is_empty() {
                item.created_at = now.clone();
            }
            if item.first_seen_at.is_empty() {
                item.first_seen_at = now.clone();
            }
            if item.persisted_updated_at.is_empty() {
                item.persisted_updated_at = now;
            }
            if item.id.is_empty() {
                item.id = NewsItem::stable_id_from_source(
                    &item.provider_id,
                    Some(&item.external_id),
                    item.canonical_url.as_deref(),
                    &item.source_url,
                    &item.original_title,
                    &item.published_at,
                );
            }

            let existing = find_existing_news_item(&transaction, &item)?;
            if let Some((existing_id, existing_hash)) = existing {
                if existing_hash != item.source_content_hash {
                    transaction.execute(
                        "UPDATE news_translations
                         SET status = 'stale', updated_at = ?2
                         WHERE news_item_id = ?1 AND source_content_hash <> ?3",
                        params![
                            existing_id,
                            item.persisted_updated_at,
                            item.source_content_hash
                        ],
                    )?;
                }
                transaction.execute(
                    "UPDATE news_items SET
                       provider_id = ?2, external_id = ?3, game_id = ?4,
                       external_game_id = ?5, category = ?6, source_url = ?7,
                       canonical_url = ?8, published_at = ?9, updated_at = ?10,
                       source_language = ?11, original_title = ?12,
                       original_summary = ?13, original_content = ?14,
                       content_format = ?15, source_content_hash = ?16,
                       provider_metadata = ?17, persisted_updated_at = ?18
                     WHERE id = ?1",
                    params![
                        existing_id,
                        item.provider_id,
                        item.external_id,
                        item.game_id,
                        item.external_game_id,
                        item.category.as_str(),
                        item.source_url,
                        item.canonical_url,
                        item.published_at,
                        item.updated_at,
                        item.source_language,
                        item.original_title,
                        item.original_summary,
                        item.original_content,
                        item.content_format.as_str(),
                        item.source_content_hash,
                        serialize_metadata(item.provider_metadata.as_ref()),
                        item.persisted_updated_at,
                    ],
                )?;
                item.id = existing_id;
            } else {
                transaction.execute(
                    "INSERT INTO news_items(
                       id, provider_id, external_id, game_id, external_game_id,
                       category, source_url, canonical_url, published_at,
                       updated_at, first_seen_at, source_language, original_title,
                       original_summary, original_content, content_format,
                       source_content_hash, provider_metadata, created_at,
                       persisted_updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                               ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
                    params![
                        item.id,
                        item.provider_id,
                        item.external_id,
                        item.game_id,
                        item.external_game_id,
                        item.category.as_str(),
                        item.source_url,
                        item.canonical_url,
                        item.published_at,
                        item.updated_at,
                        item.first_seen_at,
                        item.source_language,
                        item.original_title,
                        item.original_summary,
                        item.original_content,
                        item.content_format.as_str(),
                        item.source_content_hash,
                        serialize_metadata(item.provider_metadata.as_ref()),
                        item.created_at,
                        item.persisted_updated_at,
                    ],
                )?;
            }
            persisted.push(item);
        }

        transaction.commit()?;
        Ok(persisted)
    }

    pub fn get_news_feed(&self, query: &NewsFeedQuery) -> Result<Vec<NewsFeedItem>, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let mut sql = String::from(
            "SELECT id, provider_id, external_id, game_id, external_game_id, category,
                    source_url, canonical_url, published_at, updated_at, first_seen_at,
                    source_language, original_title, original_summary, original_content,
                    content_format, source_content_hash, provider_metadata, created_at,
                    persisted_updated_at
             FROM news_items WHERE (?1 IS NULL OR game_id = ?1)",
        );
        let mut values = vec![query
            .game_id
            .clone()
            .map(rusqlite::types::Value::from)
            .unwrap_or(rusqlite::types::Value::Null)];
        for (index, category) in query.categories.iter().enumerate() {
            if index == 0 {
                sql.push_str(" AND category IN (");
            } else {
                sql.push_str(", ");
            }
            sql.push_str(&format!("?{}", values.len() + 1));
            values.push(rusqlite::types::Value::from(category.as_str().to_string()));
        }
        if !query.categories.is_empty() {
            sql.push(')');
        }
        let limit = if query.limit == 0 { 50 } else { query.limit };
        sql.push_str(" ORDER BY published_at DESC, id DESC LIMIT ?");
        values.push(rusqlite::types::Value::from(limit as i64));

        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values), map_news_item)?;
        let items = rows.collect::<Result<Vec<_>, _>>()?;
        let mut result = Vec::with_capacity(items.len());
        for item in items {
            let translation = match (
                query.target_language.as_deref(),
                query.translation_provider_id.as_deref(),
                query.translation_provider_version.as_deref(),
            ) {
                (Some(target_language), Some(provider_id), Some(provider_version)) => {
                    get_reusable_translation_for_provider_from_connection(
                        &connection,
                        &item.id,
                        target_language,
                        &item.source_content_hash,
                        provider_id,
                        provider_version,
                        query
                            .translation_glossary_version
                            .as_deref()
                            .unwrap_or_default(),
                    )?
                }
                (Some(target_language), _, _) => get_reusable_translation_from_connection(
                    &connection,
                    &item.id,
                    target_language,
                    &item.source_content_hash,
                )?,
                _ => None,
            };
            result.push(NewsFeedItem { item, translation });
        }
        Ok(result)
    }

    pub fn get_news_item_by_id(&self, id: &str) -> Result<Option<NewsItem>, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        connection
            .query_row(
                "SELECT id, provider_id, external_id, game_id, external_game_id, category,
                        source_url, canonical_url, published_at, updated_at, first_seen_at,
                        source_language, original_title, original_summary, original_content,
                        content_format, source_content_hash, provider_metadata, created_at,
                        persisted_updated_at
                 FROM news_items WHERE id = ?1",
                params![id],
                map_news_item,
            )
            .optional()
            .map_err(DatabaseError::from)
    }

    pub fn get_news_items_by_game(&self, game_id: &str) -> Result<Vec<NewsItem>, DatabaseError> {
        let feed = self.get_news_feed(&NewsFeedQuery {
            game_id: Some(game_id.to_string()),
            limit: 0,
            ..NewsFeedQuery::default()
        })?;
        Ok(feed.into_iter().map(|entry| entry.item).collect())
    }

    pub fn save_translation(
        &self,
        translation: &NewsTranslation,
    ) -> Result<NewsTranslation, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let now = unix_timestamp();
        let provider_version = translation.provider_version.clone().unwrap_or_default();
        let glossary_version = translation.glossary_version.clone().unwrap_or_default();
        let id = if translation.id.is_empty() {
            translation.stable_id()
        } else {
            translation.id.clone()
        };
        let created_at = if translation.created_at.is_empty() {
            now.clone()
        } else {
            translation.created_at.clone()
        };
        let updated_at = if translation.updated_at.is_empty() {
            now
        } else {
            translation.updated_at.clone()
        };
        connection.execute(
            "INSERT INTO news_translations(
               id, news_item_id, source_language, target_language,
               translated_title, translated_summary, translated_content, status,
               provider_id, provider_version, glossary_version, source_content_hash,
               translated_content_hash, translated_at, last_attempt_at, error_code,
               created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12,
                       ?13, ?14, ?15, ?16, ?17, ?18)
             ON CONFLICT(news_item_id, source_language, target_language, provider_id,
                         provider_version, glossary_version, source_content_hash)
             DO UPDATE SET translated_title = excluded.translated_title,
               translated_summary = excluded.translated_summary,
               translated_content = excluded.translated_content,
               status = excluded.status,
               translated_content_hash = excluded.translated_content_hash,
               translated_at = excluded.translated_at,
               last_attempt_at = excluded.last_attempt_at,
               error_code = excluded.error_code,
               updated_at = excluded.updated_at",
            params![
                id,
                translation.news_item_id,
                translation.source_language,
                translation.target_language,
                translation.translated_title,
                translation.translated_summary,
                translation.translated_content,
                translation.status.as_str(),
                translation.provider_id,
                provider_version,
                glossary_version,
                translation.source_content_hash,
                translation.translated_content_hash,
                translation.translated_at,
                translation.last_attempt_at,
                translation.error_code,
                created_at,
                updated_at,
            ],
        )?;
        get_translation_by_key(
            &connection,
            &translation.news_item_id,
            &translation.source_language,
            &translation.target_language,
            &translation.provider_id,
            translation.provider_version.as_deref().unwrap_or_default(),
            translation.glossary_version.as_deref().unwrap_or_default(),
            &translation.source_content_hash,
        )?
        .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows.into())
    }

    pub fn get_translations_for_news(
        &self,
        news_item_id: &str,
    ) -> Result<Vec<NewsTranslation>, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let mut statement = connection.prepare(
            "SELECT id, news_item_id, source_language, target_language,
                    translated_title, translated_summary, translated_content, status,
                    provider_id, provider_version, glossary_version, source_content_hash,
                    translated_content_hash, translated_at, last_attempt_at, error_code,
                    created_at, updated_at
             FROM news_translations WHERE news_item_id = ?1 ORDER BY updated_at DESC, id DESC",
        )?;
        let rows = statement.query_map(params![news_item_id], map_translation)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(DatabaseError::from)
    }

    pub fn get_reusable_translation(
        &self,
        news_item_id: &str,
        target_language: &str,
        source_content_hash: &str,
    ) -> Result<Option<NewsTranslation>, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        get_reusable_translation_from_connection(
            &connection,
            news_item_id,
            target_language,
            source_content_hash,
        )
    }

    pub fn get_reusable_translation_for_provider(
        &self,
        news_item_id: &str,
        target_language: &str,
        source_content_hash: &str,
        provider_id: &str,
        provider_version: &str,
        glossary_version: Option<&str>,
    ) -> Result<Option<NewsTranslation>, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        get_reusable_translation_for_provider_from_connection(
            &connection,
            news_item_id,
            target_language,
            source_content_hash,
            provider_id,
            provider_version,
            glossary_version.unwrap_or_default(),
        )
    }

    pub fn mark_translation_stale(
        &self,
        news_item_id: &str,
        source_content_hash: &str,
    ) -> Result<u64, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let changed = connection.execute(
            "UPDATE news_translations SET status = 'stale', updated_at = ?3
             WHERE news_item_id = ?1 AND source_content_hash = ?2 AND status <> 'stale'",
            params![news_item_id, source_content_hash, unix_timestamp()],
        )?;
        Ok(changed as u64)
    }

    pub fn save_sync_state(&self, state: &NewsSyncState) -> Result<NewsSyncState, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let now = unix_timestamp();
        let created_at = if state.created_at.is_empty() {
            now.clone()
        } else {
            state.created_at.clone()
        };
        let updated_at = if state.updated_at.is_empty() {
            now
        } else {
            state.updated_at.clone()
        };
        connection.execute(
            "INSERT INTO news_sync_state(
               provider_id, game_id, last_successful_sync_at, last_attempt_at,
               last_error_code, cursor, is_stale, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(provider_id, game_id) DO UPDATE SET
               last_successful_sync_at = excluded.last_successful_sync_at,
               last_attempt_at = excluded.last_attempt_at,
               last_error_code = excluded.last_error_code,
               cursor = excluded.cursor,
               is_stale = excluded.is_stale,
               updated_at = excluded.updated_at",
            params![
                state.provider_id,
                state.game_id,
                state.last_successful_sync_at,
                state.last_attempt_at,
                state.last_error_code,
                state.cursor,
                state.is_stale,
                created_at,
                updated_at,
            ],
        )?;
        get_sync_state_from_connection(&connection, &state.provider_id, &state.game_id)?
            .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows.into())
    }

    pub fn get_sync_state(
        &self,
        provider_id: &str,
        game_id: &str,
    ) -> Result<Option<NewsSyncState>, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        get_sync_state_from_connection(&connection, provider_id, game_id)
    }
}

fn find_existing_news_item(
    transaction: &Transaction<'_>,
    item: &NewsItem,
) -> Result<Option<(String, String)>, DatabaseError> {
    let by_external = transaction
        .query_row(
            "SELECT id, source_content_hash FROM news_items
             WHERE provider_id = ?1 AND external_id = ?2",
            params![item.provider_id, item.external_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if by_external.is_some() {
        return Ok(by_external);
    }
    item.canonical_url
        .as_deref()
        .filter(|value| !value.is_empty())
        .map(|canonical_url| {
            transaction
                .query_row(
                    "SELECT id, source_content_hash FROM news_items
                     WHERE provider_id = ?1 AND canonical_url = ?2",
                    params![item.provider_id, canonical_url],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()
        })
        .transpose()
        .map_err(DatabaseError::from)
        .map(|value| value.flatten())
}

fn get_translation_by_key(
    connection: &rusqlite::Connection,
    news_item_id: &str,
    source_language: &str,
    target_language: &str,
    provider_id: &str,
    provider_version: &str,
    glossary_version: &str,
    source_content_hash: &str,
) -> Result<Option<NewsTranslation>, DatabaseError> {
    connection
        .query_row(
            "SELECT id, news_item_id, source_language, target_language,
                    translated_title, translated_summary, translated_content, status,
                    provider_id, provider_version, glossary_version, source_content_hash,
                    translated_content_hash, translated_at, last_attempt_at, error_code,
                    created_at, updated_at
             FROM news_translations
             WHERE news_item_id = ?1 AND source_language = ?2 AND target_language = ?3
               AND provider_id = ?4 AND provider_version = ?5 AND glossary_version = ?6
               AND source_content_hash = ?7",
            params![
                news_item_id,
                source_language,
                target_language,
                provider_id,
                provider_version,
                glossary_version,
                source_content_hash,
            ],
            map_translation,
        )
        .optional()
        .map_err(DatabaseError::from)
}

fn get_reusable_translation_from_connection(
    connection: &rusqlite::Connection,
    news_item_id: &str,
    target_language: &str,
    source_content_hash: &str,
) -> Result<Option<NewsTranslation>, DatabaseError> {
    connection
        .query_row(
            "SELECT id, news_item_id, source_language, target_language,
                    translated_title, translated_summary, translated_content, status,
                    provider_id, provider_version, glossary_version, source_content_hash,
                    translated_content_hash, translated_at, last_attempt_at, error_code,
                    created_at, updated_at
             FROM news_translations
             WHERE news_item_id = ?1 AND target_language = ?2
               AND source_content_hash = ?3 AND status = 'translated'
             ORDER BY translated_at DESC, updated_at DESC, id DESC LIMIT 1",
            params![news_item_id, target_language, source_content_hash],
            map_translation,
        )
        .optional()
        .map_err(DatabaseError::from)
}

fn get_reusable_translation_for_provider_from_connection(
    connection: &rusqlite::Connection,
    news_item_id: &str,
    target_language: &str,
    source_content_hash: &str,
    provider_id: &str,
    provider_version: &str,
    glossary_version: &str,
) -> Result<Option<NewsTranslation>, DatabaseError> {
    connection
        .query_row(
            "SELECT id, news_item_id, source_language, target_language,
                    translated_title, translated_summary, translated_content, status,
                    provider_id, provider_version, glossary_version, source_content_hash,
                    translated_content_hash, translated_at, last_attempt_at, error_code,
                    created_at, updated_at
             FROM news_translations
             WHERE news_item_id = ?1 AND target_language = ?2
               AND source_content_hash = ?3 AND provider_id = ?4
               AND provider_version = ?5 AND glossary_version = ?6
               AND status = 'translated'
             ORDER BY translated_at DESC, updated_at DESC, id DESC LIMIT 1",
            params![
                news_item_id,
                target_language,
                source_content_hash,
                provider_id,
                provider_version,
                glossary_version,
            ],
            map_translation,
        )
        .optional()
        .map_err(DatabaseError::from)
}

fn get_sync_state_from_connection(
    connection: &rusqlite::Connection,
    provider_id: &str,
    game_id: &str,
) -> Result<Option<NewsSyncState>, DatabaseError> {
    connection
        .query_row(
            "SELECT provider_id, game_id, last_successful_sync_at, last_attempt_at,
                    last_error_code, cursor, is_stale, created_at, updated_at
             FROM news_sync_state WHERE provider_id = ?1 AND game_id = ?2",
            params![provider_id, game_id],
            |row| {
                Ok(NewsSyncState {
                    provider_id: row.get(0)?,
                    game_id: row.get(1)?,
                    last_successful_sync_at: row.get(2)?,
                    last_attempt_at: row.get(3)?,
                    last_error_code: row.get(4)?,
                    cursor: row.get(5)?,
                    is_stale: row.get::<_, i64>(6)? != 0,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                })
            },
        )
        .optional()
        .map_err(DatabaseError::from)
}

fn map_news_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<NewsItem> {
    Ok(NewsItem {
        id: row.get(0)?,
        provider_id: row.get(1)?,
        external_id: row.get(2)?,
        game_id: row.get(3)?,
        external_game_id: row.get(4)?,
        category: row
            .get::<_, String>(5)?
            .parse()
            .unwrap_or(NewsCategory::Other),
        source_url: row.get(6)?,
        canonical_url: row.get(7)?,
        published_at: row.get(8)?,
        updated_at: row.get(9)?,
        first_seen_at: row.get(10)?,
        source_language: row.get(11)?,
        original_title: row.get(12)?,
        original_summary: row.get(13)?,
        original_content: row.get(14)?,
        content_format: row
            .get::<_, String>(15)?
            .parse()
            .unwrap_or(NewsContentFormat::Unknown),
        source_content_hash: row.get(16)?,
        provider_metadata: row
            .get::<_, Option<String>>(17)?
            .and_then(|value| serde_json::from_str(&value).ok()),
        created_at: row.get(18)?,
        persisted_updated_at: row.get(19)?,
    })
}

fn map_translation(row: &rusqlite::Row<'_>) -> rusqlite::Result<NewsTranslation> {
    let provider_version: String = row.get(9)?;
    let glossary_version: String = row.get(10)?;
    Ok(NewsTranslation {
        id: row.get(0)?,
        news_item_id: row.get(1)?,
        source_language: row.get(2)?,
        target_language: row.get(3)?,
        translated_title: row.get(4)?,
        translated_summary: row.get(5)?,
        translated_content: row.get(6)?,
        status: row
            .get::<_, String>(7)?
            .parse()
            .unwrap_or(TranslationStatus::Failed),
        provider_id: row.get(8)?,
        provider_version: (!provider_version.is_empty()).then_some(provider_version),
        glossary_version: (!glossary_version.is_empty()).then_some(glossary_version),
        source_content_hash: row.get(11)?,
        translated_content_hash: row.get(12)?,
        translated_at: row.get(13)?,
        last_attempt_at: row.get(14)?,
        error_code: row.get(15)?,
        created_at: row.get(16)?,
        updated_at: row.get(17)?,
    })
}

fn source_content_hash(
    source_language: &str,
    title: &str,
    summary: Option<&str>,
    content: Option<&str>,
) -> String {
    let canonical = [
        normalize_hash_text(source_language),
        normalize_hash_text(title),
        normalize_hash_text(summary.unwrap_or_default()),
        normalize_hash_text(content.unwrap_or_default()),
    ]
    .join("\u{1f}");
    sha256_hex(canonical)
}

fn normalize_hash_text(value: &str) -> String {
    value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn sha256_hex(value: impl AsRef<[u8]>) -> String {
    Sha256::digest(value)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn serialize_metadata(value: Option<&Value>) -> Option<String> {
    value.and_then(|metadata| serde_json::to_string(metadata).ok())
}

fn unix_timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

impl fmt::Display for NewsCategory {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data_directory::DataDirectoryResolver;
    use crate::settings::DatabaseState;
    use std::fs;
    use std::path::PathBuf;

    fn test_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "lumadeck-news-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos()
        ))
    }

    fn test_item(external_id: &str, title: &str, published_at: &str) -> NewsItem {
        let mut item = NewsItem {
            id: NewsItem::stable_id(
                "test",
                Some(external_id),
                Some(&format!("https://example.test/news/{external_id}")),
            ),
            provider_id: "test".to_string(),
            external_id: external_id.to_string(),
            game_id: "game-001".to_string(),
            external_game_id: Some("external-001".to_string()),
            category: NewsCategory::Official,
            source_url: format!("https://example.test/{external_id}"),
            canonical_url: Some(format!("https://example.test/news/{external_id}")),
            published_at: published_at.to_string(),
            updated_at: None,
            first_seen_at: "1700000000".to_string(),
            source_language: "en".to_string(),
            original_title: title.to_string(),
            original_summary: Some("A summary".to_string()),
            original_content: Some("The content".to_string()),
            content_format: NewsContentFormat::PlainText,
            source_content_hash: String::new(),
            provider_metadata: None,
            created_at: "1700000000".to_string(),
            persisted_updated_at: "1700000000".to_string(),
        };
        item.refresh_source_content_hash();
        item
    }

    fn with_repository<T>(test: impl FnOnce(&NewsRepository<'_>) -> T) -> T {
        let root = test_root("repository");
        let state = DatabaseState::open(DataDirectoryResolver::for_app_data(&root))
            .expect("database state");
        state
            .connection
            .lock()
            .expect("database connection")
            .execute(
                "INSERT INTO games(
                   id, title, sort_title, provider, platform, favorite, installed,
                   progress, status, created_at, updated_at
                 ) VALUES ('game-001', 'Test game', 'test game', 'test', 'test', 0, 1,
                           0, 'not-started', '1700000000', '1700000000')",
                [],
            )
            .expect("test game");
        let result = test(&NewsRepository::new(&state));
        drop(state);
        fs::remove_dir_all(root).expect("remove test directory");
        result
    }

    #[test]
    fn source_hash_ignores_irrelevant_whitespace_but_detects_content_changes() {
        assert_eq!(
            NewsItem::stable_id_from_source(
                "test",
                None,
                None,
                "https://example.test/source",
                "Title",
                "10"
            ),
            NewsItem::stable_id_from_source(
                "test",
                None,
                None,
                "https://example.test/source",
                "Title",
                "10"
            )
        );
        let mut first = test_item("news-1", "Title", "3");
        let mut second = test_item("news-1", " Title ", "3");
        second.original_summary = Some("A  summary".to_string());
        first.refresh_source_content_hash();
        second.refresh_source_content_hash();
        assert_eq!(first.source_content_hash, second.source_content_hash);
        second.original_title = "Changed title".to_string();
        second.refresh_source_content_hash();
        assert_ne!(first.source_content_hash, second.source_content_hash);
    }

    #[test]
    fn upsert_preserves_identity_updates_without_duplicates_and_orders_feed() {
        with_repository(|repository| {
            let first = test_item("news-1", "Old title", "10");
            let second = test_item("news-2", "Newer title", "20");
            let inserted = repository
                .upsert_news_items(&[first.clone(), second])
                .expect("insert news");
            let mut updated = first.clone();
            updated.original_title = "Updated title".to_string();
            let updated_result = repository
                .upsert_news_items(&[updated])
                .expect("update news");
            assert_eq!(inserted[0].id, updated_result[0].id);
            assert_eq!(
                repository.get_news_items_by_game("game-001").unwrap().len(),
                2
            );
            assert_eq!(
                repository
                    .get_news_feed(&NewsFeedQuery {
                        game_id: Some("game-001".to_string()),
                        limit: 10,
                        ..NewsFeedQuery::default()
                    })
                    .unwrap()[0]
                    .item
                    .external_id,
                "news-2"
            );
        });
    }

    #[test]
    fn translation_is_reused_and_staled_when_source_hash_changes() {
        with_repository(|repository| {
            let item = test_item("news-1", "Title", "10");
            repository
                .upsert_news_items(&[item.clone()])
                .expect("insert news");
            let translation = NewsTranslation {
                id: String::new(),
                news_item_id: item.id.clone(),
                source_language: "en".to_string(),
                target_language: "es".to_string(),
                translated_title: Some("Título".to_string()),
                translated_summary: None,
                translated_content: None,
                status: TranslationStatus::Translated,
                provider_id: "test-translator".to_string(),
                provider_version: Some("1".to_string()),
                glossary_version: None,
                source_content_hash: item.source_content_hash.clone(),
                translated_content_hash: None,
                translated_at: Some("20".to_string()),
                last_attempt_at: Some("20".to_string()),
                error_code: None,
                created_at: "20".to_string(),
                updated_at: "20".to_string(),
            };
            repository
                .save_translation(&translation)
                .expect("save translation");
            let mut failed_translation = translation.clone();
            failed_translation.id = String::new();
            failed_translation.target_language = "fr".to_string();
            failed_translation.status = TranslationStatus::Failed;
            failed_translation.error_code = Some("TRANSLATOR_UNAVAILABLE".to_string());
            repository
                .save_translation(&failed_translation)
                .expect("save failed translation");
            assert_eq!(
                repository
                    .get_news_item_by_id(&item.id)
                    .unwrap()
                    .unwrap()
                    .original_title,
                "Title"
            );
            assert!(repository
                .get_reusable_translation(&item.id, "es", &item.source_content_hash)
                .unwrap()
                .is_some());
            let mut changed = item;
            changed.original_content = Some("Changed content".to_string());
            let changed_id = changed.id.clone();
            repository
                .upsert_news_items(&[changed])
                .expect("update news");
            assert_eq!(
                repository.get_translations_for_news(&changed_id).unwrap()[0].status,
                TranslationStatus::Stale
            );
        });
    }

    #[test]
    fn category_filter_and_sync_state_round_trip() {
        with_repository(|repository| {
            let mut item = test_item("news-1", "Title", "10");
            item.category = NewsCategory::Dlc;
            repository.upsert_news_items(&[item]).expect("insert news");
            let feed = repository
                .get_news_feed(&NewsFeedQuery {
                    categories: vec![NewsCategory::Dlc],
                    limit: 10,
                    ..NewsFeedQuery::default()
                })
                .expect("filtered feed");
            assert_eq!(feed.len(), 1);
            let state = NewsSyncState {
                provider_id: "test".to_string(),
                game_id: "game-001".to_string(),
                last_successful_sync_at: Some("30".to_string()),
                last_attempt_at: Some("30".to_string()),
                last_error_code: None,
                cursor: Some("cursor-1".to_string()),
                is_stale: false,
                created_at: String::new(),
                updated_at: String::new(),
            };
            repository.save_sync_state(&state).expect("save sync state");
            assert_eq!(
                repository
                    .get_sync_state("test", "game-001")
                    .unwrap()
                    .unwrap()
                    .cursor,
                Some("cursor-1".to_string())
            );
        });
    }
}
