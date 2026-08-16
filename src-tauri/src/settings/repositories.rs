use super::{
    crypto,
    database::DatabaseError,
    models::{
        ActivityEvent, ActivitySession, ActivitySnapshot, ActivitySourceStatus, ActivityStat,
        ActivityStreak, DatabaseStatus, FrameGenerationProfile, HltbGameData, HltbLocalGame,
        HltbPendingMatch, HltbSettings, HltbSyncStatus, LaunchGame, LocalGame,
        LocalGameAchievements, LocalGameDetails, LocalSteamDetails,
        RapidApiReviewsConfigurationStatus, ReviewsCache, SteamConfigurationStatus,
        SteamCredentials, SteamGameMetrics, SteamGridDbConfigurationStatus, SteamImageSyncResult,
        SteamImageSyncStatus, SteamLaunchGame, SteamLibrarySyncSettings, SteamSyncResult,
        SteamSyncStatus, TranslationConfigurationStatus,
    },
    DatabaseState,
};
use crate::achievements::{
    distribute_total, distribute_unlocked, rarity_from_str, recent, source_hash, summarize,
    Achievement, AchievementDistribution, AchievementDistributions, AchievementSummary,
    GameAchievements, DEFAULT_RECENT_LIMIT,
};
use crate::ai::{self, DEFAULT_MODEL, OPENROUTER_PROVIDER_ID};
use crate::display::{
    DisplayHdrMode, DisplayProfile, DisplayProfileSnapshot, DisplayRefreshRateMode,
    DisplayResolutionMode, PendingDisplayProfileRestore, PendingDisplayRestore,
};
use crate::steam::{
    SteamGameDetails, SteamImageRecord, SteamImageSource, SteamLibraryGame, SteamStat,
};
use crate::steamgriddb::LocalGameIdentity;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

const STEAM_PROVIDER_ID: &str = "steam";
const STEAM_ACCOUNT_ID: &str = "steam-default";
const STEAM_CREDENTIAL_TYPE: &str = "steam_web_api_key";
const HLTB_SYNC_ID: &str = "hltb-default";
const HLTB_SOURCE: &str = "HowLongToBeat Community Database";
const STEAMGRIDDB_PROVIDER_ID: &str = "steamgriddb";
const STEAMGRIDDB_ACCOUNT_ID: &str = "steamgriddb-default";
const STEAMGRIDDB_CREDENTIAL_TYPE: &str = "steamgriddb_api_key";
const RAPIDAPI_REVIEWS_PROVIDER_ID: &str = "rapidapi-reviews";
const RAPIDAPI_REVIEWS_ACCOUNT_ID: &str = "rapidapi-reviews-default";
const RAPIDAPI_REVIEWS_CREDENTIAL_TYPE: &str = "rapidapi_reviews_api_key";
const TRANSLATION_PROVIDER_ID: &str = "google-cloud-translation";
const TRANSLATION_ACCOUNT_ID: &str = "google-cloud-translation-default";
const TRANSLATION_CREDENTIAL_TYPE: &str = "google_cloud_translation_api_key";
const AI_ACCOUNT_ID: &str = "openrouter-default";
const AI_CREDENTIAL_TYPE: &str = "openrouter_api_key";
const AI_CONFIGURATION_SETTING: &str = "ai.configuration";
const TRANSLATION_PROVIDER_SETTING: &str = "news.translation.provider";

fn steam_achievements_source_hash(
    achievements: &[Achievement],
    total: i64,
    stats: &[SteamStat],
) -> String {
    let achievement_hash = source_hash(achievements, total);
    let stats_fingerprint = stats
        .iter()
        .map(|stat| (&stat.name, &stat.value))
        .collect::<Vec<_>>();
    let encoded = serde_json::to_vec(&(achievement_hash, stats_fingerprint)).unwrap_or_default();
    Sha256::digest(encoded)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub struct SettingsRepository<'a> {
    state: &'a DatabaseState,
}

impl<'a> SettingsRepository<'a> {
    pub fn new(state: &'a DatabaseState) -> Self {
        Self { state }
    }

    pub fn get_ai_configuration(
        &self,
    ) -> Result<super::models::AIConfigurationStatus, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let setting = connection
            .query_row(
                "SELECT value_json FROM app_settings WHERE key = ?1",
                [AI_CONFIGURATION_SETTING],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let configuration = setting
            .and_then(|value| serde_json::from_str::<super::models::AIConfiguration>(&value).ok())
            .unwrap_or_else(|| super::models::AIConfiguration {
                provider_id: OPENROUTER_PROVIDER_ID.to_string(),
                model: DEFAULT_MODEL.to_string(),
            });
        let credential = connection
            .query_row(
                "SELECT encrypted_value, masked_suffix FROM provider_credentials
                 WHERE provider_account_id = ?1 AND credential_type = ?2",
                params![AI_ACCOUNT_ID, AI_CREDENTIAL_TYPE],
                |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()?;
        let (configured, masked, available, state) = match credential {
            Some((encrypted, suffix)) => match crypto::unprotect(&encrypted) {
                Ok(_) => (
                    true,
                    suffix.map(|value| format!("************{value}")),
                    true,
                    "configured",
                ),
                Err(_) => (
                    true,
                    suffix.map(|value| format!("************{value}")),
                    false,
                    "credential-unavailable",
                ),
            },
            None => (false, None, false, "not-configured"),
        };
        Ok(super::models::AIConfigurationStatus {
            configuration,
            api_key_configured: configured,
            api_key_masked: masked,
            credential_available: available,
            connection: super::models::AIConnectionStatus {
                state: state.to_string(),
                message: None,
            },
        })
    }

    pub fn get_ai_api_key(&self) -> Result<String, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let encrypted = connection
            .query_row(
                "SELECT encrypted_value FROM provider_credentials
                 WHERE provider_account_id = ?1 AND credential_type = ?2",
                params![AI_ACCOUNT_ID, AI_CREDENTIAL_TYPE],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()?
            .ok_or(DatabaseError::AccountNotConfigured)?;
        crypto::unprotect(&encrypted).map_err(DatabaseError::from)
    }

    pub fn save_ai_configuration(
        &self,
        provider_id: &str,
        model: &str,
        api_key: &str,
    ) -> Result<super::models::AIConfigurationStatus, DatabaseError> {
        if !ai::is_valid_provider(provider_id) {
            return Err(DatabaseError::InvalidAIProvider);
        }
        if !ai::is_valid_model(model) {
            return Err(DatabaseError::InvalidAIModel);
        }
        let normalized_key = if api_key.trim().is_empty() {
            self.get_ai_api_key().ok()
        } else if ai::is_valid_api_key(api_key) {
            Some(api_key.trim().to_string())
        } else {
            return Err(DatabaseError::InvalidAIApiKey);
        };
        let now = timestamp();
        let mut connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO app_settings(key, value_json, schema_version, updated_at)
             VALUES (?1, ?2, 1, ?3)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            params![AI_CONFIGURATION_SETTING, serde_json::json!({"providerId": provider_id, "model": model.trim()}).to_string(), now],
        )?;
        if let Some(key) = normalized_key {
            let encrypted = crypto::protect(&key)?;
            upsert_provider_credential(
                &transaction,
                AI_ACCOUNT_ID,
                AI_CREDENTIAL_TYPE,
                &encrypted,
                &key,
                &now,
            )?;
            transaction.execute(
                "UPDATE provider_accounts SET configuration_status = 'configured', updated_at = ?1 WHERE id = ?2 AND provider_id = ?3",
                params![now, AI_ACCOUNT_ID, OPENROUTER_PROVIDER_ID],
            )?;
        }
        transaction.commit()?;
        drop(connection);
        self.get_ai_configuration()
    }

    pub fn get_provider_configuration(
        &self,
        provider_id: &str,
    ) -> Result<SteamConfigurationStatus, DatabaseError> {
        self.get_provider_configuration_traced(provider_id, "no-correlation-id")
    }

    pub fn get_steamgriddb_configuration(
        &self,
    ) -> Result<SteamGridDbConfigurationStatus, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let enabled = connection
            .query_row(
                "SELECT enabled FROM provider_accounts WHERE id = ?1 AND provider_id = ?2",
                params![STEAMGRIDDB_ACCOUNT_ID, STEAMGRIDDB_PROVIDER_ID],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .map(|value| value != 0)
            .unwrap_or(true);
        let credential = connection
            .query_row(
                "SELECT encrypted_value, masked_suffix FROM provider_credentials
                 WHERE provider_account_id = ?1 AND credential_type = ?2",
                params![STEAMGRIDDB_ACCOUNT_ID, STEAMGRIDDB_CREDENTIAL_TYPE],
                |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()?;
        let (configured, masked, available, status) = match credential {
            Some((encrypted, suffix)) => match crypto::unprotect(&encrypted) {
                Ok(_) => (
                    true,
                    suffix.map(|value| format!("••••••••••••{value}")),
                    true,
                    "configured",
                ),
                Err(_) => (
                    true,
                    suffix.map(|value| format!("••••••••••••{value}")),
                    false,
                    "credential-unavailable",
                ),
            },
            None => (false, None, false, "not-configured"),
        };
        Ok(SteamGridDbConfigurationStatus {
            provider_id: STEAMGRIDDB_PROVIDER_ID.to_string(),
            api_key_configured: configured,
            api_key_masked: masked,
            credential_available: available,
            status: status.to_string(),
            enabled,
        })
    }

    pub fn get_steamgriddb_api_key(&self) -> Result<String, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let encrypted = connection
            .query_row(
                "SELECT encrypted_value FROM provider_credentials
                 WHERE provider_account_id = ?1 AND credential_type = ?2",
                params![STEAMGRIDDB_ACCOUNT_ID, STEAMGRIDDB_CREDENTIAL_TYPE],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()?
            .ok_or(DatabaseError::AccountNotConfigured)?;
        crypto::unprotect(&encrypted).map_err(DatabaseError::from)
    }

    pub fn get_steamgriddb_game_identity(
        &self,
        game_id: &str,
    ) -> Result<LocalGameIdentity, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        connection
            .query_row(
                "SELECT g.id, g.title, g.platform, COALESCE(g.source, ''), g.title_id,
                    (SELECT external_id FROM game_provider_links
                     WHERE game_id = g.id AND provider_id = 'steam' LIMIT 1)
                 FROM games g WHERE g.id = ?1",
                params![game_id],
                |row| {
                    let external_id = row.get::<_, Option<String>>(5)?;
                    Ok(LocalGameIdentity {
                        local_game_id: row.get(0)?,
                        title: row.get(1)?,
                        steam_app_id: external_id.and_then(|value| value.parse::<i64>().ok()),
                        platform: row.get(2)?,
                        source: row.get(3)?,
                        title_id: row.get(4)?,
                    })
                },
            )
            .optional()?
            .ok_or(DatabaseError::GameNotFound)
    }

    pub fn save_steamgriddb_api_key(
        &self,
        api_key: &str,
    ) -> Result<SteamGridDbConfigurationStatus, DatabaseError> {
        let api_key = normalize_steamgriddb_api_key(api_key)?;
        let encrypted = crypto::protect(&api_key)?;
        let now = timestamp();
        let mut connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.transaction()?;
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM provider_accounts WHERE id = ?1 AND provider_id = ?2)",
            params![STEAMGRIDDB_ACCOUNT_ID, STEAMGRIDDB_PROVIDER_ID],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(DatabaseError::AccountNotConfigured);
        }
        upsert_provider_credential(
            &transaction,
            STEAMGRIDDB_ACCOUNT_ID,
            STEAMGRIDDB_CREDENTIAL_TYPE,
            &encrypted,
            &api_key,
            &now,
        )?;
        transaction.execute(
            "UPDATE provider_accounts SET configuration_status = 'configured', updated_at = ?1
             WHERE id = ?2 AND provider_id = ?3",
            params![now, STEAMGRIDDB_ACCOUNT_ID, STEAMGRIDDB_PROVIDER_ID],
        )?;
        transaction.commit()?;
        drop(connection);
        self.get_steamgriddb_configuration()
    }

    pub fn delete_steamgriddb_api_key(
        &self,
    ) -> Result<SteamGridDbConfigurationStatus, DatabaseError> {
        let now = timestamp();
        let mut connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "DELETE FROM provider_credentials WHERE provider_account_id = ?1 AND credential_type = ?2",
            params![STEAMGRIDDB_ACCOUNT_ID, STEAMGRIDDB_CREDENTIAL_TYPE],
        )?;
        transaction.execute(
            "UPDATE provider_accounts SET configuration_status = 'not-configured', updated_at = ?1
             WHERE id = ?2 AND provider_id = ?3",
            params![now, STEAMGRIDDB_ACCOUNT_ID, STEAMGRIDDB_PROVIDER_ID],
        )?;
        transaction.commit()?;
        drop(connection);
        self.get_steamgriddb_configuration()
    }

    pub fn get_rapidapi_reviews_configuration(
        &self,
    ) -> Result<RapidApiReviewsConfigurationStatus, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let enabled = connection
            .query_row(
                "SELECT enabled FROM provider_accounts WHERE id = ?1 AND provider_id = ?2",
                params![RAPIDAPI_REVIEWS_ACCOUNT_ID, RAPIDAPI_REVIEWS_PROVIDER_ID],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .map(|value| value != 0)
            .unwrap_or(true);
        let credential = connection
            .query_row(
                "SELECT encrypted_value, masked_suffix FROM provider_credentials
                 WHERE provider_account_id = ?1 AND credential_type = ?2",
                params![
                    RAPIDAPI_REVIEWS_ACCOUNT_ID,
                    RAPIDAPI_REVIEWS_CREDENTIAL_TYPE
                ],
                |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()?;
        let (configured, masked, available, status) = match credential {
            Some((encrypted, suffix)) => match crypto::unprotect(&encrypted) {
                Ok(_) => (
                    true,
                    suffix.map(|value| format!("************{value}")),
                    true,
                    "configured",
                ),
                Err(_) => (
                    true,
                    suffix.map(|value| format!("************{value}")),
                    false,
                    "credential-unavailable",
                ),
            },
            None => (false, None, false, "not-configured"),
        };
        Ok(RapidApiReviewsConfigurationStatus {
            provider_id: RAPIDAPI_REVIEWS_PROVIDER_ID.to_string(),
            api_key_configured: configured,
            api_key_masked: masked,
            credential_available: available,
            status: status.to_string(),
            enabled,
        })
    }

    pub fn get_rapidapi_reviews_api_key(&self) -> Result<String, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let encrypted = connection
            .query_row(
                "SELECT encrypted_value FROM provider_credentials
                 WHERE provider_account_id = ?1 AND credential_type = ?2",
                params![
                    RAPIDAPI_REVIEWS_ACCOUNT_ID,
                    RAPIDAPI_REVIEWS_CREDENTIAL_TYPE
                ],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()?
            .ok_or(DatabaseError::AccountNotConfigured)?;
        crypto::unprotect(&encrypted).map_err(DatabaseError::from)
    }

    pub fn get_reviews_cache(&self, game_id: &str) -> Result<Option<ReviewsCache>, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        connection
            .query_row(
                "SELECT steam_app_id, metacritic_json, opencritic_json, steam_json, steam_updated_at
                 FROM game_reviews_cache WHERE game_id = ?1",
                params![game_id],
                |row| {
                    Ok(ReviewsCache {
                        steam_app_id: row.get(0)?,
                        metacritic_json: row.get(1)?,
                        opencritic_json: row.get(2)?,
                        steam_json: row.get(3)?,
                        steam_updated_at: row.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(DatabaseError::from)
    }

    pub fn get_game_review_consensus(
        &self,
        game_id: &str,
    ) -> Result<Option<crate::consensus::GameReviewConsensus>, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let payload = connection
            .query_row(
                "SELECT consensus_json FROM game_review_consensus WHERE game_id = ?1",
                params![game_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        payload
            .map(|value| {
                serde_json::from_str(&value).map_err(|_| DatabaseError::ConsensusDataInvalid)
            })
            .transpose()
    }

    pub fn save_game_review_consensus(
        &self,
        consensus: &crate::consensus::GameReviewConsensus,
    ) -> Result<(), DatabaseError> {
        let now = chrono::Utc::now().to_rfc3339();
        let payload =
            serde_json::to_string(consensus).map_err(|_| DatabaseError::ConsensusDataInvalid)?;
        let mut connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO game_review_consensus(
                game_id, consensus_json, generated_at, prompt_version,
                provider_id, model_id, input_fingerprint, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(game_id) DO UPDATE SET
                consensus_json = excluded.consensus_json,
                generated_at = excluded.generated_at,
                prompt_version = excluded.prompt_version,
                provider_id = excluded.provider_id,
                model_id = excluded.model_id,
                input_fingerprint = excluded.input_fingerprint,
                updated_at = excluded.updated_at",
            params![
                consensus.game_id,
                payload,
                consensus.generated_at,
                consensus.prompt_version,
                consensus.provider_id,
                consensus.model_id,
                consensus.input_fingerprint,
                now,
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn save_reviews_provider_cache(
        &self,
        game_id: &str,
        steam_app_id: i64,
        provider: &str,
        payload_json: &str,
    ) -> Result<(), DatabaseError> {
        let now = chrono::Local::now().to_rfc3339();
        let mut connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT OR IGNORE INTO game_reviews_cache(
                game_id, steam_app_id, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?3)",
            params![game_id, steam_app_id, now],
        )?;
        let update_sql = match provider {
            "metacritic" => {
                "UPDATE game_reviews_cache
                 SET steam_app_id = ?1, metacritic_json = ?2,
                     metacritic_updated_at = ?3, updated_at = ?3
                 WHERE game_id = ?4"
            }
            "opencritic" => {
                "UPDATE game_reviews_cache
                 SET steam_app_id = ?1, opencritic_json = ?2,
                     opencritic_updated_at = ?3, updated_at = ?3
                 WHERE game_id = ?4"
            }
            "steam" => {
                "UPDATE game_reviews_cache
                 SET steam_app_id = ?1, steam_json = ?2,
                     steam_updated_at = ?3, updated_at = ?3
                 WHERE game_id = ?4"
            }
            _ => return Err(DatabaseError::UnsupportedProvider),
        };
        transaction.execute(
            update_sql,
            params![steam_app_id, payload_json, now, game_id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn save_rapidapi_reviews_api_key(
        &self,
        api_key: &str,
    ) -> Result<RapidApiReviewsConfigurationStatus, DatabaseError> {
        let api_key = normalize_rapidapi_reviews_api_key(api_key)?;
        let encrypted = crypto::protect(&api_key)?;
        let now = timestamp();
        let mut connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.transaction()?;
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM provider_accounts WHERE id = ?1 AND provider_id = ?2)",
            params![RAPIDAPI_REVIEWS_ACCOUNT_ID, RAPIDAPI_REVIEWS_PROVIDER_ID],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(DatabaseError::AccountNotConfigured);
        }
        upsert_provider_credential(
            &transaction,
            RAPIDAPI_REVIEWS_ACCOUNT_ID,
            RAPIDAPI_REVIEWS_CREDENTIAL_TYPE,
            &encrypted,
            &api_key,
            &now,
        )?;
        transaction.execute(
            "UPDATE provider_accounts SET configuration_status = 'configured', updated_at = ?1
             WHERE id = ?2 AND provider_id = ?3",
            params![
                now,
                RAPIDAPI_REVIEWS_ACCOUNT_ID,
                RAPIDAPI_REVIEWS_PROVIDER_ID
            ],
        )?;
        transaction.commit()?;
        drop(connection);
        self.get_rapidapi_reviews_configuration()
    }

    pub fn delete_rapidapi_reviews_api_key(
        &self,
    ) -> Result<RapidApiReviewsConfigurationStatus, DatabaseError> {
        let now = timestamp();
        let mut connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "DELETE FROM provider_credentials WHERE provider_account_id = ?1 AND credential_type = ?2",
            params![RAPIDAPI_REVIEWS_ACCOUNT_ID, RAPIDAPI_REVIEWS_CREDENTIAL_TYPE],
        )?;
        transaction.execute(
            "UPDATE provider_accounts SET configuration_status = 'not-configured', updated_at = ?1
             WHERE id = ?2 AND provider_id = ?3",
            params![
                now,
                RAPIDAPI_REVIEWS_ACCOUNT_ID,
                RAPIDAPI_REVIEWS_PROVIDER_ID
            ],
        )?;
        transaction.commit()?;
        drop(connection);
        self.get_rapidapi_reviews_configuration()
    }

    pub fn get_translation_configuration(
        &self,
    ) -> Result<TranslationConfigurationStatus, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let enabled = connection
            .query_row(
                "SELECT enabled FROM provider_accounts WHERE id = ?1 AND provider_id = ?2",
                params![TRANSLATION_ACCOUNT_ID, TRANSLATION_PROVIDER_ID],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .map(|value| value != 0)
            .unwrap_or(true);
        let credential = connection
            .query_row(
                "SELECT encrypted_value, masked_suffix FROM provider_credentials
                 WHERE provider_account_id = ?1 AND credential_type = ?2",
                params![TRANSLATION_ACCOUNT_ID, TRANSLATION_CREDENTIAL_TYPE],
                |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()?;
        let (configured, masked, available, status) = match credential {
            Some((encrypted, suffix)) => match crypto::unprotect(&encrypted) {
                Ok(_) => (
                    true,
                    suffix.map(|value| format!("************{value}")),
                    true,
                    "configured",
                ),
                Err(_) => (
                    true,
                    suffix.map(|value| format!("************{value}")),
                    false,
                    "credential-unavailable",
                ),
            },
            None => (false, None, false, "not-configured"),
        };
        Ok(TranslationConfigurationStatus {
            provider_id: TRANSLATION_PROVIDER_ID.to_string(),
            api_key_configured: configured,
            api_key_masked: masked,
            credential_available: available,
            status: status.to_string(),
            enabled,
        })
    }

    pub fn get_translation_api_key(&self) -> Result<String, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let encrypted = connection
            .query_row(
                "SELECT encrypted_value FROM provider_credentials
                 WHERE provider_account_id = ?1 AND credential_type = ?2",
                params![TRANSLATION_ACCOUNT_ID, TRANSLATION_CREDENTIAL_TYPE],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()?
            .ok_or(DatabaseError::AccountNotConfigured)?;
        crypto::unprotect(&encrypted).map_err(DatabaseError::from)
    }

    pub fn save_translation_api_key(
        &self,
        api_key: &str,
    ) -> Result<TranslationConfigurationStatus, DatabaseError> {
        let api_key = normalize_translation_api_key(api_key)?;
        let encrypted = crypto::protect(&api_key)?;
        let now = timestamp();
        let mut connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.transaction()?;
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM provider_accounts WHERE id = ?1 AND provider_id = ?2)",
            params![TRANSLATION_ACCOUNT_ID, TRANSLATION_PROVIDER_ID],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(DatabaseError::AccountNotConfigured);
        }
        upsert_provider_credential(
            &transaction,
            TRANSLATION_ACCOUNT_ID,
            TRANSLATION_CREDENTIAL_TYPE,
            &encrypted,
            &api_key,
            &now,
        )?;
        transaction.execute(
            "UPDATE provider_accounts SET configuration_status = 'configured', updated_at = ?1
             WHERE id = ?2 AND provider_id = ?3",
            params![now, TRANSLATION_ACCOUNT_ID, TRANSLATION_PROVIDER_ID],
        )?;
        transaction.commit()?;
        drop(connection);
        self.get_translation_configuration()
    }

    pub fn delete_translation_api_key(
        &self,
    ) -> Result<TranslationConfigurationStatus, DatabaseError> {
        let now = timestamp();
        let mut connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "DELETE FROM provider_credentials WHERE provider_account_id = ?1 AND credential_type = ?2",
            params![TRANSLATION_ACCOUNT_ID, TRANSLATION_CREDENTIAL_TYPE],
        )?;
        transaction.execute(
            "UPDATE provider_accounts SET configuration_status = 'not-configured', updated_at = ?1
             WHERE id = ?2 AND provider_id = ?3",
            params![now, TRANSLATION_ACCOUNT_ID, TRANSLATION_PROVIDER_ID],
        )?;
        transaction.commit()?;
        drop(connection);
        self.get_translation_configuration()
    }

    pub fn get_translation_provider_selection(&self) -> Result<Option<String>, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let value = connection
            .query_row(
                "SELECT value_json FROM app_settings WHERE key = ?1",
                params![TRANSLATION_PROVIDER_SETTING],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        value
            .map(|value| {
                serde_json::from_str::<String>(&value)
                    .map_err(|_| DatabaseError::UnsupportedProvider)
            })
            .transpose()
    }

    pub fn set_translation_provider_selection(
        &self,
        provider_id: &str,
    ) -> Result<String, DatabaseError> {
        if !matches!(provider_id, "google-public" | "google-cloud-translation") {
            return Err(DatabaseError::UnsupportedProvider);
        }
        let value_json =
            serde_json::to_string(provider_id).map_err(|_| DatabaseError::UnsupportedProvider)?;
        let now = timestamp();
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        connection.execute(
            "INSERT INTO app_settings(key, value_json, schema_version, updated_at)
             VALUES (?1, ?2, 1, ?3)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json,
               schema_version = excluded.schema_version, updated_at = excluded.updated_at",
            params![TRANSLATION_PROVIDER_SETTING, value_json, now],
        )?;
        Ok(provider_id.to_string())
    }

    pub fn get_steam_credentials(&self) -> Result<SteamCredentials, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let steam_id64 = connection
            .query_row(
                "SELECT external_account_id FROM provider_accounts WHERE id = ?1 AND provider_id = ?2 AND external_account_id IS NOT NULL",
                params![STEAM_ACCOUNT_ID, STEAM_PROVIDER_ID],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or(DatabaseError::AccountNotConfigured)?;
        let encrypted = connection
            .query_row(
                "SELECT encrypted_value FROM provider_credentials WHERE provider_account_id = ?1 AND credential_type = ?2",
                params![STEAM_ACCOUNT_ID, STEAM_CREDENTIAL_TYPE],
                |row| row.get::<_, Vec<u8>>(0),
            )
            .optional()?
            .ok_or(DatabaseError::AccountNotConfigured)?;
        let api_key = crypto::unprotect(&encrypted)?;
        Ok(SteamCredentials {
            steam_id64,
            api_key,
        })
    }

    pub fn get_steam_cache(&self) -> Result<HashMap<i64, (i64, i64)>, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let mut statement = connection.prepare(
            "SELECT steam_app_id, COALESCE(steam_total_playtime_minutes, 0),
                CASE
                    WHEN (
                        NULLIF(TRIM(COALESCE(steam_description, '')), '') IS NULL
                        OR NULLIF(TRIM(COALESCE(steam_short_description, '')), '') IS NULL
                        OR NULLIF(TRIM(COALESCE(steam_release_date, '')), '') IS NULL
                        OR NULLIF(TRIM(COALESCE(steam_controller_support, '')), '') IS NULL
                        OR steam_multiplayer IS NULL
                        OR steam_single_player IS NULL
                        OR steam_cloud IS NULL
                    )
                    THEN '0'
                    ELSE COALESCE(steam_updated_at, '0')
                END
             FROM game_details WHERE steam_app_id IS NOT NULL",
        )?;
        let rows = statement.query_map([], |row| {
            let app_id: i64 = row.get(0)?;
            let playtime: i64 = row.get(1)?;
            let updated_at = row.get::<_, String>(2)?.parse::<i64>().unwrap_or(0);
            Ok((app_id, (playtime, updated_at)))
        })?;
        let mut cache = HashMap::new();
        for row in rows {
            let (app_id, value) = row?;
            cache.insert(app_id, value);
        }
        Ok(cache)
    }

    pub fn get_steam_game_for_metadata(
        &self,
        game_id: &str,
    ) -> Result<Option<SteamLibraryGame>, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        connection
            .query_row(
                "SELECT g.title, d.steam_app_id,
                    COALESCE(d.steam_total_playtime_minutes, 0),
                    d.steam_playtime_2weeks_minutes, d.steam_last_played_at,
                    COALESCE(d.steam_installed, g.installed, 0),
                    d.steam_has_community_visible_stats, d.steam_icon_url,
                    COALESCE(
                        d.steam_logo_url,
                        (SELECT source_url FROM steam_game_assets
                         WHERE game_id = g.id AND asset_type = 'logo'
                         ORDER BY updated_at DESC LIMIT 1),
                        'https://cdn.cloudflare.steamstatic.com/steam/apps/' || d.steam_app_id || '/logo.png'
                    )
                 FROM games g
                 JOIN game_provider_links link ON link.game_id = g.id
                 JOIN game_details d ON d.game_id = g.id
                 WHERE g.id = ?1 AND link.provider_id = ?2
                 LIMIT 1",
                params![game_id, STEAM_PROVIDER_ID],
                |row| {
                    Ok(SteamLibraryGame {
                        app_id: row.get(1)?,
                        name: row.get(0)?,
                        total_playtime_minutes: row.get(2)?,
                        playtime_2weeks_minutes: row.get(3)?,
                        last_played_at: row.get(4)?,
                        installed: Some(row.get::<_, i64>(5)? != 0),
                        has_community_visible_stats: row
                            .get::<_, Option<i64>>(6)?
                            .map(|value| value != 0),
                        icon_url: row.get(7)?,
                        logo_url: row.get(8)?,
                        should_persist: true,
                        details: None,
                    })
                },
            )
            .optional()
            .map_err(DatabaseError::from)
    }

    pub fn sync_steam_game_metadata(&self, game: &SteamLibraryGame) -> Result<(), DatabaseError> {
        let details = game
            .details
            .as_ref()
            .filter(|details| details.complete)
            .ok_or(DatabaseError::SteamMetadataUnavailable)?;
        let app_id = game.app_id.to_string();
        let now = timestamp();
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.unchecked_transaction()?;
        let game_id: String = transaction
            .query_row(
                "SELECT game_id FROM game_provider_links
                 WHERE provider_id = ?1 AND external_id = ?2",
                params![STEAM_PROVIDER_ID, app_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(DatabaseError::GameNotFound)?;

        upsert_steam_details(&transaction, &game_id, game, details, &now)?;
        replace_steam_child_rows(&transaction, &game_id, details, false)?;
        if let Some(installed) = game.installed {
            transaction.execute(
                "UPDATE game_details SET steam_installed = ?1 WHERE game_id = ?2",
                params![bool_value(Some(installed)), game_id],
            )?;
        }
        transaction.execute(
            "UPDATE games SET updated_at = ?1 WHERE id = ?2",
            params![now, game_id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn update_steam_active_players(
        &self,
        game_id: &str,
        active_players: i64,
    ) -> Result<(), DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let updated = connection.execute(
            "UPDATE game_details SET steam_active_players = ?1, steam_metrics_updated_at = ?2 WHERE game_id = ?3",
            params![active_players, timestamp(), game_id],
        )?;
        if updated == 0 {
            return Err(DatabaseError::GameNotFound);
        }
        Ok(())
    }

    pub fn get_steam_game_metrics(&self, game_id: &str) -> Result<SteamGameMetrics, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        steam_game_metrics(&connection, game_id)?.ok_or(DatabaseError::GameNotFound)
    }

    pub fn get_game_activity(&self, game_id: &str) -> Result<ActivitySnapshot, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM games WHERE id = ?1)",
            params![game_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(DatabaseError::GameNotFound);
        }

        let metrics = steam_game_metrics(&connection, game_id)?;

        let mut session_statement = connection.prepare(
            "SELECT id, started_at, ended_at, duration_seconds, status, source
             FROM game_sessions
             WHERE game_id = ?1
             ORDER BY CAST(started_at AS INTEGER) DESC, id DESC
             LIMIT 20",
        )?;
        let sessions = session_statement
            .query_map(params![game_id], |row| {
                Ok(ActivitySession {
                    id: row.get(0)?,
                    started_at: row.get(1)?,
                    ended_at: row.get(2)?,
                    duration_seconds: row.get(3)?,
                    status: row.get(4)?,
                    source: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut event_statement = connection.prepare(
            "SELECT id, event_type, occurred_at, title, description, value_json, source
             FROM game_activity_events
             WHERE game_id = ?1
             ORDER BY CAST(occurred_at AS INTEGER) DESC, id DESC
             LIMIT 40",
        )?;
        let mut events = event_statement
            .query_map(params![game_id], |row| {
                let value_json = row.get::<_, Option<String>>(5)?;
                Ok(ActivityEvent {
                    id: format!("local:{}", row.get::<_, i64>(0)?),
                    event_type: row.get(1)?,
                    occurred_at: row.get(2)?,
                    title: row.get(3)?,
                    description: row.get(4)?,
                    value: value_json.and_then(|value| serde_json::from_str(&value).ok()),
                    source: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut achievement_statement = connection.prepare(
            "SELECT api_name, display_name, description, unlock_time
             FROM steam_game_achievements
             WHERE game_id = ?1 AND achieved = 1 AND unlock_time IS NOT NULL
             ORDER BY CAST(unlock_time AS INTEGER) DESC
             LIMIT 40",
        )?;
        for achievement in achievement_statement.query_map(params![game_id], |row| {
            let api_name: String = row.get(0)?;
            let title: Option<String> = row.get(1)?;
            Ok(ActivityEvent {
                id: format!("steam-achievement:{api_name}"),
                event_type: "achievement_unlocked".to_string(),
                occurred_at: row.get(3)?,
                title: title.unwrap_or(api_name),
                description: row.get(2)?,
                value: None,
                source: "steam".to_string(),
            })
        })? {
            events.push(achievement?);
        }
        events.sort_by(|left, right| {
            activity_timestamp(&right.occurred_at)
                .cmp(&activity_timestamp(&left.occurred_at))
                .then_with(|| left.id.cmp(&right.id))
        });
        events.truncate(40);

        let mut stat_statement = connection.prepare(
            "SELECT name, value_json
             FROM steam_game_stats
             WHERE game_id = ?1
             ORDER BY name
             LIMIT 12",
        )?;
        let stats = stat_statement
            .query_map(params![game_id], |row| {
                let key: String = row.get(0)?;
                let value_json: String = row.get(1)?;
                let value = serde_json::from_str(&value_json)
                    .unwrap_or_else(|_| serde_json::Value::String(value_json.clone()));
                Ok(ActivityStat {
                    label: activity_stat_label(&key),
                    key,
                    value,
                    source: "steam".to_string(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let status = if sessions.is_empty() && events.is_empty() && stats.is_empty() {
            "no-data"
        } else {
            "ready"
        };
        let streak = activity_streak(&sessions);
        Ok(ActivitySnapshot {
            status: status.to_string(),
            metrics,
            last_session: sessions.first().cloned(),
            sessions,
            events,
            stats,
            streak,
            friends: Vec::new(),
            friends_status: "unavailable".to_string(),
            sources: vec![
                ActivitySourceStatus {
                    source: "local".to_string(),
                    status: if status == "no-data" {
                        "no-data".to_string()
                    } else {
                        "ready".to_string()
                    },
                    error: None,
                },
                ActivitySourceStatus {
                    source: "steam".to_string(),
                    status: "pending".to_string(),
                    error: None,
                },
            ],
        })
    }

    pub fn get_steam_app_id(&self, game_id: &str) -> Result<Option<i64>, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        connection
            .query_row(
                "SELECT steam_app_id FROM game_details WHERE game_id = ?1",
                params![game_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(DatabaseError::from)
    }

    pub fn get_steam_launch_game(
        &self,
        game_id: &str,
    ) -> Result<Option<SteamLaunchGame>, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        connection
            .query_row(
                "SELECT g.provider, g.platform,
                        COALESCE(d.steam_installed, g.installed), d.steam_app_id
                 FROM games g
                 LEFT JOIN game_details d ON d.game_id = g.id
                 WHERE g.id = ?1",
                params![game_id],
                |row| {
                    Ok(SteamLaunchGame {
                        provider: row.get(0)?,
                        platform: row.get(1)?,
                        installed: row.get(2)?,
                        steam_app_id: row.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(DatabaseError::from)
    }

    pub fn get_launch_game(&self, game_id: &str) -> Result<Option<LaunchGame>, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        connection
            .query_row(
                "SELECT g.provider, g.platform, g.installed, d.steam_app_id,
                        g.source, g.emulator_id, g.game_path, g.title_id
                 FROM games g
                 LEFT JOIN game_details d ON d.game_id = g.id
                 WHERE g.id = ?1",
                params![game_id],
                |row| {
                    Ok(LaunchGame {
                        provider: row.get(0)?,
                        platform: row.get(1)?,
                        installed: row.get(2)?,
                        steam_app_id: row.get(3)?,
                        source: row.get(4)?,
                        emulator_id: row.get(5)?,
                        game_path: row.get(6)?,
                        title_id: row.get(7)?,
                    })
                },
            )
            .optional()
            .map_err(DatabaseError::from)
    }

    pub fn get_display_profile(&self, game_id: &str) -> Result<DisplayProfile, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        connection
            .query_row(
                "SELECT enabled, display_id, device_name, width, height, refresh_rate,
                        restore_on_exit, updated_at, resolution_mode, refresh_rate_mode, hdr_mode,
                        rtx_hdr_preset, rtx_hdr_peak_nits
                 FROM game_display_profiles WHERE game_id = ?1",
                params![game_id],
                |row| {
                    Ok(DisplayProfile {
                        game_id: game_id.to_string(),
                        enabled: row.get::<_, i64>(0)? != 0,
                        display_id: row.get(1)?,
                        device_name: row.get(2)?,
                        width: row.get::<_, Option<i64>>(3)?.map(|value| value as u32),
                        height: row.get::<_, Option<i64>>(4)?.map(|value| value as u32),
                        refresh_rate: row.get::<_, Option<i64>>(5)?.map(|value| value as u32),
                        restore_on_exit: row.get::<_, i64>(6)? != 0,
                        updated_at: row.get(7)?,
                        resolution_mode: parse_resolution_mode(&row.get::<_, String>(8)?),
                        refresh_rate_mode: parse_refresh_rate_mode(&row.get::<_, String>(9)?),
                        hdr_mode: parse_hdr_mode(&row.get::<_, String>(10)?),
                        rtx_hdr_preset: parse_rtx_hdr_preset(&row.get::<_, Option<String>>(11)?),
                        rtx_hdr_peak_nits: row.get::<_, i64>(12)? as u32,
                    })
                },
            )
            .optional()
            .map(|profile| {
                profile.unwrap_or(DisplayProfile {
                    game_id: game_id.to_string(),
                    enabled: false,
                    display_id: None,
                    device_name: None,
                    width: None,
                    height: None,
                    refresh_rate: None,
                    restore_on_exit: true,
                    updated_at: None,
                    resolution_mode: DisplayResolutionMode::System,
                    refresh_rate_mode: DisplayRefreshRateMode::System,
                    hdr_mode: DisplayHdrMode::System,
                    rtx_hdr_preset: None,
                    rtx_hdr_peak_nits: crate::rtx_hdr::RTX_HDR_PEAK_NITS_DEFAULT,
                })
            })
            .map_err(DatabaseError::from)
    }

    pub fn save_display_profile(
        &self,
        profile: &DisplayProfile,
    ) -> Result<DisplayProfile, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let now = timestamp();
        connection.execute(
            "INSERT INTO game_display_profiles(
                game_id, enabled, display_id, device_name, width, height, refresh_rate,
                restore_on_exit, updated_at, resolution_mode, refresh_rate_mode, hdr_mode,
                rtx_hdr_preset, rtx_hdr_peak_nits
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
             ON CONFLICT(game_id) DO UPDATE SET
                enabled = excluded.enabled,
                display_id = excluded.display_id,
                device_name = excluded.device_name,
                width = excluded.width,
                height = excluded.height,
                refresh_rate = excluded.refresh_rate,
                restore_on_exit = excluded.restore_on_exit,
                updated_at = excluded.updated_at,
                resolution_mode = excluded.resolution_mode,
                refresh_rate_mode = excluded.refresh_rate_mode,
                hdr_mode = excluded.hdr_mode,
                rtx_hdr_preset = excluded.rtx_hdr_preset,
                rtx_hdr_peak_nits = excluded.rtx_hdr_peak_nits",
            params![
                profile.game_id,
                bool_value(Some(profile.enabled)),
                profile.display_id,
                profile.device_name,
                profile.width.map(i64::from),
                profile.height.map(i64::from),
                profile.refresh_rate.map(i64::from),
                bool_value(Some(profile.restore_on_exit)),
                now,
                display_resolution_mode_name(profile.resolution_mode),
                display_refresh_rate_mode_name(profile.refresh_rate_mode),
                display_hdr_mode_name(profile.hdr_mode),
                profile.rtx_hdr_preset.map(rtx_hdr_preset_name),
                i64::from(profile.rtx_hdr_peak_nits),
            ],
        )?;
        drop(connection);
        self.get_display_profile(&profile.game_id)
    }

    pub fn reset_display_profile(&self, game_id: &str) -> Result<(), DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        connection.execute(
            "DELETE FROM game_display_profiles WHERE game_id = ?1",
            params![game_id],
        )?;
        Ok(())
    }

    pub fn get_frame_generation_profile(
        &self,
        game_id: &str,
    ) -> Result<FrameGenerationProfile, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        connection
            .query_row(
                "SELECT provider, enabled, mode, multiplier, auto_scale,
                        auto_scale_delay, target_executable, updated_at
                 FROM game_frame_generation_profiles WHERE game_id = ?1",
                params![game_id],
                |row| {
                    Ok(FrameGenerationProfile {
                        game_id: game_id.to_string(),
                        provider: row.get(0)?,
                        enabled: row.get::<_, i64>(1)? != 0,
                        mode: row.get(2)?,
                        multiplier: row.get(3)?,
                        auto_scale: row.get::<_, i64>(4)? != 0,
                        auto_scale_delay: row.get(5)?,
                        target_executable: row.get(6)?,
                        updated_at: row.get(7)?,
                        restart_required: false,
                    })
                },
            )
            .optional()
            .map(|profile| profile.unwrap_or_else(|| FrameGenerationProfile::off(game_id)))
            .map_err(DatabaseError::from)
    }

    pub fn save_frame_generation_profile(
        &self,
        profile: &FrameGenerationProfile,
    ) -> Result<FrameGenerationProfile, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let now = timestamp();
        connection.execute(
            "INSERT INTO game_frame_generation_profiles(
                game_id, provider, enabled, mode, multiplier, auto_scale,
                auto_scale_delay, target_executable, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(game_id) DO UPDATE SET
                provider = excluded.provider,
                enabled = excluded.enabled,
                mode = excluded.mode,
                multiplier = excluded.multiplier,
                auto_scale = excluded.auto_scale,
                auto_scale_delay = excluded.auto_scale_delay,
                target_executable = excluded.target_executable,
                updated_at = excluded.updated_at",
            params![
                profile.game_id,
                profile.provider,
                bool_value(Some(profile.enabled)),
                profile.mode,
                profile.multiplier,
                bool_value(Some(profile.auto_scale)),
                profile.auto_scale_delay,
                profile.target_executable,
                now,
            ],
        )?;
        drop(connection);
        self.get_frame_generation_profile(&profile.game_id)
    }

    pub fn set_frame_generation_target(
        &self,
        game_id: &str,
        target_executable: &str,
    ) -> Result<FrameGenerationProfile, DatabaseError> {
        let current = self.get_frame_generation_profile(game_id)?;
        self.save_frame_generation_profile(&FrameGenerationProfile {
            target_executable: Some(target_executable.to_string()),
            ..current
        })
    }

    pub fn get_pending_display_restore(
        &self,
    ) -> Result<Option<PendingDisplayRestore>, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        connection
            .query_row(
                "SELECT display_id, width, height, refresh_rate, created_at
                 FROM pending_display_restore WHERE id = 1",
                [],
                |row| {
                    Ok(PendingDisplayRestore {
                        display_id: row.get(0)?,
                        width: row.get::<_, i64>(1)? as u32,
                        height: row.get::<_, i64>(2)? as u32,
                        refresh_rate: row.get::<_, i64>(3)? as u32,
                        created_at: row.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(DatabaseError::from)
    }

    pub fn save_pending_display_restore(
        &self,
        pending: &PendingDisplayRestore,
    ) -> Result<(), DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        connection.execute(
            "INSERT INTO pending_display_restore(id, display_id, width, height, refresh_rate, created_at)
             VALUES (1, ?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
                display_id = excluded.display_id,
                width = excluded.width,
                height = excluded.height,
                refresh_rate = excluded.refresh_rate,
                created_at = excluded.created_at",
            params![
                pending.display_id,
                i64::from(pending.width),
                i64::from(pending.height),
                i64::from(pending.refresh_rate),
                pending.created_at,
            ],
        )?;
        Ok(())
    }

    pub fn clear_pending_display_restore(&self) -> Result<(), DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        connection.execute("DELETE FROM pending_display_restore WHERE id = 1", [])?;
        Ok(())
    }

    pub fn get_pending_display_profile_restore(
        &self,
    ) -> Result<Option<PendingDisplayProfileRestore>, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        connection
            .query_row(
                "SELECT session_id, game_id, display_id, width, height, refresh_rate,
                        hdr_enabled, captured_at, changed_resolution,
                        changed_refresh_rate, changed_hdr, rtx_hdr_snapshot_json,
                        auto_hdr_snapshot_json, rtx_hdr_executable, changed_rtx_hdr,
                        changed_auto_hdr
                 FROM pending_display_profile_restore WHERE id = 1",
                [],
                |row| {
                    Ok(PendingDisplayProfileRestore {
                        session_id: row.get(0)?,
                        game_id: row.get(1)?,
                        snapshot: DisplayProfileSnapshot {
                            display_id: row.get(2)?,
                            width: row.get::<_, i64>(3)? as u32,
                            height: row.get::<_, i64>(4)? as u32,
                            refresh_rate: row.get::<_, i64>(5)? as u32,
                            hdr_enabled: row.get::<_, i64>(6)? != 0,
                            captured_at: row.get(7)?,
                        },
                        changed_resolution: row.get::<_, i64>(8)? != 0,
                        changed_refresh_rate: row.get::<_, i64>(9)? != 0,
                        changed_hdr: row.get::<_, i64>(10)? != 0,
                        rtx_hdr_snapshot: row.get(11)?,
                        auto_hdr_snapshot: row.get(12)?,
                        rtx_hdr_executable: row.get(13)?,
                        changed_rtx_hdr: row.get::<_, i64>(14)? != 0,
                        changed_auto_hdr: row.get::<_, i64>(15)? != 0,
                    })
                },
            )
            .optional()
            .map_err(DatabaseError::from)
    }

    pub fn save_pending_display_profile_restore(
        &self,
        pending: &PendingDisplayProfileRestore,
    ) -> Result<(), DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        connection.execute(
            "INSERT INTO pending_display_profile_restore(
                id, session_id, game_id, display_id, width, height, refresh_rate,
                hdr_enabled, captured_at, changed_resolution, changed_refresh_rate, changed_hdr,
                rtx_hdr_snapshot_json, auto_hdr_snapshot_json, rtx_hdr_executable,
                changed_rtx_hdr, changed_auto_hdr
             ) VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
             ON CONFLICT(id) DO UPDATE SET
                session_id = excluded.session_id,
                game_id = excluded.game_id,
                display_id = excluded.display_id,
                width = excluded.width,
                height = excluded.height,
                refresh_rate = excluded.refresh_rate,
                hdr_enabled = excluded.hdr_enabled,
                captured_at = excluded.captured_at,
                changed_resolution = excluded.changed_resolution,
                changed_refresh_rate = excluded.changed_refresh_rate,
                changed_hdr = excluded.changed_hdr,
                rtx_hdr_snapshot_json = excluded.rtx_hdr_snapshot_json,
                auto_hdr_snapshot_json = excluded.auto_hdr_snapshot_json,
                rtx_hdr_executable = excluded.rtx_hdr_executable,
                changed_rtx_hdr = excluded.changed_rtx_hdr,
                changed_auto_hdr = excluded.changed_auto_hdr",
            params![
                pending.session_id,
                pending.game_id,
                pending.snapshot.display_id,
                i64::from(pending.snapshot.width),
                i64::from(pending.snapshot.height),
                i64::from(pending.snapshot.refresh_rate),
                bool_value(Some(pending.snapshot.hdr_enabled)),
                pending.snapshot.captured_at,
                bool_value(Some(pending.changed_resolution)),
                bool_value(Some(pending.changed_refresh_rate)),
                bool_value(Some(pending.changed_hdr)),
                pending.rtx_hdr_snapshot,
                pending.auto_hdr_snapshot,
                pending.rtx_hdr_executable,
                bool_value(Some(pending.changed_rtx_hdr)),
                bool_value(Some(pending.changed_auto_hdr)),
            ],
        )?;
        Ok(())
    }

    pub fn clear_pending_display_profile_restore(&self) -> Result<(), DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        connection.execute(
            "DELETE FROM pending_display_profile_restore WHERE id = 1",
            [],
        )?;
        Ok(())
    }

    pub fn start_game_session(&self, game_id: &str) -> Result<i64, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.unchecked_transaction()?;
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM games WHERE id = ?1)",
            params![game_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(DatabaseError::GameNotFound);
        }
        if let Some(active_id) = transaction
            .query_row(
                "SELECT id FROM game_sessions WHERE game_id = ?1 AND status = 'active' ORDER BY id DESC LIMIT 1",
                params![game_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
        {
            transaction.commit()?;
            return Ok(active_id);
        }
        let now = timestamp();
        transaction.execute(
            "INSERT INTO game_sessions(game_id, started_at, status, source, created_at, updated_at)
             VALUES (?1, ?2, 'active', 'lumadeck', ?2, ?2)",
            params![game_id, now],
        )?;
        let session_id = transaction.last_insert_rowid();
        transaction.execute(
            "INSERT INTO game_activity_events(game_id, event_type, occurred_at, title, source, created_at)
             VALUES (?1, 'session_started', ?2, 'Sesión iniciada', 'local', ?2)",
            params![game_id, now],
        )?;
        transaction.commit()?;
        Ok(session_id)
    }

    pub fn end_game_session(
        &self,
        game_id: &str,
        session_id: i64,
        interrupted: bool,
    ) -> Result<(), DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.unchecked_transaction()?;
        let started_at: String = transaction
            .query_row(
                "SELECT started_at FROM game_sessions WHERE id = ?1 AND game_id = ?2 AND status = 'active'",
                params![session_id, game_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(DatabaseError::GameNotFound)?;
        let now = timestamp();
        let duration_seconds = (activity_timestamp(&now) - activity_timestamp(&started_at)).max(0);
        let status = if interrupted {
            "interrupted"
        } else {
            "completed"
        };
        transaction.execute(
            "UPDATE game_sessions
             SET ended_at = ?1, duration_seconds = ?2, status = ?3, updated_at = ?1
             WHERE id = ?4 AND game_id = ?5",
            params![now, duration_seconds, status, session_id, game_id],
        )?;
        transaction.execute(
            "INSERT INTO game_activity_events(game_id, event_type, occurred_at, title, description, value_json, source, created_at)
             VALUES (?1, 'session_ended', ?2, ?3, NULL, ?4, 'local', ?2)",
            params![
                game_id,
                now,
                if interrupted { "Sesión interrumpida" } else { "Sesión finalizada" },
                serde_json::json!({ "durationSeconds": duration_seconds }).to_string(),
            ],
        )?;
        transaction.execute(
            "UPDATE games SET last_played_at = ?1, updated_at = ?1 WHERE id = ?2",
            params![now, game_id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn recover_stale_game_sessions(&self) -> Result<i64, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.unchecked_transaction()?;
        let now = timestamp();
        let active_ids: Vec<(i64, String, String)> = {
            let mut statement = transaction.prepare(
                "SELECT id, game_id, started_at FROM game_sessions WHERE status = 'active'",
            )?;
            let rows =
                statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        for (session_id, game_id, started_at) in &active_ids {
            transaction.execute(
                "UPDATE game_sessions
                 SET ended_at = ?1, duration_seconds = 0, status = 'interrupted', updated_at = ?1
                 WHERE id = ?2 AND status = 'active'",
                params![started_at, session_id],
            )?;
            transaction.execute(
                "INSERT INTO game_activity_events(game_id, event_type, occurred_at, title, description, value_json, source, created_at)
                 VALUES (?1, 'stale_session', ?2, 'Sesión recuperada', ?3, ?4, 'local', ?2)",
                params![
                    game_id,
                    now,
                    "La sesión activa anterior se cerró sin sumar tiempo durante el cierre de LumaDeck.",
                    serde_json::json!({ "reason": "stale_running_state_recovered", "sessionId": session_id }).to_string(),
                ],
            )?;
        }
        transaction.commit()?;
        Ok(active_ids.len() as i64)
    }

    pub fn get_steam_library_sync_settings(
        &self,
    ) -> Result<SteamLibrarySyncSettings, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let value = connection
            .query_row(
                "SELECT value_json FROM app_settings WHERE key = 'steam.library.sync_scope'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let scope = value
            .and_then(|json| serde_json::from_str::<String>(&json).ok())
            .filter(|scope| scope == "all" || scope == "installed")
            .unwrap_or_else(|| "all".to_string());
        Ok(SteamLibrarySyncSettings { scope })
    }

    pub fn recover_interrupted_syncs(&self) -> Result<(), DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let completed_at = timestamp();
        connection.execute(
            "UPDATE steam_sync_state SET status = 'error', completed_at = ?1, current_app_id = NULL, error_message = 'STEAM_SYNC_INTERRUPTED_ON_START' WHERE account_id = ?2 AND status = 'running'",
            params![completed_at, STEAM_ACCOUNT_ID],
        )?;
        connection.execute(
            "UPDATE steam_image_sync_state SET status = 'error', completed_at = ?1, current_app_id = NULL, error_message = 'STEAM_IMAGE_SYNC_INTERRUPTED_ON_START' WHERE account_id = ?2 AND status = 'running'",
            params![completed_at, STEAM_ACCOUNT_ID],
        )?;
        Ok(())
    }

    pub fn set_steam_library_sync_scope(
        &self,
        scope: &str,
    ) -> Result<SteamLibrarySyncSettings, DatabaseError> {
        if scope != "all" && scope != "installed" {
            return Err(DatabaseError::InvalidSteamSyncScope);
        }
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        connection.execute(
            "INSERT INTO app_settings(key, value_json, schema_version, updated_at) VALUES ('steam.library.sync_scope', ?1, 1, datetime('now')) ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            params![serde_json::to_string(scope).map_err(|_| rusqlite::Error::InvalidQuery)?],
        )?;
        Ok(SteamLibrarySyncSettings {
            scope: scope.to_string(),
        })
    }

    pub fn get_hltb_settings(&self) -> Result<HltbSettings, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let value = connection
            .query_row(
                "SELECT value_json FROM app_settings WHERE key = 'hltb.integration'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(value
            .and_then(|json| serde_json::from_str::<HltbSettings>(&json).ok())
            .unwrap_or_default())
    }

    pub fn set_hltb_settings(
        &self,
        settings: &HltbSettings,
    ) -> Result<HltbSettings, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let value = serde_json::to_string(settings).map_err(|_| rusqlite::Error::InvalidQuery)?;
        connection.execute(
            "INSERT INTO app_settings(key, value_json, schema_version, updated_at) VALUES ('hltb.integration', ?1, 1, datetime('now')) ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            params![value],
        )?;
        Ok(settings.clone())
    }

    pub fn get_hltb_sync_status(&self) -> Result<HltbSyncStatus, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        connection
            .query_row(
                "SELECT status, processed_count, total_count, found_count, unmatched_count,
                    exact_match_count, approximate_match_count, error_count, duration_ms,
                    started_at, completed_at, last_error
                 FROM hltb_sync_state WHERE id = ?1",
                params![HLTB_SYNC_ID],
                |row| {
                    Ok(HltbSyncStatus {
                        status: row.get(0)?,
                        processed_count: row.get(1)?,
                        total_count: row.get(2)?,
                        found_count: row.get(3)?,
                        unmatched_count: row.get(4)?,
                        exact_match_count: row.get(5)?,
                        approximate_match_count: row.get(6)?,
                        error_count: row.get(7)?,
                        duration_ms: row.get(8)?,
                        started_at: row.get(9)?,
                        completed_at: row.get(10)?,
                        last_error: row.get(11)?,
                    })
                },
            )
            .optional()?
            .ok_or(DatabaseError::Sqlite(rusqlite::Error::QueryReturnedNoRows))
    }

    pub fn get_hltb_local_games(
        &self,
        only_missing: bool,
    ) -> Result<Vec<HltbLocalGame>, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let mut statement = connection.prepare(
            "SELECT g.id, g.title FROM games g
             WHERE ?1 = 0 OR NOT EXISTS (
                SELECT 1 FROM hltb_game_times h
                WHERE h.game_id = g.id AND h.status = 'matched'
                  AND h.main_story_minutes IS NOT NULL
               )
               AND NOT EXISTS (
                SELECT 1 FROM hltb_match_overrides o
                WHERE o.game_id = g.id AND o.resolution_status = 'ignored'
               )
             ORDER BY g.sort_title",
        )?;
        let games = statement
            .query_map([i64::from(only_missing)], |row| {
                Ok(HltbLocalGame {
                    id: row.get(0)?,
                    title: row.get(1)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(games)
    }

    pub fn get_hltb_pending_matches(&self) -> Result<Vec<HltbPendingMatch>, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let mut statement = connection.prepare(
            "SELECT g.id, g.title, o.alias_query, o.resolution_status
             FROM games g
             LEFT JOIN hltb_game_times h ON h.game_id = g.id
             LEFT JOIN hltb_match_overrides o ON o.game_id = g.id
             WHERE (h.game_id IS NULL OR h.status IN ('unmatched', 'error'))
               AND COALESCE(o.resolution_status, 'pending') != 'ignored'
             ORDER BY g.sort_title",
        )?;
        let matches = statement
            .query_map([], |row| {
                Ok(HltbPendingMatch {
                    game_id: row.get(0)?,
                    title: row.get(1)?,
                    alias_query: row.get(2)?,
                    resolution_status: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(matches)
    }

    pub fn set_hltb_match_override(
        &self,
        game_id: &str,
        alias_query: &str,
        candidate: Option<&crate::hltb::HltbCandidate>,
        resolution_status: &str,
    ) -> Result<(), DatabaseError> {
        let mut connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.transaction()?;
        let now = timestamp();
        transaction.execute(
            "INSERT INTO hltb_match_overrides(
                game_id, alias_query, hltb_id, matched_title, main_story_minutes,
                main_extra_minutes, completionist_minutes, resolution_status,
                created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)
             ON CONFLICT(game_id) DO UPDATE SET
                alias_query = excluded.alias_query, hltb_id = excluded.hltb_id,
                matched_title = excluded.matched_title,
                main_story_minutes = excluded.main_story_minutes,
                main_extra_minutes = excluded.main_extra_minutes,
                completionist_minutes = excluded.completionist_minutes,
                resolution_status = excluded.resolution_status,
                updated_at = excluded.updated_at",
            params![
                game_id,
                alias_query.trim(),
                candidate.map(|value| value.hltb_id.as_str()),
                candidate.map(|value| value.title.as_str()),
                candidate.and_then(|value| value.main_story_minutes),
                candidate.and_then(|value| value.main_extra_minutes),
                candidate.and_then(|value| value.completionist_minutes),
                resolution_status,
                now,
            ],
        )?;
        transaction.execute(
            "INSERT INTO hltb_game_times(
                game_id, hltb_id, matched_title, main_story_minutes, main_extra_minutes,
                completionist_minutes, match_confidence, match_type, last_synced_at,
                source, status, last_error
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, NULL)
             ON CONFLICT(game_id) DO UPDATE SET
                hltb_id = excluded.hltb_id, matched_title = excluded.matched_title,
                main_story_minutes = excluded.main_story_minutes,
                main_extra_minutes = excluded.main_extra_minutes,
                completionist_minutes = excluded.completionist_minutes,
                match_confidence = excluded.match_confidence,
                match_type = excluded.match_type,
                last_synced_at = excluded.last_synced_at,
                source = excluded.source, status = excluded.status, last_error = NULL",
            params![
                game_id,
                candidate.map(|value| value.hltb_id.as_str()),
                candidate.map(|value| value.title.as_str()),
                candidate.and_then(|value| value.main_story_minutes),
                candidate.and_then(|value| value.main_extra_minutes),
                candidate.and_then(|value| value.completionist_minutes),
                candidate.map(|_| 1.0_f64),
                candidate.map(|_| "manual"),
                now,
                HLTB_SOURCE,
                if resolution_status == "manual" {
                    "matched"
                } else {
                    "ignored"
                },
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn clear_hltb_match_override(&self, game_id: &str) -> Result<(), DatabaseError> {
        let mut connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "DELETE FROM hltb_match_overrides WHERE game_id = ?1",
            params![game_id],
        )?;
        transaction.execute(
            "DELETE FROM hltb_game_times WHERE game_id = ?1",
            params![game_id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn begin_hltb_sync(&self, total_count: i64, started_at: &str) -> Result<(), DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        connection.execute(
            "UPDATE hltb_sync_state SET status = 'running', processed_count = 0, total_count = ?1,
                found_count = 0, unmatched_count = 0, exact_match_count = 0,
                approximate_match_count = 0, error_count = 0, duration_ms = NULL,
                started_at = ?2, completed_at = NULL, last_error = NULL WHERE id = ?3",
            params![total_count, started_at, HLTB_SYNC_ID],
        )?;
        Ok(())
    }

    pub fn update_hltb_sync_progress(&self, processed: i64) -> Result<(), DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        connection.execute(
            "UPDATE hltb_sync_state SET processed_count = ?1 WHERE id = ?2",
            params![processed, HLTB_SYNC_ID],
        )?;
        Ok(())
    }

    pub fn save_hltb_game(
        &self,
        game_id: &str,
        result: Option<&crate::hltb::HltbResult>,
        status: &str,
        error: Option<&str>,
    ) -> Result<(), DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let now = timestamp();
        connection.execute(
            "INSERT INTO hltb_game_times(game_id, hltb_id, matched_title, main_story_minutes,
                main_extra_minutes, completionist_minutes, match_confidence, match_type,
                last_synced_at, source, status, last_error)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(game_id) DO UPDATE SET
                hltb_id = COALESCE(excluded.hltb_id, hltb_game_times.hltb_id),
                matched_title = COALESCE(excluded.matched_title, hltb_game_times.matched_title),
                main_story_minutes = COALESCE(excluded.main_story_minutes, hltb_game_times.main_story_minutes),
                main_extra_minutes = COALESCE(excluded.main_extra_minutes, hltb_game_times.main_extra_minutes),
                completionist_minutes = COALESCE(excluded.completionist_minutes, hltb_game_times.completionist_minutes),
                match_confidence = COALESCE(excluded.match_confidence, hltb_game_times.match_confidence),
                match_type = COALESCE(excluded.match_type, hltb_game_times.match_type),
                last_synced_at = excluded.last_synced_at, source = excluded.source,
                status = excluded.status, last_error = excluded.last_error",
            params![
                game_id,
                result.map(|value| value.hltb_id.as_str()),
                result.map(|value| value.matched_title.as_str()),
                result.and_then(|value| value.main_story_minutes),
                result.and_then(|value| value.main_extra_minutes),
                result.and_then(|value| value.completionist_minutes),
                result.map(|value| value.confidence),
                result.map(|value| value.match_type),
                now,
                HLTB_SOURCE,
                status,
                error,
            ],
        )?;
        Ok(())
    }

    pub fn finish_hltb_sync(
        &self,
        found_count: i64,
        unmatched_count: i64,
        exact_match_count: i64,
        approximate_match_count: i64,
        error_count: i64,
        duration_ms: i64,
        completed_at: &str,
    ) -> Result<(), DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        connection.execute(
            "UPDATE hltb_sync_state SET status = CASE WHEN ?5 > 0 THEN 'error' ELSE 'completed' END,
                found_count = ?1, unmatched_count = ?2, exact_match_count = ?3,
                approximate_match_count = ?4, error_count = ?5, processed_count = total_count,
                duration_ms = ?6, completed_at = ?7, last_error = NULL WHERE id = ?8",
            params![
                found_count,
                unmatched_count,
                exact_match_count,
                approximate_match_count,
                error_count,
                duration_ms,
                completed_at,
                HLTB_SYNC_ID
            ],
        )?;
        Ok(())
    }

    pub fn fail_hltb_sync(&self, duration_ms: i64, error: &str) -> Result<(), DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        connection.execute(
            "UPDATE hltb_sync_state SET status = 'error', duration_ms = ?1, completed_at = ?2, last_error = ?3 WHERE id = ?4",
            params![duration_ms, timestamp(), error, HLTB_SYNC_ID],
        )?;
        Ok(())
    }

    pub fn cancel_hltb_sync(&self, duration_ms: i64) -> Result<(), DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        connection.execute(
            "UPDATE hltb_sync_state SET status = 'cancelled', duration_ms = ?1, completed_at = ?2, last_error = NULL WHERE id = ?3",
            params![duration_ms, timestamp(), HLTB_SYNC_ID],
        )?;
        Ok(())
    }

    pub fn get_local_games(&self) -> Result<Vec<LocalGame>, DatabaseError> {
        self.get_local_games_with_options(None, true)
    }

    pub fn get_local_game_summaries(&self) -> Result<Vec<LocalGame>, DatabaseError> {
        self.get_local_games_with_options(None, false)
    }

    pub fn get_local_game(&self, game_id: &str) -> Result<LocalGame, DatabaseError> {
        self.get_local_games_with_options(Some(game_id), true)?
            .into_iter()
            .next()
            .ok_or(DatabaseError::GameNotFound)
    }

    fn get_local_games_with_options(
        &self,
        game_id: Option<&str>,
        include_details: bool,
    ) -> Result<Vec<LocalGame>, DatabaseError> {
        let media_index_started = std::time::Instant::now();
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let installed_only = connection
            .query_row(
                "SELECT value_json FROM app_settings WHERE key = 'steam.library.sync_scope'",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .and_then(|value| serde_json::from_str::<String>(&value).ok())
            .as_deref()
            == Some("installed");
        let mut statement = connection.prepare(
            "SELECT g.id, g.title, g.sort_title, g.platform, g.provider,
                COALESCE((SELECT a.cached_path FROM game_artwork_selections s JOIN artwork_assets a ON a.id = s.artwork_asset_id WHERE s.game_id = g.id AND s.slot = 'grid_horizontal'), (SELECT local_path FROM steam_game_assets WHERE game_id = g.id AND asset_type = 'horizontal_cover' AND local_path IS NOT NULL ORDER BY updated_at DESC LIMIT 1), (SELECT local_path FROM steam_game_assets WHERE game_id = g.id AND asset_type = 'vertical_cover' AND local_path IS NOT NULL ORDER BY updated_at DESC LIMIT 1), d.steam_header_url, d.steam_logo_url, (SELECT r.media_url FROM launchbox_media_refs r JOIN launchbox_catalog_state c ON c.id = 1 AND c.catalog_version = r.catalog_version WHERE g.source = 'emulator' AND r.provider_game_id IN (SELECT m.provider_game_id FROM external_identity_mappings m WHERE m.provider = 'launchbox' AND m.native_id = g.title_id AND m.platform = CASE WHEN lower(g.platform) LIKE '%switch%' THEN 'nintendo_switch' ELSE replace(lower(g.platform), ' ', '_') END) AND r.media_type = 'box_front' LIMIT 1), ''),
                COALESCE((SELECT a.cached_path FROM game_artwork_selections s JOIN artwork_assets a ON a.id = s.artwork_asset_id WHERE s.game_id = g.id AND s.slot = 'grid_vertical'), (SELECT local_path FROM steam_game_assets WHERE game_id = g.id AND asset_type = 'vertical_cover' AND local_path IS NOT NULL ORDER BY updated_at DESC LIMIT 1), ''),
                COALESCE((SELECT a.cached_path FROM game_artwork_selections s JOIN artwork_assets a ON a.id = s.artwork_asset_id WHERE s.game_id = g.id AND s.slot = 'logo'), (SELECT local_path FROM steam_game_assets WHERE game_id = g.id AND asset_type = 'logo' AND local_path IS NOT NULL ORDER BY updated_at DESC LIMIT 1), d.steam_logo_url, (SELECT source_url FROM steam_game_assets WHERE game_id = g.id AND asset_type = 'logo' ORDER BY updated_at DESC LIMIT 1), (SELECT r.media_url FROM launchbox_media_refs r JOIN launchbox_catalog_state c ON c.id = 1 AND c.catalog_version = r.catalog_version WHERE g.source = 'emulator' AND r.provider_game_id IN (SELECT m.provider_game_id FROM external_identity_mappings m WHERE m.provider = 'launchbox' AND m.native_id = g.title_id AND m.platform = CASE WHEN lower(g.platform) LIKE '%switch%' THEN 'nintendo_switch' ELSE replace(lower(g.platform), ' ', '_') END) AND r.media_type = 'clear_logo' LIMIT 1), CASE WHEN d.steam_app_id IS NOT NULL THEN 'https://cdn.cloudflare.steamstatic.com/steam/apps/' || d.steam_app_id || '/logo.png' ELSE '' END),
                COALESCE((SELECT a.cached_path FROM game_artwork_selections s JOIN artwork_assets a ON a.id = s.artwork_asset_id WHERE s.game_id = g.id AND s.slot = 'hero'), (SELECT local_path FROM steam_game_assets WHERE game_id = g.id AND asset_type = 'hero' AND local_path IS NOT NULL ORDER BY updated_at DESC LIMIT 1), NULLIF(d.steam_background_url, ''), (SELECT source_url FROM steam_game_assets WHERE game_id = g.id AND asset_type = 'hero' ORDER BY updated_at DESC LIMIT 1), (SELECT r.media_url FROM launchbox_media_refs r JOIN launchbox_catalog_state c ON c.id = 1 AND c.catalog_version = r.catalog_version WHERE g.source = 'emulator' AND r.provider_game_id IN (SELECT m.provider_game_id FROM external_identity_mappings m WHERE m.provider = 'launchbox' AND m.native_id = g.title_id AND m.platform = CASE WHEN lower(g.platform) LIKE '%switch%' THEN 'nintendo_switch' ELSE replace(lower(g.platform), ' ', '_') END) AND r.media_type IN ('fanart', 'banner') LIMIT 1), CASE WHEN d.steam_app_id IS NOT NULL THEN 'https://cdn.cloudflare.steamstatic.com/steam/apps/' || d.steam_app_id || '/library_hero_2x.jpg' ELSE '' END),
                COALESCE(d.steam_description, d.steam_short_description, ''), COALESCE(d.steam_release_date, ''),
                CASE
                    WHEN g.source = 'emulator' AND g.emulator_id = 'eden' THEN
                        COALESCE((SELECT total_seconds / 60
                                  FROM external_playtime_snapshots
                                  WHERE game_id = g.id AND provider = 'eden'
                                  ORDER BY observed_at DESC LIMIT 1),
                                 COALESCE(g.playtime_minutes, 0) +
                                 COALESCE((SELECT SUM(COALESCE(duration_seconds, 0)) / 60
                                           FROM game_sessions
                                           WHERE game_id = g.id AND source = 'lumadeck'), 0))
                    ELSE
                        COALESCE(g.playtime_minutes, 0) +
                        COALESCE((SELECT SUM(COALESCE(duration_seconds, 0)) / 60
                                  FROM game_sessions
                                  WHERE game_id = g.id AND source = 'lumadeck'), 0)
                END + COALESCE(d.steam_total_playtime_minutes, 0),
                CASE
                    WHEN g.last_played_at IS NOT NULL THEN g.last_played_at
                    WHEN d.steam_last_played_at IS NULL THEN
                        (SELECT CAST(MAX(CAST(ended_at AS INTEGER)) AS TEXT)
                         FROM game_sessions
                         WHERE game_id = g.id AND source = 'lumadeck'
                           AND ended_at IS NOT NULL)
                    WHEN (SELECT MAX(CAST(ended_at AS INTEGER))
                          FROM game_sessions
                          WHERE game_id = g.id AND source = 'lumadeck'
                            AND ended_at IS NOT NULL) >
                         CAST(d.steam_last_played_at AS INTEGER)
                    THEN (SELECT CAST(MAX(CAST(ended_at AS INTEGER)) AS TEXT)
                          FROM game_sessions
                          WHERE game_id = g.id AND source = 'lumadeck'
                            AND ended_at IS NOT NULL)
                    ELSE d.steam_last_played_at
                END,
                g.favorite,
                COALESCE(d.steam_installed, g.installed), g.progress, g.status,
                 d.steam_achievement_total, d.steam_achievement_unlocked, d.steam_achievement_progress,
                 COALESCE(d.steam_header_url, (SELECT source_url FROM steam_game_assets WHERE game_id = g.id AND asset_type = 'horizontal_cover' ORDER BY updated_at DESC LIMIT 1), d.steam_logo_url, '') AS cover_fallback,
                 COALESCE((SELECT source_url FROM steam_game_assets WHERE game_id = g.id AND asset_type = 'vertical_cover' ORDER BY updated_at DESC LIMIT 1), d.steam_header_url, '') AS vertical_cover_fallback,
                 COALESCE(d.steam_logo_url, (SELECT source_url FROM steam_game_assets WHERE game_id = g.id AND asset_type = 'logo' ORDER BY updated_at DESC LIMIT 1), CASE WHEN d.steam_app_id IS NOT NULL THEN 'https://cdn.cloudflare.steamstatic.com/steam/apps/' || d.steam_app_id || '/logo.png' ELSE '' END) AS logo_fallback,
                 COALESCE(NULLIF(d.steam_background_url, ''), (SELECT source_url FROM steam_game_assets WHERE game_id = g.id AND asset_type = 'hero' ORDER BY updated_at DESC LIMIT 1), CASE WHEN d.steam_app_id IS NOT NULL THEN 'https://cdn.cloudflare.steamstatic.com/steam/apps/' || d.steam_app_id || '/library_hero_2x.jpg' ELSE '' END) AS background_fallback,
                 COALESCE((SELECT local_path FROM steam_game_assets WHERE game_id = g.id AND asset_type = 'icon' AND local_path IS NOT NULL ORDER BY updated_at DESC LIMIT 1), d.steam_icon_url, '') AS icon_fallback,
                 COALESCE((SELECT a.cached_path
                           FROM game_artwork_selections s
                           JOIN artwork_assets a ON a.id = s.artwork_asset_id
                           WHERE s.game_id = g.id
                             AND s.slot = 'grid_square'
                             AND ((a.width = 1024 AND a.height = 1024) OR (a.width = 512 AND a.height = 512))
                           ORDER BY s.updated_at DESC LIMIT 1), '') AS square_cover
                 , g.hidden, g.source, g.emulator_id, g.game_path, g.title_id
             FROM games g LEFT JOIN game_details d ON d.game_id = g.id
             WHERE (?1 IS NULL OR g.id = ?1) ORDER BY g.sort_title",
        )?;
        let mut games = statement
            .query_map(params![game_id], |row| {
                let game_id: String = row.get(0)?;
                let release_date: String = row.get(10)?;
                let release_year = release_date
                    .get(0..4)
                    .and_then(|value| value.parse::<i64>().ok())
                    .unwrap_or(0);
                let cover_url: String = row.get(5)?;
                let vertical_cover_url: String = row.get(6)?;
                let logo_url: String = row.get(7)?;
                let background_url: String = row.get(8)?;
                let cover_fallback: String = row.get(20)?;
                let vertical_cover_fallback: String = row.get(21)?;
                let logo_fallback: String = row.get(22)?;
                let background_fallback: String = row.get(23)?;
                let icon_fallback: String = row.get(24)?;
                let square_cover_url: String = row.get(25)?;
                Ok(LocalGame {
                    id: game_id.clone(),
                    title: row.get(1)?,
                    sort_title: row.get(2)?,
                    platform: row.get(3)?,
                    provider: row.get(4)?,
                    cover_url: resolve_media_asset_path(
                        self.state,
                        &game_id,
                        "grid",
                        &cover_url,
                        &cover_fallback,
                    ),
                    vertical_cover_url: resolve_media_asset_path(
                        self.state,
                        &game_id,
                        "grid",
                        &vertical_cover_url,
                        &vertical_cover_fallback,
                    ),
                    square_cover_url: resolve_media_asset_path(
                        self.state,
                        &game_id,
                        "grid",
                        &square_cover_url,
                        "",
                    ),
                    logo_url: resolve_media_asset_path(
                        self.state,
                        &game_id,
                        "logo",
                        &logo_url,
                        &logo_fallback,
                    ),
                    background_url: resolve_media_asset_path(
                        self.state,
                        &game_id,
                        "hero",
                        &background_url,
                        &background_fallback,
                    ),
                    screenshots: Vec::new(),
                    icon_url: resolve_local_asset_path(self.state, &icon_fallback, ""),
                    description: row.get(9)?,
                    genres: Vec::new(),
                    release_year,
                    playtime_minutes: row.get(11)?,
                    last_played_at: row.get(12)?,
                    favorite: row.get::<_, i64>(13)? != 0,
                    hidden: row.get::<_, i64>(26)? != 0,
                    installed: row.get::<_, i64>(14)? != 0,
                    progress: row.get(15)?,
                    status: row.get(16)?,
                    achievements: {
                        let total = row.get::<_, Option<i64>>(17)?;
                        let unlocked = row.get::<_, Option<i64>>(18)?;
                        let progress = row.get::<_, Option<f64>>(19)?;
                        match (total, unlocked, progress) {
                            (None, None, None) => None,
                            (total, unlocked, progress) => Some(LocalGameAchievements {
                                total,
                                unlocked,
                                progress,
                            }),
                        }
                    },
                    details: None,
                    source: row.get(27)?,
                    emulator: row.get(28)?,
                    game_path: row.get(29)?,
                    title_id: row.get(30)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        if installed_only {
            let has_installed_steam_game = games
                .iter()
                .any(|game| game.provider == "Steam" && game.installed);
            if has_installed_steam_game {
                games.retain(|game| game.provider != "Steam" || game.installed);
            }
        }
        if include_details {
            for game in &mut games {
                if media_trace_enabled(&game.id) {
                    self.state.log(
                        "media-timing",
                        "GAME_MEDIA_QUERY",
                        &format!(
                            "timestamp_ms={} gameId={} type=all",
                            epoch_millis(),
                            game.id
                        ),
                    );
                }
                let mut genres_statement = connection.prepare(
                    "SELECT value FROM steam_game_genres WHERE game_id = ?1 ORDER BY value",
                )?;
                game.genres = genres_statement
                    .query_map(params![game.id], |genre_row| genre_row.get(0))?
                    .collect::<Result<Vec<String>, _>>()?;
                if game.genres.is_empty() {
                    let mut tags_statement = connection.prepare(
                        "SELECT value FROM steam_game_tags WHERE game_id = ?1 ORDER BY value",
                    )?;
                    game.genres = tags_statement
                        .query_map(params![game.id], |tag_row| tag_row.get(0))?
                        .collect::<Result<Vec<String>, _>>()?;
                }
                let mut screenshots_statement = connection.prepare(
                "SELECT local_path FROM steam_game_assets WHERE game_id = ?1 AND asset_type = 'screenshot' AND local_path IS NOT NULL ORDER BY external_id",
            )?;
                game.screenshots = screenshots_statement
                    .query_map(params![game.id], |screenshot_row| screenshot_row.get(0))?
                    .collect::<Result<Vec<String>, _>>()?
                    .into_iter()
                    .map(|path| {
                        resolve_media_asset_path(self.state, &game.id, "screenshot", &path, "")
                    })
                    .collect();

                let mut movies_statement = connection.prepare(
                "SELECT full_url FROM steam_game_media WHERE game_id = ?1 AND media_type = 'movie' AND full_url IS NOT NULL AND full_url <> '' ORDER BY external_id",
            )?;
                let movies = movies_statement
                    .query_map(params![game.id], |movie_row| movie_row.get(0))?
                    .collect::<Result<Vec<String>, _>>()?;

                let steam_details = connection
                    .query_row(
                        "SELECT steam_app_id, steam_description, steam_short_description,
                        steam_release_date, steam_review_score, steam_multiplayer,
                        steam_single_player, steam_cloud, steam_trading_cards, steam_workshop
                     FROM game_details WHERE game_id = ?1",
                        params![game.id],
                        |row| {
                            Ok((
                                row.get::<_, Option<i64>>(0)?,
                                row.get::<_, Option<String>>(1)?,
                                row.get::<_, Option<String>>(2)?,
                                row.get::<_, Option<String>>(3)?,
                                row.get::<_, Option<i64>>(4)?,
                                row.get::<_, Option<i64>>(5)?,
                                row.get::<_, Option<i64>>(6)?,
                                row.get::<_, Option<i64>>(7)?,
                                row.get::<_, Option<i64>>(8)?,
                                row.get::<_, Option<i64>>(9)?,
                            ))
                        },
                    )
                    .optional()?;
                let hltb = connection
                .query_row(
                    "SELECT game_id, hltb_id, matched_title, main_story_minutes, main_extra_minutes,
                        completionist_minutes, match_confidence, match_type, last_synced_at,
                        source, status, last_error FROM hltb_game_times WHERE game_id = ?1",
                    params![game.id],
                    |row| {
                        Ok(HltbGameData {
                            game_id: row.get(0)?,
                            hltb_id: row.get(1)?,
                            matched_title: row.get(2)?,
                            main_story_minutes: row.get(3)?,
                            main_extra_minutes: row.get(4)?,
                            completionist_minutes: row.get(5)?,
                            match_confidence: row.get(6)?,
                            match_type: row.get(7)?,
                            last_synced_at: row.get(8)?,
                            source: row.get(9)?,
                            status: row.get(10)?,
                            last_error: row.get(11)?,
                        })
                    },
                )
                .optional()?;
                if let Some((
                    steam_app_id,
                    description,
                    short_description,
                    release_date,
                    review_score,
                    multiplayer,
                    single_player,
                    cloud,
                    trading_cards,
                    workshop,
                )) = steam_details
                {
                    let tags = connection
                        .prepare("SELECT value FROM steam_game_tags WHERE game_id = ?1")?
                        .query_map(params![game.id], |tag_row| tag_row.get(0))?
                        .collect::<Result<Vec<String>, _>>()?;
                    let categories = connection
                    .prepare(
                        "SELECT value FROM steam_game_categories WHERE game_id = ?1 ORDER BY value",
                    )?
                    .query_map(params![game.id], |category_row| category_row.get(0))?
                    .collect::<Result<Vec<String>, _>>()?;
                    game.details = Some(LocalGameDetails {
                        steam: Some(LocalSteamDetails {
                            app_id: steam_app_id.unwrap_or_default(),
                            description,
                            short_description,
                            release_date,
                            review_score,
                            tags,
                            genres: game.genres.clone(),
                            categories,
                            screenshots: game.screenshots.clone(),
                            movies,
                            multiplayer: multiplayer.map(|value| value != 0),
                            single_player: single_player.map(|value| value != 0),
                            cloud: cloud.map(|value| value != 0),
                            trading_cards: trading_cards.map(|value| value != 0),
                            workshop: workshop.map(|value| value != 0),
                        }),
                        launchbox: None,
                        hltb: hltb.clone(),
                    });
                }
                if game.details.is_none() && hltb.is_some() {
                    game.details = Some(LocalGameDetails {
                        steam: None,
                        launchbox: None,
                        hltb,
                    });
                } else if let Some(details) = game.details.as_mut() {
                    details.hltb = hltb;
                }

                if game.source == "emulator" {
                    if let Some(launchbox) = crate::launchbox::get_game_metadata(
                        &connection,
                        &game.id,
                        game.title_id.as_deref(),
                        &game.title,
                        &game.platform,
                    )? {
                        self.state.log(
                        "launchbox-metadata",
                        "metadata_resolved",
                        &format!(
                            "game_id={} title_id={} genres={} local_multiplayer={} max_local_players={} rating_raw={} rating_scale={} rating_count={}",
                            game.id,
                            game.title_id.as_deref().unwrap_or("<none>"),
                            launchbox.normalized_genres.join("|"),
                            launchbox.local_multiplayer,
                            launchbox
                                .max_local_players
                                .map(|value| value.to_string())
                                .unwrap_or_else(|| "<none>".to_string()),
                            launchbox
                                .community_rating_raw
                                .map(|value| value.to_string())
                                .unwrap_or_else(|| "<none>".to_string()),
                            launchbox
                                .community_rating_scale
                                .map(|value| value.to_string())
                                .unwrap_or_else(|| "<none>".to_string()),
                            launchbox
                                .community_rating_count
                                .map(|value| value.to_string())
                                .unwrap_or_else(|| "<none>".to_string())
                        ),
                    );
                        if let Some(description) = launchbox.description.as_deref() {
                            if !description.is_empty() {
                                game.description = description.to_string();
                            }
                        }
                        if !launchbox.normalized_genres.is_empty() {
                            game.genres = launchbox.normalized_genres.clone();
                        }
                        if let Some(release_date) = launchbox.release_date.as_deref() {
                            game.release_year = release_date
                                .get(0..4)
                                .and_then(|value| value.parse::<i64>().ok())
                                .unwrap_or(game.release_year);
                        }
                        if !launchbox.screenshots.is_empty() {
                            game.screenshots = launchbox
                                .screenshots
                                .iter()
                                .map(|path| resolve_local_asset_path(self.state, path, ""))
                                .collect();
                        }
                        let mut launchbox = launchbox;
                        launchbox.screenshots = launchbox
                            .screenshots
                            .iter()
                            .map(|path| resolve_local_asset_path(self.state, path, ""))
                            .collect();
                        if let Some(details) = game.details.as_mut() {
                            details.launchbox = Some(launchbox);
                        } else {
                            game.details = Some(LocalGameDetails {
                                steam: None,
                                launchbox: Some(launchbox),
                                hltb: None,
                            });
                        }
                    }
                }

                if !asset_is_available(&game.cover_url)
                    && asset_is_available(&game.vertical_cover_url)
                {
                    game.cover_url = game.vertical_cover_url.clone();
                }
            }
        } else {
            for game in &mut games {
                game.description.clear();
                game.genres.clear();
            }
        }
        self.state.log(
            "media-timing",
            "GAME_MEDIA_QUERY",
            &format!(
                "timestamp_ms={} gameId=* type=all duration_ms={} game_count={}",
                epoch_millis(),
                media_index_started.elapsed().as_millis(),
                games.len()
            ),
        );
        Ok(games)
    }

    pub fn set_game_favorite(&self, game_id: &str, favorite: bool) -> Result<bool, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let updated = connection.execute(
            "UPDATE games SET favorite = ?1, updated_at = ?2 WHERE id = ?3",
            params![i64::from(favorite), timestamp(), game_id],
        )?;
        if updated == 0 {
            return Err(DatabaseError::GameNotFound);
        }
        Ok(favorite)
    }

    pub fn set_game_hidden(&self, game_id: &str, hidden: bool) -> Result<bool, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let updated = connection.execute(
            "UPDATE games SET hidden = ?1, updated_at = ?2 WHERE id = ?3",
            params![i64::from(hidden), timestamp(), game_id],
        )?;
        if updated == 0 {
            return Err(DatabaseError::GameNotFound);
        }
        Ok(hidden)
    }

    pub fn get_steam_app_ids(&self) -> Result<Vec<i64>, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let mut statement = connection.prepare(
            "SELECT steam_app_id FROM game_details WHERE steam_app_id IS NOT NULL ORDER BY steam_app_id",
        )?;
        let app_ids = statement
            .query_map([], |row| row.get(0))?
            .collect::<Result<Vec<i64>, _>>()
            .map_err(DatabaseError::from);
        app_ids
    }

    pub fn clear_stale_steam_achievements(&self) -> Result<i64, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.unchecked_transaction()?;
        let cleared = transaction.execute(
            "UPDATE game_details
             SET steam_achievement_total = NULL,
                 steam_achievement_unlocked = NULL,
                 steam_achievement_progress = NULL
             WHERE steam_achievement_total = 0
               AND steam_achievement_unlocked = 0",
            [],
        )?;
        transaction.execute(
            "DELETE FROM steam_game_achievements
             WHERE game_id IN (
                 SELECT game_id FROM game_details
                 WHERE steam_achievement_total IS NULL
                   AND steam_achievement_unlocked IS NULL
             )",
            [],
        )?;
        transaction.commit()?;
        Ok(cleared as i64)
    }

    pub fn save_steam_achievements(
        &self,
        app_id: i64,
        achievements: &[Achievement],
        genres: &[String],
        total: i64,
        stats: &[SteamStat],
    ) -> Result<bool, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let game_id: Option<String> = connection
            .query_row(
                "SELECT game_id FROM game_details WHERE steam_app_id = ?1",
                params![app_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(game_id) = game_id else {
            return Ok(false);
        };
        let unlocked = achievements
            .iter()
            .filter(|achievement| achievement.unlocked)
            .count() as i64;
        let progress = (total > 0).then(|| unlocked as f64 * 100.0 / total as f64);
        let fingerprint = steam_achievements_source_hash(achievements, total, stats);
        let previous_hash: Option<String> = connection
            .query_row(
                "SELECT source_hash FROM steam_achievement_sync_state WHERE game_id = ?1",
                params![game_id],
                |row| row.get(0),
            )
            .optional()?;
        let changed = previous_hash.as_deref() != Some(fingerprint.as_str());
        let transaction = connection.unchecked_transaction()?;
        let updated_at = timestamp();
        transaction.execute(
            "UPDATE game_details SET steam_achievement_total = ?1, steam_achievement_unlocked = ?2, steam_achievement_progress = ?3, steam_updated_at = ?4 WHERE game_id = ?5",
            params![total, unlocked, progress, updated_at, game_id],
        )?;
        if !genres.is_empty() {
            transaction.execute(
                "DELETE FROM steam_game_genres WHERE game_id = ?1",
                params![game_id],
            )?;
            for genre in genres {
                transaction.execute(
                    "INSERT INTO steam_game_genres(game_id, value) VALUES (?1, ?2)",
                    params![game_id, genre],
                )?;
            }
        }
        if changed {
            transaction.execute(
                "DELETE FROM steam_game_achievements WHERE game_id = ?1",
                params![game_id],
            )?;
            for achievement in achievements {
                transaction.execute(
                    "INSERT INTO steam_game_achievements(game_id, api_name, display_name, description, achieved, unlock_time, hidden, unlock_percentage, rarity, virtual_tier, icon_unlocked, icon_locked, local_icon_unlocked, local_icon_locked, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
                    params![
                        game_id,
                        achievement.api_name,
                        achievement.display_name,
                        achievement.description,
                        achievement.unlocked,
                        achievement.unlock_time,
                        achievement.hidden,
                        achievement.unlock_percentage,
                        achievement.rarity.as_str(),
                        achievement.rarity.virtual_tier().as_str(),
                        achievement.icon_unlocked,
                        achievement.icon_locked,
                        achievement.local_icon_unlocked,
                        achievement.local_icon_locked,
                        updated_at,
                    ],
                )?;
            }
            transaction.execute(
                "DELETE FROM steam_game_stats WHERE game_id = ?1",
                params![game_id],
            )?;
            for stat in stats {
                transaction.execute(
                    "INSERT INTO steam_game_stats(game_id, name, value_json) VALUES (?1, ?2, ?3)",
                    params![game_id, stat.name, stat.value.to_string()],
                )?;
            }
        }
        transaction.execute(
            "INSERT INTO steam_achievement_sync_state(game_id, steam_app_id, status, schema_version, source_hash, last_synced_at, last_attempted_at, error_message) VALUES (?1, ?2, 'completed', 1, ?3, ?4, ?4, NULL) ON CONFLICT(game_id) DO UPDATE SET steam_app_id = excluded.steam_app_id, status = excluded.status, schema_version = excluded.schema_version, source_hash = excluded.source_hash, last_synced_at = excluded.last_synced_at, last_attempted_at = excluded.last_attempted_at, error_message = NULL",
            params![game_id, app_id, fingerprint, updated_at],
        )?;
        transaction.commit()?;
        Ok(changed)
    }

    pub fn get_game_achievements(&self, game_id: &str) -> Result<GameAchievements, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let steam_app_id: i64 = connection
            .query_row(
                "SELECT steam_app_id FROM game_details WHERE game_id = ?1 AND steam_app_id IS NOT NULL",
                params![game_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(DatabaseError::GameNotFound)?;
        let mut statement = connection.prepare(
            "SELECT api_name, COALESCE(display_name, api_name), COALESCE(description, ''), hidden, achieved, unlock_time,
                    unlock_percentage, rarity, virtual_tier, icon_unlocked, icon_locked,
                    local_icon_unlocked, local_icon_locked
             FROM steam_game_achievements
             WHERE game_id = ?1
             ORDER BY api_name",
        )?;
        let achievements = statement
            .query_map(params![game_id], |row| {
                let rarity = rarity_from_str(&row.get::<_, String>(7)?);
                Ok(Achievement {
                    api_name: row.get(0)?,
                    display_name: row.get(1)?,
                    description: row.get(2)?,
                    hidden: row.get::<_, i64>(3)? != 0,
                    unlocked: row.get::<_, i64>(4)? != 0,
                    unlock_time: row.get(5)?,
                    unlock_percentage: row.get(6)?,
                    rarity,
                    virtual_tier: rarity.virtual_tier(),
                    icon_unlocked: row.get(9)?,
                    icon_locked: row.get(10)?,
                    local_icon_unlocked: row
                        .get::<_, Option<String>>(11)?
                        .and_then(|value| resolve_optional_local_asset_path(self.state, &value)),
                    local_icon_locked: row
                        .get::<_, Option<String>>(12)?
                        .and_then(|value| resolve_optional_local_asset_path(self.state, &value)),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let sync_state = connection
            .query_row(
                "SELECT status, schema_version, last_synced_at
                 FROM steam_achievement_sync_state WHERE game_id = ?1",
                params![game_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()?;
        let (sync_status, schema_version, last_synced_at) =
            sync_state.unwrap_or(("never-synced".to_string(), 1, None));
        Ok(GameAchievements {
            game_id: game_id.to_string(),
            steam_app_id,
            summary: summarize(&achievements),
            distribution: distribute_total(&achievements),
            recent: recent(&achievements, DEFAULT_RECENT_LIMIT),
            total_distribution: distribute_total(&achievements),
            unlocked_distribution: distribute_unlocked(&achievements),
            achievements,
            last_synced_at,
            sync_status,
            schema_version,
        })
    }

    pub fn get_achievement_summary(
        &self,
        game_id: &str,
    ) -> Result<AchievementSummary, DatabaseError> {
        Ok(self.get_game_achievements(game_id)?.summary)
    }

    pub fn get_achievement_distribution(
        &self,
        game_id: &str,
    ) -> Result<AchievementDistribution, DatabaseError> {
        Ok(self.get_game_achievements(game_id)?.distribution)
    }

    pub fn get_achievement_distributions(
        &self,
        game_id: &str,
    ) -> Result<AchievementDistributions, DatabaseError> {
        let achievements = self.get_game_achievements(game_id)?;
        Ok(AchievementDistributions {
            total: achievements.total_distribution,
            unlocked: achievements.unlocked_distribution,
        })
    }

    fn get_provider_configuration_traced(
        &self,
        provider_id: &str,
        correlation_id: &str,
    ) -> Result<SteamConfigurationStatus, DatabaseError> {
        if provider_id != STEAM_PROVIDER_ID {
            return Err(DatabaseError::UnsupportedProvider);
        }
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let account = connection.query_row(
            "SELECT id, external_account_id, configuration_status FROM provider_accounts WHERE id = ?1 AND provider_id = ?2",
            params![STEAM_ACCOUNT_ID, STEAM_PROVIDER_ID],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?, row.get::<_, String>(2)?)),
        ).optional()?;
        let Some((account_id, steam_id, status)) = account else {
            return Ok(not_configured());
        };
        let credential = connection.query_row(
            "SELECT encrypted_value, masked_suffix FROM provider_credentials WHERE provider_account_id = ?1 AND credential_type = ?2",
            params![account_id, STEAM_CREDENTIAL_TYPE],
            |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Option<String>>(1)?)),
        ).optional()?;
        let (api_key_configured, api_key_masked, credential_unavailable) = match credential {
            Some((encrypted, suffix)) => match crypto::unprotect(&encrypted) {
                Ok(_) => (
                    true,
                    suffix.map(|value| format!("••••••••••••{value}")),
                    false,
                ),
                Err(_) => (
                    true,
                    suffix.map(|value| format!("••••••••••••{value}")),
                    true,
                ),
            },
            None => (false, None, false),
        };
        let status = status_from_values(
            account_id,
            steam_id,
            api_key_configured,
            api_key_masked,
            credential_unavailable,
            status,
        );
        self.state.log(
            correlation_id,
            "RESPONSE_BUILD_SUCCESS",
            &format!(
                "response_type=SteamConfigurationStatus status={}",
                status.status
            ),
        );
        Ok(status)
    }

    fn log_error(&self, correlation_id: &str, stage: &str, error: &DatabaseError) {
        let sqlite = error
            .sqlite_diagnostic()
            .map(|value| format!(" sqlite={value}"))
            .unwrap_or_default();
        self.state.log(
            correlation_id,
            stage,
            &format!(
                "error_variant={} display={} source_chain={}{}",
                error.variant_name(),
                error,
                error.source_chain(),
                sqlite
            ),
        );
    }

    fn log_sql_error(&self, correlation_id: &str, stage: &str, error: &rusqlite::Error) {
        let sqlite = match error {
            rusqlite::Error::SqliteFailure(code, message) => format!(
                "code={:?}; extended_code={}; message={}",
                code.code,
                code.extended_code,
                message.as_deref().unwrap_or("<none>")
            ),
            other => format!("variant={other:?}"),
        };
        self.state.log(
            correlation_id,
            stage,
            &format!(
                "error_variant=Sqlite display={} source_chain={} sqlite={sqlite}",
                error, error
            ),
        );
    }

    pub fn save_steam_account_configuration(
        &self,
        steam_id64: &str,
        api_key: &str,
        correlation_id: &str,
    ) -> Result<SteamConfigurationStatus, DatabaseError> {
        validate_steam_id(steam_id64).map_err(|error| {
            self.log_error(correlation_id, "VALIDATION_FAILED", &error);
            error
        })?;
        self.state.log(
            correlation_id,
            "VALIDATION_SUCCESS",
            &format!("steam_id64_length={}", steam_id64.trim().len()),
        );
        if api_key.trim().is_empty() {
            self.state.log(
                correlation_id,
                "CREDENTIAL_SKIPPED_EMPTY",
                "api_key_present=false",
            );
            return self.update_steam_id(steam_id64, correlation_id);
        }
        let api_key = normalize_api_key(api_key)?;
        let encrypted = crypto::protect(&api_key)?;
        let now = timestamp();
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.unchecked_transaction().map_err(|error| {
            self.log_sql_error(correlation_id, "TRANSACTION_BEGIN", &error);
            DatabaseError::Sqlite(error)
        })?;
        self.state.log(
            correlation_id,
            "TRANSACTION_BEGIN",
            "operation=save_steam_account_configuration",
        );
        self.state.log(
            correlation_id,
            "ACCOUNT_UPSERT_BEGIN",
            "operation=provider_accounts_upsert",
        );
        upsert_account(&transaction, steam_id64, &now).map_err(|error| {
            self.log_error(correlation_id, "ACCOUNT_UPSERT_FAILED", &error);
            error
        })?;
        self.state.log(
            correlation_id,
            "ACCOUNT_UPSERT_SUCCESS",
            "operation=provider_accounts_upsert",
        );
        self.state.log(
            correlation_id,
            "CREDENTIAL_UPSERT_BEGIN",
            "operation=provider_credentials_upsert",
        );
        upsert_credential(&transaction, &encrypted, &api_key, &now).map_err(|error| {
            self.log_error(correlation_id, "CREDENTIAL_UPSERT_FAILED", &error);
            error
        })?;
        self.state.log(
            correlation_id,
            "CREDENTIAL_UPSERT_SUCCESS",
            "operation=provider_credentials_upsert",
        );
        self.state
            .log(correlation_id, "TRANSACTION_COMMIT_BEGIN", "");
        transaction.commit().map_err(|error| {
            self.log_sql_error(correlation_id, "TRANSACTION_COMMIT_FAILED", &error);
            DatabaseError::Sqlite(error)
        })?;
        self.state
            .log(correlation_id, "TRANSACTION_COMMIT_SUCCESS", "");
        drop(connection);
        self.get_provider_configuration_traced(STEAM_PROVIDER_ID, correlation_id)
    }

    pub fn update_steam_id(
        &self,
        steam_id64: &str,
        correlation_id: &str,
    ) -> Result<SteamConfigurationStatus, DatabaseError> {
        validate_steam_id(steam_id64).map_err(|error| {
            self.log_error(correlation_id, "VALIDATION_FAILED", &error);
            error
        })?;
        self.state.log(
            correlation_id,
            "VALIDATION_SUCCESS",
            &format!("steam_id64_length={}", steam_id64.trim().len()),
        );
        let now = timestamp();
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.unchecked_transaction().map_err(|error| {
            self.log_sql_error(correlation_id, "TRANSACTION_BEGIN", &error);
            DatabaseError::Sqlite(error)
        })?;
        self.state.log(
            correlation_id,
            "TRANSACTION_BEGIN",
            "operation=update_steam_id",
        );
        self.state.log(
            correlation_id,
            "PROVIDER_UPSERT_BEGIN",
            "operation=provider_exists_check provider_id=steam",
        );
        let provider_exists: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM providers WHERE id = ?1)",
                params![STEAM_PROVIDER_ID],
                |row| row.get(0),
            )
            .map_err(|error| {
                self.log_sql_error(correlation_id, "PROVIDER_UPSERT_FAILED", &error);
                DatabaseError::Sqlite(error)
            })?;
        self.state.log(
            correlation_id,
            "PROVIDER_UPSERT_SUCCESS",
            &format!("provider_id=steam exists={provider_exists}"),
        );
        self.state.log(
            correlation_id,
            "ACCOUNT_UPSERT_BEGIN",
            "operation=provider_accounts_upsert",
        );
        transaction.execute(
            "INSERT INTO provider_accounts (id, provider_id, external_account_id, enabled, configuration_status, created_at, updated_at) VALUES (?1, ?2, ?3, 1, 'partially-configured', ?4, ?4) ON CONFLICT(id) DO UPDATE SET external_account_id = excluded.external_account_id, enabled = 1, configuration_status = CASE WHEN EXISTS (SELECT 1 FROM provider_credentials WHERE provider_account_id = provider_accounts.id AND credential_type = ?5) THEN 'configured' ELSE 'partially-configured' END, updated_at = excluded.updated_at",
            params![STEAM_ACCOUNT_ID, STEAM_PROVIDER_ID, steam_id64, now, STEAM_CREDENTIAL_TYPE],
        )
        .map_err(|error| {
            self.log_sql_error(correlation_id, "ACCOUNT_UPSERT_FAILED", &error);
            DatabaseError::Sqlite(error)
        })?;
        self.state.log(
            correlation_id,
            "ACCOUNT_UPSERT_SUCCESS",
            "operation=provider_accounts_upsert",
        );
        self.state
            .log(correlation_id, "TRANSACTION_COMMIT_BEGIN", "");
        transaction.commit().map_err(|error| {
            self.log_sql_error(correlation_id, "TRANSACTION_COMMIT_FAILED", &error);
            DatabaseError::Sqlite(error)
        })?;
        self.state.log(
            correlation_id,
            "TRANSACTION_COMMIT_SUCCESS",
            "status=partially-configured",
        );
        drop(connection);
        self.get_provider_configuration_traced(STEAM_PROVIDER_ID, correlation_id)
    }

    pub fn replace_steam_api_key(
        &self,
        api_key: &str,
        correlation_id: &str,
    ) -> Result<SteamConfigurationStatus, DatabaseError> {
        let api_key = normalize_api_key(api_key)?;
        let encrypted = crypto::protect(&api_key)?;
        let now = timestamp();
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.unchecked_transaction().map_err(|error| {
            self.log_sql_error(correlation_id, "TRANSACTION_BEGIN", &error);
            DatabaseError::Sqlite(error)
        })?;
        self.state.log(
            correlation_id,
            "TRANSACTION_BEGIN",
            "operation=replace_steam_api_key",
        );
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM provider_accounts WHERE id = ?1 AND provider_id = ?2)",
            params![STEAM_ACCOUNT_ID, STEAM_PROVIDER_ID],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(DatabaseError::AccountNotConfigured);
        }
        self.state.log(
            correlation_id,
            "CREDENTIAL_UPSERT_BEGIN",
            "operation=provider_credentials_upsert",
        );
        upsert_credential(&transaction, &encrypted, &api_key, &now).map_err(|error| {
            self.log_error(correlation_id, "CREDENTIAL_UPSERT_FAILED", &error);
            error
        })?;
        self.state.log(
            correlation_id,
            "CREDENTIAL_UPSERT_SUCCESS",
            "operation=provider_credentials_upsert",
        );
        transaction
            .execute("UPDATE provider_accounts SET configuration_status = CASE WHEN external_account_id IS NOT NULL THEN 'configured' ELSE 'partially-configured' END, updated_at = ?1 WHERE id = ?2", params![now, STEAM_ACCOUNT_ID])
            .map_err(|error| {
                self.log_sql_error(correlation_id, "ACCOUNT_STATUS_UPDATE_FAILED", &error);
                DatabaseError::Sqlite(error)
            })?;
        self.state
            .log(correlation_id, "TRANSACTION_COMMIT_BEGIN", "");
        transaction.commit().map_err(|error| {
            self.log_sql_error(correlation_id, "TRANSACTION_COMMIT_FAILED", &error);
            DatabaseError::Sqlite(error)
        })?;
        self.state
            .log(correlation_id, "TRANSACTION_COMMIT_SUCCESS", "");
        drop(connection);
        self.get_provider_configuration_traced(STEAM_PROVIDER_ID, correlation_id)
    }

    pub fn disconnect_provider_account(
        &self,
        account_id: &str,
    ) -> Result<SteamConfigurationStatus, DatabaseError> {
        if account_id != STEAM_ACCOUNT_ID {
            return Err(DatabaseError::AccountNotConfigured);
        }
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "DELETE FROM provider_accounts WHERE id = ?1 AND provider_id = ?2",
            params![account_id, STEAM_PROVIDER_ID],
        )?;
        transaction.commit()?;
        drop(connection);
        self.get_provider_configuration(STEAM_PROVIDER_ID)
    }

    pub fn get_database_status(&self) -> Result<DatabaseStatus, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let schema_version = connection.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )?;
        let provider_count =
            connection.query_row("SELECT COUNT(*) FROM providers", [], |row| row.get(0))?;
        Ok(DatabaseStatus {
            path: self.state.path.to_string_lossy().into_owned(),
            schema_version,
            provider_count,
        })
    }

    pub fn begin_steam_sync(
        &self,
        found_count: i64,
        started_at: &str,
    ) -> Result<(), DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "INSERT INTO steam_sync_state(account_id, status, found_count, progress_completed, progress_total, started_at, completed_at, error_message) VALUES (?1, 'running', ?2, 0, ?2, ?3, NULL, NULL) ON CONFLICT(account_id) DO UPDATE SET status = 'running', found_count = excluded.found_count, created_count = 0, updated_count = 0, progress_completed = 0, progress_total = excluded.progress_total, duration_ms = NULL, started_at = excluded.started_at, completed_at = NULL, current_app_id = NULL, error_message = NULL",
            params![STEAM_ACCOUNT_ID, found_count, started_at],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn update_steam_sync_progress(
        &self,
        completed: i64,
        total: i64,
        app_id: Option<i64>,
    ) -> Result<(), DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "UPDATE steam_sync_state SET found_count = ?2, progress_completed = ?1, progress_total = ?2, current_app_id = ?3 WHERE account_id = ?4 AND status = 'running'",
            params![completed, total, app_id, STEAM_ACCOUNT_ID],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn get_steam_sync_status(&self) -> Result<SteamSyncStatus, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let status = connection.query_row(
            "SELECT status, found_count, created_count, updated_count, progress_completed, progress_total, duration_ms, started_at, completed_at, current_app_id, error_message FROM steam_sync_state WHERE account_id = ?1",
            params![STEAM_ACCOUNT_ID],
            |row| Ok(SteamSyncStatus { status: row.get(0)?, found_count: row.get(1)?, created_count: row.get(2)?, updated_count: row.get(3)?, progress_completed: row.get(4)?, progress_total: row.get(5)?, duration_ms: row.get(6)?, started_at: row.get(7)?, completed_at: row.get(8)?, current_app_id: row.get(9)?, error_message: row.get(10)? }),
        ).optional()?;
        let mut status = status.unwrap_or(SteamSyncStatus {
            status: "idle".to_string(),
            found_count: 0,
            created_count: 0,
            updated_count: 0,
            progress_completed: 0,
            progress_total: 0,
            duration_ms: None,
            started_at: None,
            completed_at: None,
            current_app_id: None,
            error_message: None,
        });
        if self
            .state
            .steam_sync_running
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            status.status = "running".to_string();
            status.progress_completed =
                self.state
                    .steam_sync_progress
                    .load(std::sync::atomic::Ordering::SeqCst) as i64;
            status.progress_total =
                self.state
                    .steam_sync_total
                    .load(std::sync::atomic::Ordering::SeqCst) as i64;
        }
        Ok(status)
    }

    pub fn get_steam_image_sources(&self) -> Result<Vec<SteamImageSource>, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        backfill_steam_image_sources(&connection)?;
        let mut statement = connection.prepare(
            "SELECT a.game_id, d.steam_app_id, a.asset_type, a.external_id, a.source_url,
                    a.local_path, a.mime_type, a.width, a.height, a.byte_size, a.downloaded_at
             FROM steam_game_assets a
             JOIN game_details d ON d.game_id = a.game_id
             WHERE d.steam_app_id IS NOT NULL
             ORDER BY d.steam_app_id, a.asset_type, a.external_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(SteamImageSource {
                game_id: row.get(0)?,
                app_id: row.get(1)?,
                asset_type: row.get(2)?,
                external_id: row.get(3)?,
                source_url: row.get(4)?,
                local_path: row.get(5)?,
                mime_type: row.get(6)?,
                width: row.get(7)?,
                height: row.get(8)?,
                byte_size: row.get(9)?,
                downloaded_at: row.get(10)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(DatabaseError::from)
    }

    pub fn upsert_steam_image_sources(
        &self,
        sources: &[SteamImageSource],
    ) -> Result<(), DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.unchecked_transaction()?;
        for source in sources {
            transaction.execute(
                "INSERT INTO steam_game_assets(game_id, asset_type, external_id, source_url, local_path, mime_type, width, height, byte_size, downloaded_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11) ON CONFLICT(game_id, asset_type, external_id) DO UPDATE SET source_url = excluded.source_url, local_path = CASE WHEN steam_game_assets.source_url = excluded.source_url THEN steam_game_assets.local_path ELSE NULL END, mime_type = CASE WHEN steam_game_assets.source_url = excluded.source_url THEN steam_game_assets.mime_type ELSE NULL END, width = CASE WHEN steam_game_assets.source_url = excluded.source_url THEN steam_game_assets.width ELSE NULL END, height = CASE WHEN steam_game_assets.source_url = excluded.source_url THEN steam_game_assets.height ELSE NULL END, byte_size = CASE WHEN steam_game_assets.source_url = excluded.source_url THEN steam_game_assets.byte_size ELSE NULL END, downloaded_at = CASE WHEN steam_game_assets.source_url = excluded.source_url THEN steam_game_assets.downloaded_at ELSE NULL END, updated_at = excluded.updated_at",
                params![source.game_id, source.asset_type, source.external_id, source.source_url, source.local_path, source.mime_type, source.width, source.height, source.byte_size, source.downloaded_at, timestamp()],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn persist_steam_image_records(
        &self,
        records: &[SteamImageRecord],
    ) -> Result<(), DatabaseError> {
        let now = timestamp();
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.unchecked_transaction()?;
        for record in records {
            transaction.execute(
                "UPDATE steam_game_assets SET source_url = ?1, local_path = ?2, mime_type = ?3, width = ?4, height = ?5, byte_size = ?6, downloaded_at = ?7, updated_at = ?8 WHERE game_id = ?9 AND asset_type = ?10 AND external_id = ?11",
                params![record.source_url, record.local_path, record.mime_type, record.width, record.height, record.byte_size, record.downloaded_at, now, record.game_id, record.asset_type, record.external_id],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn begin_steam_image_sync(
        &self,
        found_count: i64,
        started_at: &str,
    ) -> Result<(), DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "INSERT INTO steam_image_sync_state(account_id, status, found_count, progress_completed, progress_total, started_at, completed_at, error_message) VALUES (?1, 'running', ?2, 0, ?2, ?3, NULL, NULL) ON CONFLICT(account_id) DO UPDATE SET status = 'running', found_count = excluded.found_count, downloaded_count = 0, skipped_count = 0, progress_completed = 0, progress_total = excluded.progress_total, duration_ms = NULL, started_at = excluded.started_at, completed_at = NULL, current_app_id = NULL, error_message = NULL",
            params![STEAM_ACCOUNT_ID, found_count, started_at],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn update_steam_image_sync_progress(
        &self,
        completed: i64,
        total: i64,
        app_id: Option<i64>,
    ) -> Result<(), DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "UPDATE steam_image_sync_state SET progress_completed = ?1, progress_total = ?2, current_app_id = ?3 WHERE account_id = ?4 AND status = 'running'",
            params![completed, total, app_id, STEAM_ACCOUNT_ID],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn get_steam_image_sync_status(&self) -> Result<SteamImageSyncStatus, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let status = connection
            .query_row(
                "SELECT status, found_count, downloaded_count, skipped_count, progress_completed, progress_total, duration_ms, started_at, completed_at, current_app_id, error_message FROM steam_image_sync_state WHERE account_id = ?1",
                params![STEAM_ACCOUNT_ID],
                |row| {
                    Ok(SteamImageSyncStatus {
                        status: row.get(0)?,
                        found_count: row.get(1)?,
                        downloaded_count: row.get(2)?,
                        skipped_count: row.get(3)?,
                        progress_completed: row.get(4)?,
                        progress_total: row.get(5)?,
                        duration_ms: row.get(6)?,
                        started_at: row.get(7)?,
                        completed_at: row.get(8)?,
                        current_app_id: row.get(9)?,
                        error_message: row.get(10)?,
                    })
                },
            )
            .optional()?;
        let mut status = status.unwrap_or(SteamImageSyncStatus {
            status: "idle".to_string(),
            found_count: 0,
            downloaded_count: 0,
            skipped_count: 0,
            progress_completed: 0,
            progress_total: 0,
            duration_ms: None,
            started_at: None,
            completed_at: None,
            current_app_id: None,
            error_message: None,
        });
        if self
            .state
            .steam_image_sync_running
            .load(std::sync::atomic::Ordering::SeqCst)
        {
            status.status = "running".to_string();
            status.progress_completed =
                self.state
                    .steam_image_sync_progress
                    .load(std::sync::atomic::Ordering::SeqCst) as i64;
            status.progress_total =
                self.state
                    .steam_image_sync_total
                    .load(std::sync::atomic::Ordering::SeqCst) as i64;
        }
        Ok(status)
    }

    pub fn sync_steam_image_records(
        &self,
        records: &[SteamImageRecord],
        found_count: i64,
        skipped_count: i64,
        duration_ms: i64,
        completed_at: &str,
    ) -> Result<SteamImageSyncResult, DatabaseError> {
        let now = timestamp();
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.unchecked_transaction()?;
        for record in records {
            transaction.execute(
                "UPDATE steam_game_assets SET source_url = ?1, local_path = ?2, mime_type = ?3, width = ?4, height = ?5, byte_size = ?6, downloaded_at = ?7, updated_at = ?8 WHERE game_id = ?9 AND asset_type = ?10 AND external_id = ?11",
                params![record.source_url, record.local_path, record.mime_type, record.width, record.height, record.byte_size, record.downloaded_at, now, record.game_id, record.asset_type, record.external_id],
            )?;
        }
        transaction.execute(
            "UPDATE steam_image_sync_state SET status = 'completed', found_count = ?1, downloaded_count = ?2, skipped_count = ?3, progress_completed = progress_total, duration_ms = ?4, completed_at = ?5, current_app_id = NULL, error_message = NULL WHERE account_id = ?6",
            params![found_count, records.len() as i64, skipped_count, duration_ms, completed_at, STEAM_ACCOUNT_ID],
        )?;
        transaction.commit()?;
        Ok(SteamImageSyncResult {
            status: "completed".to_string(),
            found_count,
            downloaded_count: records.len() as i64,
            skipped_count,
            duration_ms,
            completed_at: Some(completed_at.to_string()),
        })
    }

    pub fn fail_steam_image_sync(
        &self,
        duration_ms: i64,
        error_message: &str,
    ) -> Result<(), DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "UPDATE steam_image_sync_state SET status = 'error', duration_ms = ?1, completed_at = ?2, current_app_id = NULL, error_message = ?3 WHERE account_id = ?4",
            params![duration_ms, timestamp(), error_message, STEAM_ACCOUNT_ID],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn cancel_steam_image_sync(&self, duration_ms: i64) -> Result<(), DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "UPDATE steam_image_sync_state SET status = 'cancelled', duration_ms = ?1, completed_at = ?2, current_app_id = NULL, error_message = NULL WHERE account_id = ?3",
            params![duration_ms, timestamp(), STEAM_ACCOUNT_ID],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn sync_steam_games(
        &self,
        games: &[SteamLibraryGame],
        installed_scope: bool,
        duration_ms: i64,
        completed_at: &str,
    ) -> Result<SteamSyncResult, DatabaseError> {
        let now = timestamp();
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.unchecked_transaction()?;
        if installed_scope {
            transaction.execute(
                "UPDATE game_details SET steam_installed = 0 WHERE steam_app_id IS NOT NULL",
                [],
            )?;
        }
        let mut created_count = 0_i64;
        let mut updated_count = 0_i64;
        for game in games {
            let app_id = game.app_id.to_string();
            let existing: Option<String> = transaction.query_row(
                "SELECT game_id FROM game_provider_links WHERE provider_id = ?1 AND external_id = ?2",
                params![STEAM_PROVIDER_ID, app_id],
                |row| row.get(0),
            ).optional()?;
            let is_new = existing.is_none();
            let game_id = existing.unwrap_or_else(|| format!("steam-{}", game.app_id));
            if is_new {
                transaction.execute(
                    "INSERT INTO games(id, title, sort_title, provider, platform, created_at, updated_at) VALUES (?1, ?2, ?3, 'Steam', 'PC', ?4, ?4)",
                    params![game_id, game.name, game.name.to_lowercase(), now],
                )?;
                created_count += 1;
            } else if game.should_persist {
                updated_count += 1;
            }
            transaction.execute(
                "INSERT INTO game_provider_links(game_id, provider_id, external_id, is_owned, last_synced_at) VALUES (?1, ?2, ?3, 1, ?4) ON CONFLICT(provider_id, external_id) DO UPDATE SET is_owned = 1, last_synced_at = excluded.last_synced_at",
                params![game_id, STEAM_PROVIDER_ID, app_id, now],
            )?;
            if game.should_persist {
                if let Some(details) = game.details.as_ref() {
                    if details.complete {
                        upsert_steam_details(&transaction, &game_id, game, details, &now)?;
                        replace_steam_child_rows(&transaction, &game_id, details, true)?;
                    } else {
                        upsert_steam_base(&transaction, &game_id, game, details, &now)?;
                    }
                }
            }
            if let Some(installed) = game.installed {
                transaction.execute(
                    "UPDATE game_details SET steam_installed = ?1 WHERE game_id = ?2",
                    params![bool_value(Some(installed)), game_id],
                )?;
            }
            transaction.execute(
                "UPDATE games SET updated_at = ?1 WHERE id = ?2",
                params![now, game_id],
            )?;
        }
        transaction.execute(
            "UPDATE provider_accounts SET last_sync_at = ?1, updated_at = ?1 WHERE id = ?2 AND provider_id = ?3",
            params![completed_at, STEAM_ACCOUNT_ID, STEAM_PROVIDER_ID],
        )?;
        transaction.execute(
            "UPDATE steam_sync_state SET status = 'completed', created_count = ?1, updated_count = ?2, progress_completed = progress_total, duration_ms = ?3, completed_at = ?4, current_app_id = NULL, error_message = NULL WHERE account_id = ?5",
            params![created_count, updated_count, duration_ms, completed_at, STEAM_ACCOUNT_ID],
        )?;
        transaction.commit()?;
        Ok(SteamSyncResult {
            status: "completed".to_string(),
            found_count: games.len() as i64,
            created_count,
            updated_count,
            duration_ms,
            completed_at: Some(completed_at.to_string()),
        })
    }

    pub fn fail_steam_sync(
        &self,
        duration_ms: i64,
        error_message: &str,
    ) -> Result<(), DatabaseError> {
        let completed_at = timestamp();
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute("UPDATE steam_sync_state SET status = 'error', duration_ms = ?1, completed_at = ?2, error_message = ?3 WHERE account_id = ?4", params![duration_ms, completed_at, error_message, STEAM_ACCOUNT_ID])?;
        transaction.commit()?;
        Ok(())
    }

    pub fn cancel_steam_sync(&self, duration_ms: i64) -> Result<(), DatabaseError> {
        let completed_at = timestamp();
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute("UPDATE steam_sync_state SET status = 'cancelled', duration_ms = ?1, completed_at = ?2, current_app_id = NULL, error_message = NULL WHERE account_id = ?3", params![duration_ms, completed_at, STEAM_ACCOUNT_ID])?;
        transaction.commit()?;
        Ok(())
    }
}

fn resolve_media_asset_path(
    state: &DatabaseState,
    game_id: &str,
    media_type: &str,
    value: &str,
    fallback: &str,
) -> String {
    if !media_trace_enabled(game_id) {
        return resolve_local_asset_path(state, value, fallback);
    }
    let lookup_started = std::time::Instant::now();
    state.log(
        "media-timing",
        "CACHE_METADATA_HIT",
        &format!(
            "timestamp_ms={} gameId={} type={} path={}",
            epoch_millis(),
            game_id,
            media_type,
            value
        ),
    );
    state.log(
        "media-timing",
        "LOCAL_FILE_LOOKUP_START",
        &format!(
            "timestamp_ms={} gameId={} type={} path={}",
            epoch_millis(),
            game_id,
            media_type,
            value
        ),
    );
    let resolved = resolve_local_asset_path(state, value, fallback);
    let found = !is_remote_asset(&resolved) && Path::new(&resolved).is_file();
    if found {
        state.log(
            "media-timing",
            "LOCAL_FILE_FOUND",
            &format!(
                "timestamp_ms={} gameId={} type={} path={} duration_ms={}",
                epoch_millis(),
                game_id,
                media_type,
                resolved,
                lookup_started.elapsed().as_micros() as f64 / 1000.0
            ),
        );
    }
    resolved
}

fn media_trace_enabled(game_id: &str) -> bool {
    std::env::var("LUDECK_MEDIA_TRACE_GAME_ID")
        .map(|value| value == game_id)
        .unwrap_or(false)
}

pub(crate) fn resolve_local_asset_path(
    state: &DatabaseState,
    value: &str,
    fallback: &str,
) -> String {
    if value.is_empty() || is_remote_asset(value) {
        return value.to_string();
    }

    let original = Path::new(value);
    if original.is_file() {
        return value.to_string();
    }

    let Some(file_name) = original.file_name() else {
        return value.to_string();
    };
    let current_path = if original.starts_with(Path::new("artwork")) {
        state.data_directory.cache_directory().join(original)
    } else if original.starts_with(Path::new("cache")) {
        state.data_directory.root().join(original)
    } else {
        state
            .data_directory
            .steam_images_directory()
            .join(file_name)
    };
    if current_path.is_file() {
        current_path.to_string_lossy().into_owned()
    } else {
        if fallback.is_empty() {
            value.to_string()
        } else {
            fallback.to_string()
        }
    }
}

fn resolve_optional_local_asset_path(state: &DatabaseState, value: &str) -> Option<String> {
    let resolved = resolve_local_asset_path(state, value, "");
    Path::new(&resolved).is_file().then_some(resolved)
}

fn is_remote_asset(value: &str) -> bool {
    value.starts_with("http://")
        || value.starts_with("https://")
        || value.starts_with("data:")
        || value.starts_with("asset:")
}

fn epoch_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or_default()
}

fn asset_is_available(value: &str) -> bool {
    is_remote_asset(value) || (!value.is_empty() && Path::new(value).is_file())
}

fn upsert_steam_base(
    transaction: &Transaction<'_>,
    game_id: &str,
    game: &SteamLibraryGame,
    details: &SteamGameDetails,
    now: &str,
) -> Result<(), DatabaseError> {
    transaction.execute(
        "INSERT INTO game_details(game_id, steam_app_id, steam_name, steam_total_playtime_minutes, steam_playtime_2weeks_minutes, steam_last_played_at, steam_owned, steam_has_community_visible_stats, steam_icon_url, steam_logo_url, steam_updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7, ?8, ?9, ?10) ON CONFLICT(game_id) DO UPDATE SET steam_app_id = excluded.steam_app_id, steam_name = excluded.steam_name, steam_total_playtime_minutes = excluded.steam_total_playtime_minutes, steam_playtime_2weeks_minutes = excluded.steam_playtime_2weeks_minutes, steam_last_played_at = excluded.steam_last_played_at, steam_owned = excluded.steam_owned, steam_has_community_visible_stats = excluded.steam_has_community_visible_stats, steam_icon_url = excluded.steam_icon_url, steam_logo_url = excluded.steam_logo_url, steam_updated_at = excluded.steam_updated_at",
        params![game_id, details.app_id, details.name.as_deref().unwrap_or(&game.name), game.total_playtime_minutes, game.playtime_2weeks_minutes, game.last_played_at, bool_value(game.has_community_visible_stats), game.icon_url, game.logo_url, now],
    )?;
    Ok(())
}

fn backfill_steam_image_sources(connection: &rusqlite::Connection) -> Result<(), DatabaseError> {
    let now = timestamp();
    connection.execute(
        "INSERT OR IGNORE INTO steam_game_assets(game_id, asset_type, external_id, source_url, updated_at)
         SELECT game_id, 'horizontal_cover', CAST(steam_app_id AS TEXT), COALESCE(steam_header_url, 'https://cdn.cloudflare.steamstatic.com/steam/apps/' || steam_app_id || '/library_header_2x.jpg'), ?1
         FROM game_details WHERE steam_app_id IS NOT NULL",
        params![now],
    )?;
    connection.execute(
        "INSERT OR IGNORE INTO steam_game_assets(game_id, asset_type, external_id, source_url, updated_at)
         SELECT game_id, 'logo', CAST(steam_app_id AS TEXT), COALESCE(steam_logo_url, 'https://cdn.cloudflare.steamstatic.com/steam/apps/' || steam_app_id || '/logo.png'), ?1
         FROM game_details WHERE steam_app_id IS NOT NULL",
        params![now],
    )?;
    connection.execute(
        "INSERT OR IGNORE INTO steam_game_assets(game_id, asset_type, external_id, source_url, updated_at)
         SELECT game_id, 'hero', CAST(steam_app_id AS TEXT), COALESCE(steam_background_url, 'https://cdn.cloudflare.steamstatic.com/steam/apps/' || steam_app_id || '/library_hero_2x.jpg'), ?1
         FROM game_details WHERE steam_app_id IS NOT NULL",
        params![now],
    )?;
    connection.execute(
        "INSERT OR IGNORE INTO steam_game_assets(game_id, asset_type, external_id, source_url, updated_at)
         SELECT game_id, 'vertical_cover', CAST(steam_app_id AS TEXT), 'https://cdn.cloudflare.steamstatic.com/steam/apps/' || steam_app_id || '/library_600x900_2x.jpg', ?1
         FROM game_details WHERE steam_app_id IS NOT NULL",
        params![now],
    )?;
    connection.execute(
        "INSERT OR IGNORE INTO steam_game_assets(game_id, asset_type, external_id, source_url, updated_at)
         SELECT game_id, 'icon', CAST(steam_app_id AS TEXT), steam_icon_url, ?1
         FROM game_details WHERE steam_app_id IS NOT NULL AND steam_icon_url IS NOT NULL AND steam_icon_url <> ''",
        params![now],
    )?;
    connection.execute(
        "INSERT OR IGNORE INTO steam_game_assets(game_id, asset_type, external_id, source_url, updated_at)
         SELECT game_id, 'screenshot', external_id, full_url, ?1
         FROM steam_game_media WHERE media_type = 'screenshot' AND full_url IS NOT NULL AND full_url <> ''",
        params![now],
    )?;
    Ok(())
}

fn upsert_steam_details(
    transaction: &Transaction<'_>,
    game_id: &str,
    game: &SteamLibraryGame,
    details: &SteamGameDetails,
    now: &str,
) -> Result<(), DatabaseError> {
    let unlocked = details
        .achievements
        .iter()
        .filter(|achievement| achievement.unlocked)
        .count() as i64;
    let total = details
        .achievement_total
        .unwrap_or(details.achievements.len() as i64);
    let progress = (total > 0).then(|| unlocked as f64 * 100.0 / total as f64);
    let stats = stats_json(&details.stats);
    transaction.execute(
        "INSERT INTO game_details(game_id, steam_app_id, steam_name, steam_total_playtime_minutes, steam_playtime_2weeks_minutes, steam_last_played_at, steam_installed, steam_owned, steam_hidden, steam_acquired_at, steam_achievement_total, steam_achievement_unlocked, steam_achievement_progress, steam_review_score, steam_review_count, steam_review_score_description, steam_controller_support, steam_release_date, steam_description, steam_short_description, steam_website, steam_min_requirements_json, steam_recommended_requirements_json, steam_header_url, steam_background_url, steam_price_json, steam_stats_json, steam_early_access, steam_adult_content, steam_multiplayer, steam_single_player, steam_cloud, steam_trading_cards, steam_workshop, steam_family_sharing, steam_has_community_visible_stats, steam_icon_url, steam_logo_url, steam_updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, 1, NULL, NULL, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35) ON CONFLICT(game_id) DO UPDATE SET steam_app_id = excluded.steam_app_id, steam_name = excluded.steam_name, steam_total_playtime_minutes = excluded.steam_total_playtime_minutes, steam_playtime_2weeks_minutes = excluded.steam_playtime_2weeks_minutes, steam_last_played_at = excluded.steam_last_played_at, steam_owned = excluded.steam_owned, steam_achievement_total = excluded.steam_achievement_total, steam_achievement_unlocked = excluded.steam_achievement_unlocked, steam_achievement_progress = excluded.steam_achievement_progress, steam_review_score = excluded.steam_review_score, steam_review_count = excluded.steam_review_count, steam_review_score_description = excluded.steam_review_score_description, steam_controller_support = excluded.steam_controller_support, steam_release_date = excluded.steam_release_date, steam_description = excluded.steam_description, steam_short_description = excluded.steam_short_description, steam_website = excluded.steam_website, steam_min_requirements_json = excluded.steam_min_requirements_json, steam_recommended_requirements_json = excluded.steam_recommended_requirements_json, steam_header_url = COALESCE(excluded.steam_header_url, game_details.steam_header_url), steam_background_url = COALESCE(excluded.steam_background_url, game_details.steam_background_url), steam_price_json = excluded.steam_price_json, steam_stats_json = excluded.steam_stats_json, steam_early_access = excluded.steam_early_access, steam_adult_content = excluded.steam_adult_content, steam_multiplayer = excluded.steam_multiplayer, steam_single_player = excluded.steam_single_player, steam_cloud = excluded.steam_cloud, steam_trading_cards = excluded.steam_trading_cards, steam_workshop = excluded.steam_workshop, steam_family_sharing = excluded.steam_family_sharing, steam_has_community_visible_stats = excluded.steam_has_community_visible_stats, steam_icon_url = COALESCE(excluded.steam_icon_url, game_details.steam_icon_url), steam_logo_url = COALESCE(excluded.steam_logo_url, game_details.steam_logo_url), steam_updated_at = excluded.steam_updated_at",
        params![game_id, details.app_id, details.name.as_deref().unwrap_or(&game.name), game.total_playtime_minutes, game.playtime_2weeks_minutes, game.last_played_at, total, unlocked, progress, details.review_score, details.review_count, details.review_score_description, details.controller_support, details.release_date, details.description, details.short_description, details.website, json_value(&details.minimum_requirements), json_value(&details.recommended_requirements), details.header_url, details.background_url, json_value(&details.price), stats, bool_value(details.early_access), bool_value(details.adult_content), bool_value(details.multiplayer), bool_value(details.single_player), bool_value(details.cloud), bool_value(details.trading_cards), bool_value(details.workshop), bool_value(details.family_sharing), bool_value(game.has_community_visible_stats), game.icon_url, game.logo_url, now],
    )?;
    Ok(())
}

fn replace_steam_child_rows(
    transaction: &Transaction<'_>,
    game_id: &str,
    details: &SteamGameDetails,
    include_assets: bool,
) -> Result<(), DatabaseError> {
    for table in [
        "steam_game_tags",
        "steam_game_genres",
        "steam_game_categories",
        "steam_game_developers",
        "steam_game_publishers",
        "steam_game_languages",
        "steam_game_platforms",
        "steam_game_stats",
        "steam_game_media",
        "steam_game_dlc",
    ] {
        transaction.execute(
            &format!("DELETE FROM {table} WHERE game_id = ?1"),
            params![game_id],
        )?;
    }
    for value in &details.tags {
        transaction.execute(
            "INSERT OR IGNORE INTO steam_game_tags(game_id, value) VALUES (?1, ?2)",
            params![game_id, value],
        )?;
    }
    for value in &details.genres {
        transaction.execute(
            "INSERT OR IGNORE INTO steam_game_genres(game_id, value) VALUES (?1, ?2)",
            params![game_id, value],
        )?;
    }
    for item in &details.categories {
        transaction.execute(
            "INSERT OR IGNORE INTO steam_game_categories(game_id, category_id, value) VALUES (?1, ?2, ?3)",
            params![game_id, item.id, item.value],
        )?;
    }
    for value in &details.developers {
        transaction.execute(
            "INSERT OR IGNORE INTO steam_game_developers(game_id, value) VALUES (?1, ?2)",
            params![game_id, value],
        )?;
    }
    for value in &details.publishers {
        transaction.execute(
            "INSERT OR IGNORE INTO steam_game_publishers(game_id, value) VALUES (?1, ?2)",
            params![game_id, value],
        )?;
    }
    for value in &details.languages {
        transaction.execute(
            "INSERT OR IGNORE INTO steam_game_languages(game_id, value) VALUES (?1, ?2)",
            params![game_id, value],
        )?;
    }
    for value in &details.platforms {
        transaction.execute(
            "INSERT OR IGNORE INTO steam_game_platforms(game_id, value) VALUES (?1, ?2)",
            params![game_id, value],
        )?;
    }
    for achievement in &details.achievements {
        transaction.execute("INSERT OR IGNORE INTO steam_game_achievements(game_id, api_name, display_name, description, achieved, unlock_time) VALUES (?1, ?2, ?3, ?4, ?5, ?6)", params![game_id, achievement.api_name, achievement.display_name, achievement.description, achievement.unlocked, achievement.unlock_time])?;
    }
    for stat in &details.stats {
        transaction.execute(
            "INSERT OR IGNORE INTO steam_game_stats(game_id, name, value_json) VALUES (?1, ?2, ?3)",
            params![game_id, stat.name, stat.value.to_string()],
        )?;
    }
    for media in details.movies.iter().chain(details.screenshots.iter()) {
        transaction.execute("INSERT OR IGNORE INTO steam_game_media(game_id, media_type, external_id, name, thumbnail_url, full_url, metadata_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)", params![game_id, media.media_type, media.external_id, media.name, media.thumbnail_url, media.full_url, media.metadata.to_string()])?;
    }
    for app_id in &details.dlc {
        transaction.execute(
            "INSERT OR IGNORE INTO steam_game_dlc(game_id, dlc_app_id) VALUES (?1, ?2)",
            params![game_id, app_id],
        )?;
    }
    if include_assets {
        for asset in &details.assets {
            transaction.execute(
                "INSERT INTO steam_game_assets(game_id, asset_type, external_id, source_url, updated_at) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT(game_id, asset_type, external_id) DO UPDATE SET source_url = excluded.source_url, local_path = CASE WHEN steam_game_assets.source_url = excluded.source_url THEN steam_game_assets.local_path ELSE NULL END, mime_type = CASE WHEN steam_game_assets.source_url = excluded.source_url THEN steam_game_assets.mime_type ELSE NULL END, width = CASE WHEN steam_game_assets.source_url = excluded.source_url THEN steam_game_assets.width ELSE NULL END, height = CASE WHEN steam_game_assets.source_url = excluded.source_url THEN steam_game_assets.height ELSE NULL END, byte_size = CASE WHEN steam_game_assets.source_url = excluded.source_url THEN steam_game_assets.byte_size ELSE NULL END, downloaded_at = CASE WHEN steam_game_assets.source_url = excluded.source_url THEN steam_game_assets.downloaded_at ELSE NULL END, updated_at = excluded.updated_at",
                params![game_id, asset.asset_type, asset.external_id, asset.source_url, timestamp()],
            )?;
        }
    }
    Ok(())
}

fn json_value(value: &Option<Value>) -> Option<String> {
    value.as_ref().map(Value::to_string)
}
fn bool_value(value: Option<bool>) -> Option<i64> {
    value.map(i64::from)
}

fn parse_resolution_mode(value: &str) -> DisplayResolutionMode {
    match value.trim().to_ascii_uppercase().as_str() {
        "CUSTOM" => DisplayResolutionMode::Custom,
        _ => DisplayResolutionMode::System,
    }
}

fn parse_refresh_rate_mode(value: &str) -> DisplayRefreshRateMode {
    match value.trim().to_ascii_uppercase().as_str() {
        "CUSTOM" => DisplayRefreshRateMode::Custom,
        _ => DisplayRefreshRateMode::System,
    }
}

fn parse_hdr_mode(value: &str) -> DisplayHdrMode {
    match value.trim().to_ascii_uppercase().as_str() {
        "OFF" => DisplayHdrMode::Off,
        "ON" => DisplayHdrMode::On,
        "AUTO" => DisplayHdrMode::Auto,
        _ => DisplayHdrMode::System,
    }
}

fn parse_rtx_hdr_preset(value: &Option<String>) -> Option<crate::rtx_hdr::RtxHdrPreset> {
    match value
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_ascii_uppercase()
        .as_str()
    {
        "NATURAL" => Some(crate::rtx_hdr::RtxHdrPreset::Natural),
        "VIBRANT" => Some(crate::rtx_hdr::RtxHdrPreset::Vibrant),
        _ => None,
    }
}

fn display_resolution_mode_name(value: DisplayResolutionMode) -> &'static str {
    match value {
        DisplayResolutionMode::System => "SYSTEM",
        DisplayResolutionMode::Custom => "CUSTOM",
    }
}

fn display_refresh_rate_mode_name(value: DisplayRefreshRateMode) -> &'static str {
    match value {
        DisplayRefreshRateMode::System => "SYSTEM",
        DisplayRefreshRateMode::Custom => "CUSTOM",
    }
}

fn display_hdr_mode_name(value: DisplayHdrMode) -> &'static str {
    match value {
        DisplayHdrMode::System => "SYSTEM",
        DisplayHdrMode::Off => "OFF",
        DisplayHdrMode::On => "ON",
        DisplayHdrMode::Auto => "AUTO",
    }
}

fn rtx_hdr_preset_name(value: crate::rtx_hdr::RtxHdrPreset) -> &'static str {
    match value {
        crate::rtx_hdr::RtxHdrPreset::Natural => "NATURAL",
        crate::rtx_hdr::RtxHdrPreset::Vibrant => "VIBRANT",
    }
}

fn stats_json(stats: &[crate::steam::SteamStat]) -> Option<String> {
    let mut values = Map::new();
    for stat in stats {
        values.insert(stat.name.clone(), stat.value.clone());
    }
    Some(Value::Object(values).to_string())
}

fn upsert_account(
    transaction: &Transaction<'_>,
    steam_id64: &str,
    now: &str,
) -> Result<(), DatabaseError> {
    transaction.execute(
        "INSERT INTO provider_accounts (id, provider_id, external_account_id, display_name, enabled, configuration_status, created_at, updated_at) VALUES (?1, ?2, ?3, NULL, 1, 'configured', ?4, ?4) ON CONFLICT(id) DO UPDATE SET external_account_id = excluded.external_account_id, enabled = 1, configuration_status = 'configured', updated_at = excluded.updated_at",
        params![STEAM_ACCOUNT_ID, STEAM_PROVIDER_ID, steam_id64, now],
    )?;
    Ok(())
}

fn upsert_credential(
    transaction: &Transaction<'_>,
    encrypted: &[u8],
    api_key: &str,
    now: &str,
) -> Result<(), DatabaseError> {
    upsert_provider_credential(
        transaction,
        STEAM_ACCOUNT_ID,
        STEAM_CREDENTIAL_TYPE,
        encrypted,
        api_key,
        now,
    )
}

fn upsert_provider_credential(
    transaction: &Transaction<'_>,
    account_id: &str,
    credential_type: &str,
    encrypted: &[u8],
    api_key: &str,
    now: &str,
) -> Result<(), DatabaseError> {
    transaction.execute(
        "INSERT INTO provider_credentials (id, provider_account_id, credential_type, encrypted_value, protection_method, masked_suffix, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, 'windows_dpapi_current_user', ?5, ?6, ?6) ON CONFLICT(provider_account_id, credential_type) DO UPDATE SET encrypted_value = excluded.encrypted_value, protection_method = excluded.protection_method, masked_suffix = excluded.masked_suffix, updated_at = excluded.updated_at",
        params![format!("{account_id}-{credential_type}"), account_id, credential_type, encrypted, suffix(api_key), now],
    )?;
    Ok(())
}

fn validate_steam_id(value: &str) -> Result<(), DatabaseError> {
    let normalized = value.trim();
    if normalized.len() != 17
        || !normalized
            .chars()
            .all(|character| character.is_ascii_digit())
    {
        return Err(DatabaseError::InvalidSteamId);
    }
    Ok(())
}

fn normalize_api_key(value: &str) -> Result<String, DatabaseError> {
    let normalized = value.trim().to_string();
    if normalized.len() < 16
        || normalized.len() > 64
        || !normalized
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-_".contains(character))
    {
        return Err(DatabaseError::InvalidApiKey);
    }
    Ok(normalized)
}

fn normalize_steamgriddb_api_key(value: &str) -> Result<String, DatabaseError> {
    let normalized = value.trim().to_string();
    if normalized.is_empty()
        || normalized.len() > 256
        || normalized
            .chars()
            .any(|character| character == '\r' || character == '\n')
    {
        return Err(DatabaseError::InvalidApiKey);
    }
    Ok(normalized)
}

fn normalize_rapidapi_reviews_api_key(value: &str) -> Result<String, DatabaseError> {
    let normalized = value.trim().to_string();
    if normalized.len() < 10
        || normalized.len() > 256
        || normalized
            .chars()
            .any(|character| character == '\r' || character == '\n')
    {
        return Err(DatabaseError::InvalidApiKey);
    }
    Ok(normalized)
}

fn normalize_translation_api_key(value: &str) -> Result<String, DatabaseError> {
    let normalized = value.trim().to_string();
    if normalized.len() < 10
        || normalized.len() > 256
        || normalized
            .chars()
            .any(|character| character == '\r' || character == '\n')
    {
        return Err(DatabaseError::InvalidTranslationApiKey);
    }
    Ok(normalized)
}

fn suffix(value: &str) -> String {
    value
        .chars()
        .rev()
        .take(4)
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}
fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn steam_game_metrics(
    connection: &Connection,
    game_id: &str,
) -> Result<Option<SteamGameMetrics>, rusqlite::Error> {
    connection
        .query_row(
            "SELECT COALESCE(d.steam_total_playtime_minutes, 0),
                    d.steam_last_played_at, COALESCE(g.progress, 0),
                    d.steam_achievement_total, d.steam_achievement_unlocked,
                    d.steam_active_players,
                    COALESCE((SELECT SUM(COALESCE(duration_seconds, 0))
                              FROM game_sessions
                              WHERE game_id = g.id AND source = 'lumadeck'), 0),
                    (SELECT MAX(CAST(ended_at AS INTEGER))
                     FROM game_sessions
                     WHERE game_id = g.id AND source = 'lumadeck'
                       AND ended_at IS NOT NULL)
             FROM games g LEFT JOIN game_details d ON d.game_id = g.id
             WHERE g.id = ?1",
            params![game_id],
            |row| {
                let steam_last_played_at: Option<String> = row.get(1)?;
                let local_playtime_seconds: i64 = row.get(6)?;
                let local_last_played_at: Option<i64> = row.get(7)?;
                let local_last_played_at = local_last_played_at.map(|value| value.to_string());
                let last_played_at = match (steam_last_played_at, local_last_played_at) {
                    (Some(steam), Some(local))
                        if activity_timestamp(&local) > activity_timestamp(&steam) =>
                    {
                        Some(local)
                    }
                    (Some(steam), _) => Some(steam),
                    (None, local) => local,
                };
                Ok(SteamGameMetrics {
                    total_playtime_minutes: row.get::<_, i64>(0)? + local_playtime_seconds / 60,
                    last_played_at,
                    progress: row.get(2)?,
                    achievement_total: row.get(3)?,
                    achievement_unlocked: row.get(4)?,
                    active_players: row.get(5)?,
                })
            },
        )
        .optional()
}

fn activity_timestamp(value: &str) -> i64 {
    value.parse::<i64>().unwrap_or(0)
}

fn activity_streak(sessions: &[ActivitySession]) -> ActivityStreak {
    let days: HashSet<i64> = sessions
        .iter()
        .map(|session| activity_timestamp(&session.started_at) / 86_400)
        .collect();
    let Some(latest) = days.iter().copied().max() else {
        return ActivityStreak {
            current_days: 0,
            best_days: 0,
        };
    };

    let mut current_days = 0;
    let mut day = latest;
    while days.contains(&day) {
        current_days += 1;
        day -= 1;
    }

    let mut best_days = 0;
    for candidate in &days {
        if days.contains(&(candidate - 1)) {
            continue;
        }
        let mut length = 0;
        let mut cursor = *candidate;
        while days.contains(&cursor) {
            length += 1;
            cursor += 1;
        }
        best_days = best_days.max(length);
    }
    ActivityStreak {
        current_days,
        best_days,
    }
}

fn activity_stat_label(value: &str) -> String {
    value
        .split(['_', '-'])
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut characters = part.chars();
            match characters.next() {
                Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
fn not_configured() -> SteamConfigurationStatus {
    SteamConfigurationStatus {
        provider_id: STEAM_PROVIDER_ID.to_string(),
        account_id: None,
        steam_id64_masked: None,
        api_key_configured: false,
        api_key_masked: None,
        status: "not-configured".to_string(),
    }
}
fn status_from_values(
    account_id: String,
    steam_id: Option<String>,
    api_key_configured: bool,
    api_key_masked: Option<String>,
    credential_unavailable: bool,
    stored_status: String,
) -> SteamConfigurationStatus {
    let status = if credential_unavailable {
        "credential-unavailable"
    } else if steam_id.is_some() && api_key_configured {
        "configured"
    } else if steam_id.is_some() || api_key_configured {
        "partially-configured"
    } else {
        stored_status.as_str()
    };
    SteamConfigurationStatus {
        provider_id: STEAM_PROVIDER_ID.to_string(),
        account_id: Some(account_id),
        steam_id64_masked: steam_id.map(|value| mask_steam_id(&value)),
        api_key_configured,
        api_key_masked,
        status: status.to_string(),
    }
}
fn mask_steam_id(value: &str) -> String {
    format!("{}••••••••", value.chars().take(7).collect::<String>())
}

#[cfg(all(test, windows))]
mod tests {
    use super::{DatabaseError, SettingsRepository};
    use crate::consensus::{ConsensusSources, GameReviewConsensus};
    use crate::data_directory::DataDirectoryResolver;
    use crate::display::{
        DisplayHdrMode, DisplayProfile, DisplayProfileSnapshot, DisplayRefreshRateMode,
        DisplayResolutionMode, PendingDisplayProfileRestore, PendingDisplayRestore,
    };
    use crate::settings::DatabaseState;
    use crate::steam::{SteamGameDetails, SteamLibraryGame, SteamNamedValue, SteamStat};
    use rusqlite::params;
    use serde_json::json;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn account_lifecycle_is_transactional_and_masked() {
        let directory = std::env::temp_dir().join(format!(
            "lumadeck-settings-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let state =
            DatabaseState::open(DataDirectoryResolver::for_app_data(&directory)).expect("database");
        let repository = SettingsRepository::new(&state);

        let partial = repository
            .update_steam_id("76561198012345678", "test-correlation")
            .expect("partial Steam account");
        assert_eq!(partial.status, "partially-configured");

        let configured = repository
            .replace_steam_api_key("ABCDEFGHIJKLMNOP", "test-correlation")
            .expect("DPAPI credential");
        assert_eq!(configured.status, "configured");
        let credentials = repository
            .get_steam_credentials()
            .expect("Steam credentials");
        assert_eq!(credentials.steam_id64, "76561198012345678");
        assert_eq!(credentials.api_key, "ABCDEFGHIJKLMNOP");
        assert_eq!(
            configured.api_key_masked.as_deref(),
            Some("••••••••••••MNOP")
        );

        let connection = state.connection.lock().expect("database lock");
        let encrypted: Vec<u8> = connection
            .query_row(
                "SELECT encrypted_value FROM provider_credentials WHERE provider_account_id = 'steam-default'",
                [],
                |row| row.get(0),
            )
            .expect("credential blob");
        assert!(!String::from_utf8_lossy(&encrypted).contains("ABCDEFGHIJKLMNOP"));
        drop(connection);

        let disconnected = repository
            .disconnect_provider_account("steam-default")
            .expect("disconnect");
        assert_eq!(disconnected.status, "not-configured");
        drop(state);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn save_only_steam_id_creates_partial_account_without_credential() {
        let directory = std::env::temp_dir().join(format!(
            "lumadeck-steamid-only-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let state =
            DatabaseState::open(DataDirectoryResolver::for_app_data(&directory)).expect("database");
        let repository = SettingsRepository::new(&state);
        let steam_id = "76561198012345678";

        let database_status = repository.get_database_status().expect("database status");
        assert!(database_status.path.ends_with("lumadeck.db"));
        assert_eq!(database_status.schema_version, 34);
        assert_eq!(database_status.provider_count, 7);
        let connection = state.connection.lock().expect("database lock");
        let foreign_keys: i64 = connection
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .expect("foreign keys pragma");
        assert_eq!(foreign_keys, 1);
        drop(connection);

        let partial = repository
            .save_steam_account_configuration(steam_id, "", "test-correlation")
            .expect("SteamID-only save");
        assert_eq!(partial.status, "partially-configured");
        assert_eq!(partial.account_id.as_deref(), Some("steam-default"));
        assert!(!partial.api_key_configured);

        let connection = state.connection.lock().expect("database lock");
        let provider_exists: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM providers WHERE id = 'steam'",
                [],
                |row| row.get(0),
            )
            .expect("Steam provider");
        let account: (i64, String, String) = connection
            .query_row(
                "SELECT COUNT(*), external_account_id, configuration_status FROM provider_accounts WHERE id = 'steam-default'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("Steam account");
        let credential_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM provider_credentials WHERE provider_account_id = 'steam-default'",
                [],
                |row| row.get(0),
            )
            .expect("credential count");
        assert_eq!(provider_exists, 1);
        assert_eq!(
            account,
            (1, steam_id.to_string(), "partially-configured".to_string())
        );
        assert_eq!(credential_count, 0);
        drop(connection);

        let updated = repository
            .save_steam_account_configuration("76561198087654321", "", "test-correlation")
            .expect("second SteamID-only save");
        assert_eq!(updated.status, "partially-configured");

        let connection = state.connection.lock().expect("database lock");
        let account_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM provider_accounts WHERE id = 'steam-default'",
                [],
                |row| row.get(0),
            )
            .expect("account count");
        let updated_id: String = connection
            .query_row(
                "SELECT external_account_id FROM provider_accounts WHERE id = 'steam-default'",
                [],
                |row| row.get(0),
            )
            .expect("updated SteamID");
        assert_eq!(account_count, 1);
        assert_eq!(updated_id, "76561198087654321");
        drop(connection);

        drop(state);
        let reopened = DatabaseState::open(DataDirectoryResolver::for_app_data(&directory))
            .expect("reopen database");
        let persisted = SettingsRepository::new(&reopened)
            .get_provider_configuration("steam")
            .expect("persisted configuration");
        assert_eq!(persisted.status, "partially-configured");
        assert_eq!(persisted.account_id.as_deref(), Some("steam-default"));
        assert!(!persisted.api_key_configured);
        assert!(persisted.steam_id64_masked.is_some());

        let invalid = SettingsRepository::new(&reopened)
            .save_steam_account_configuration("7656119801234567", "", "test-correlation")
            .expect_err("invalid SteamID must fail validation");
        assert!(matches!(
            invalid,
            crate::settings::DatabaseError::InvalidSteamId
        ));
        let connection = reopened.connection.lock().expect("database lock");
        let persisted_id: String = connection
            .query_row(
                "SELECT external_account_id FROM provider_accounts WHERE id = 'steam-default'",
                [],
                |row| row.get(0),
            )
            .expect("persisted SteamID");
        assert_eq!(persisted_id, "76561198087654321");
        drop(connection);
        drop(reopened);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn recovers_persisted_running_image_sync_on_restart() {
        let directory = std::env::temp_dir().join(format!(
            "lumadeck-image-sync-recovery-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let state =
            DatabaseState::open(DataDirectoryResolver::for_app_data(&directory)).expect("database");
        let repository = SettingsRepository::new(&state);
        repository
            .update_steam_id("76561198012345678", "recovery-test")
            .expect("account");
        repository
            .begin_steam_image_sync(3, "1700000000")
            .expect("begin image sync");
        assert_eq!(
            repository
                .get_steam_image_sync_status()
                .expect("running status")
                .status,
            "running"
        );

        repository
            .recover_interrupted_syncs()
            .expect("recover interrupted sync");
        let recovered = repository
            .get_steam_image_sync_status()
            .expect("recovered status");
        assert_eq!(recovered.status, "error");
        assert_eq!(
            recovered.error_message.as_deref(),
            Some("STEAM_IMAGE_SYNC_INTERRUPTED_ON_START")
        );

        drop(state);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn steam_sync_is_idempotent_and_keeps_one_game_per_app_id() {
        let directory = std::env::temp_dir().join(format!(
            "lumadeck-steam-sync-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let state =
            DatabaseState::open(DataDirectoryResolver::for_app_data(&directory)).expect("database");
        let repository = SettingsRepository::new(&state);
        repository
            .update_steam_id("76561198012345678", "sync-test")
            .expect("account");
        repository
            .begin_steam_sync(0, "1700000000")
            .expect("begin sync");
        repository
            .update_steam_sync_progress(223, 223, None)
            .expect("update sync progress");
        let sync_status = repository.get_steam_sync_status().expect("sync status");
        assert_eq!(sync_status.found_count, 223);
        assert_eq!(sync_status.progress_completed, 223);
        assert_eq!(sync_status.progress_total, 223);
        let mut details = SteamGameDetails::default();
        details.complete = true;
        details.app_id = 730;
        details.tags.push("Action".to_string());
        let game = SteamLibraryGame {
            app_id: 730,
            name: "Counter-Strike 2".to_string(),
            total_playtime_minutes: 120,
            playtime_2weeks_minutes: Some(30),
            last_played_at: Some("1700000000".to_string()),
            installed: Some(true),
            has_community_visible_stats: Some(true),
            icon_url: None,
            logo_url: None,
            should_persist: true,
            details: Some(details),
        };
        let first = repository
            .sync_steam_games(std::slice::from_ref(&game), false, 10, "1700000010")
            .expect("first sync");
        let mut cached_game = game.clone();
        cached_game.should_persist = false;
        let second = repository
            .sync_steam_games(std::slice::from_ref(&cached_game), true, 10, "1700000020")
            .expect("second sync");
        repository
            .set_steam_library_sync_scope("installed")
            .expect("installed library scope");
        let visible_games = repository.get_local_games().expect("visible games");
        assert_eq!(visible_games.len(), 1);
        assert!(visible_games[0].installed);
        assert_eq!(first.created_count, 1);
        assert_eq!(second.created_count, 0);
        let connection = state.connection.lock().expect("database lock");
        let game_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM games", [], |row| row.get(0))
            .expect("games");
        let link_count: i64 = connection.query_row("SELECT COUNT(*) FROM game_provider_links WHERE provider_id = 'steam' AND external_id = '730'", [], |row| row.get(0)).expect("link");
        let tag_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM steam_game_tags WHERE value = 'Action'",
                [],
                |row| row.get(0),
            )
            .expect("tag");
        assert_eq!(game_count, 1);
        assert_eq!(link_count, 1);
        assert_eq!(tag_count, 1);
        drop(connection);
        drop(state);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn steam_cache_retries_games_without_description() {
        let directory = std::env::temp_dir().join(format!(
            "lumadeck-steam-cache-description-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let state =
            DatabaseState::open(DataDirectoryResolver::for_app_data(&directory)).expect("database");
        let repository = SettingsRepository::new(&state);
        let game = SteamLibraryGame {
            app_id: 3768760,
            name: "007 First Light".to_string(),
            total_playtime_minutes: 0,
            playtime_2weeks_minutes: None,
            last_played_at: None,
            installed: Some(true),
            has_community_visible_stats: Some(true),
            icon_url: None,
            logo_url: None,
            should_persist: true,
            details: Some(SteamGameDetails {
                app_id: 3768760,
                complete: false,
                ..SteamGameDetails::default()
            }),
        };

        repository
            .sync_steam_games(std::slice::from_ref(&game), false, 10, "1700000010")
            .expect("save incomplete Steam details");
        let cache = repository.get_steam_cache().expect("Steam cache");
        assert_eq!(
            cache.get(&3768760).map(|(_, updated_at)| *updated_at),
            Some(0)
        );

        drop(state);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn local_session_is_reflected_in_steam_metrics() {
        let directory = std::env::temp_dir().join(format!(
            "lumadeck-local-session-metrics-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let state =
            DatabaseState::open(DataDirectoryResolver::for_app_data(&directory)).expect("database");
        let repository = SettingsRepository::new(&state);
        {
            let connection = state.connection.lock().expect("database lock");
            connection
                .execute(
                    "INSERT INTO games(id, title, sort_title, provider, platform, created_at, updated_at)
                     VALUES (?1, ?2, ?2, 'steam', 'PC', ?3, ?3)",
                    params!["local-session-game", "Local Session Game", "1700000000"],
                )
                .expect("game");
            connection
                .execute(
                    "INSERT INTO game_details(game_id, steam_app_id, steam_updated_at)
                     VALUES (?1, ?2, ?3)",
                    params!["local-session-game", 3768760_i64, "1700000000"],
                )
                .expect("details");
        }

        let session_id = repository
            .start_game_session("local-session-game")
            .expect("start session");
        let started_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_secs()
            .saturating_sub(1_200)
            .to_string();
        {
            let connection = state.connection.lock().expect("database lock");
            connection
                .execute(
                    "UPDATE game_sessions SET started_at = ?1 WHERE id = ?2",
                    params![started_at, session_id],
                )
                .expect("rewind session for deterministic duration");
        }
        repository
            .end_game_session("local-session-game", session_id, false)
            .expect("end session");

        let metrics = repository
            .get_steam_game_metrics("local-session-game")
            .expect("metrics");
        assert_eq!(metrics.total_playtime_minutes, 20);
        assert!(metrics.last_played_at.is_some());

        let games = repository.get_local_games().expect("local games");
        let game = games
            .iter()
            .find(|game| game.id == "local-session-game")
            .expect("local session game");
        assert_eq!(game.playtime_minutes, 20);
        assert_eq!(game.last_played_at, metrics.last_played_at);

        drop(state);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn stale_eden_session_recovery_is_idempotent_and_does_not_add_time() {
        let directory = std::env::temp_dir().join(format!(
            "lumadeck-eden-session-recovery-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let state =
            DatabaseState::open(DataDirectoryResolver::for_app_data(&directory)).expect("database");
        let repository = SettingsRepository::new(&state);
        {
            let connection = state.connection.lock().expect("database lock");
            connection
                .execute(
                    "INSERT INTO games(id, title, sort_title, provider, platform, source, emulator_id, playtime_minutes, created_at, updated_at)
                     VALUES (?1, ?2, ?2, 'Eden', 'Nintendo Switch', 'emulator', 'eden', 120, ?3, ?3)",
                    params!["eden-recovery-game", "Recovery Game", "1700000000"],
                )
                .expect("game");
        }
        let first_session = repository
            .start_game_session("eden-recovery-game")
            .expect("start session");
        let second_session = repository
            .start_game_session("eden-recovery-game")
            .expect("deduplicated session");
        assert_eq!(first_session, second_session);
        assert_eq!(
            repository.recover_stale_game_sessions().expect("recovery"),
            1
        );
        assert_eq!(
            repository
                .recover_stale_game_sessions()
                .expect("idempotent recovery"),
            0
        );
        let game = repository
            .get_local_games()
            .expect("local games")
            .into_iter()
            .find(|game| game.id == "eden-recovery-game")
            .expect("recovered game");
        assert_eq!(game.playtime_minutes, 120);
        let connection = state.connection.lock().expect("database lock");
        let status: String = connection
            .query_row(
                "SELECT status FROM game_sessions WHERE id = ?1",
                params![first_session],
                |row| row.get(0),
            )
            .expect("session status");
        assert_eq!(status, "interrupted");
        drop(connection);
        drop(state);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn eden_external_playtime_is_canonical_and_does_not_double_count_sessions() {
        let directory = std::env::temp_dir().join(format!(
            "lumadeck-eden-playtime-canonical-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let state =
            DatabaseState::open(DataDirectoryResolver::for_app_data(&directory)).expect("database");
        let repository = SettingsRepository::new(&state);
        {
            let connection = state.connection.lock().expect("database lock");
            connection
                .execute(
                    "INSERT INTO games(id, title, sort_title, provider, platform, source, emulator_id, title_id, playtime_minutes, created_at, updated_at)
                     VALUES (?1, ?2, ?2, 'Eden', 'Nintendo Switch', 'emulator', 'eden', ?3, 300, ?4, ?4)",
                    params![
                        "eden-canonical-game",
                        "Canonical Game",
                        "0100000000010000",
                        "1700000000"
                    ],
                )
                .expect("game");
            connection
                .execute(
                    "INSERT INTO external_playtime_snapshots(provider, emulator_installation_id, title_id, game_id, total_seconds, observed_at, format)
                     VALUES ('eden', 'eden-installation-test', ?1, ?2, 19800, '1700000000', 'playtime.bin:v1:le:u64,u64')",
                    params!["0100000000010000", "eden-canonical-game"],
                )
                .expect("snapshot");
        }
        let session_id = repository
            .start_game_session("eden-canonical-game")
            .expect("start session");
        let started_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_secs()
            .saturating_sub(1_800)
            .to_string();
        {
            let connection = state.connection.lock().expect("database lock");
            connection
                .execute(
                    "UPDATE game_sessions SET started_at = ?1 WHERE id = ?2",
                    params![started_at, session_id],
                )
                .expect("rewind session");
        }
        repository
            .end_game_session("eden-canonical-game", session_id, false)
            .expect("end session");
        let game = repository
            .get_local_games()
            .expect("local games")
            .into_iter()
            .find(|game| game.id == "eden-canonical-game")
            .expect("canonical game");
        assert_eq!(game.playtime_minutes, 330);
        drop(state);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn game_favorite_toggle_persists_in_local_games() {
        let directory = std::env::temp_dir().join(format!(
            "lumadeck-favorite-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let state =
            DatabaseState::open(DataDirectoryResolver::for_app_data(&directory)).expect("database");
        let repository = SettingsRepository::new(&state);
        {
            let connection = state.connection.lock().expect("database lock");
            connection
                .execute(
                    "INSERT INTO games(id, title, sort_title, provider, platform, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                    rusqlite::params![
                        "favorite-test",
                        "Favorite Test",
                        "favorite test",
                        "Steam",
                        "PC",
                        "1700000000"
                    ],
                )
                .expect("game");
        }

        assert!(matches!(
            repository.set_game_favorite("favorite-test", true),
            Ok(true)
        ));
        assert!(
            repository
                .get_local_games()
                .expect("local games")
                .into_iter()
                .find(|game| game.id == "favorite-test")
                .expect("favorite game")
                .favorite
        );
        assert!(matches!(
            repository.set_game_favorite("favorite-test", false),
            Ok(false)
        ));
        assert!(matches!(
            repository.set_game_favorite("missing-game", true),
            Err(DatabaseError::GameNotFound)
        ));

        drop(state);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn game_visibility_toggle_preserves_favorite_and_activity_data() {
        let directory = std::env::temp_dir().join(format!(
            "lumadeck-hidden-game-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let state =
            DatabaseState::open(DataDirectoryResolver::for_app_data(&directory)).expect("database");
        let repository = SettingsRepository::new(&state);
        {
            let connection = state.connection.lock().expect("database lock");
            connection
                .execute(
                    "INSERT INTO games(id, title, sort_title, provider, platform, favorite, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, 1, ?6, ?6)",
                    rusqlite::params![
                        "hidden-test",
                        "Hidden Test",
                        "hidden test",
                        "Steam",
                        "PC",
                        "1700000000"
                    ],
                )
                .expect("game");
            connection
                .execute(
                    "INSERT INTO game_sessions(game_id, source, started_at, ended_at, duration_seconds, status, created_at, updated_at)
                     VALUES (?1, 'lumadeck', '1700000000', '1700000600', 600, 'completed', '1700000000', '1700000600')",
                    rusqlite::params!["hidden-test"],
                )
                .expect("activity");
        }

        assert!(matches!(
            repository.set_game_hidden("hidden-test", true),
            Ok(true)
        ));
        let hidden = repository
            .get_local_games()
            .expect("local games")
            .into_iter()
            .find(|game| game.id == "hidden-test")
            .expect("hidden game");
        assert!(hidden.hidden);
        assert!(hidden.favorite);
        assert_eq!(hidden.playtime_minutes, 10);

        assert!(matches!(
            repository.set_game_hidden("hidden-test", false),
            Ok(false)
        ));
        let restored = repository
            .get_local_games()
            .expect("local games")
            .into_iter()
            .find(|game| game.id == "hidden-test")
            .expect("restored game");
        assert!(!restored.hidden);
        assert!(restored.favorite);
        assert_eq!(restored.playtime_minutes, 10);

        drop(state);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn steam_details_upsert_persists_requested_metadata_columns() {
        let directory = std::env::temp_dir().join(format!(
            "lumadeck-steam-details-columns-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let state =
            DatabaseState::open(DataDirectoryResolver::for_app_data(&directory)).expect("database");
        let repository = SettingsRepository::new(&state);
        let game = SteamLibraryGame {
            app_id: 3768760,
            name: "007 First Light".to_string(),
            total_playtime_minutes: 308,
            playtime_2weeks_minutes: Some(12),
            last_played_at: Some("1785649000".to_string()),
            installed: Some(true),
            has_community_visible_stats: Some(true),
            icon_url: None,
            logo_url: None,
            should_persist: true,
            details: Some(SteamGameDetails {
                complete: true,
                app_id: 3768760,
                controller_support: Some("full".to_string()),
                release_date: Some("May 26, 2026".to_string()),
                description: Some("<p>Full description</p>".to_string()),
                short_description: Some("Short description".to_string()),
                minimum_requirements: Some(json!({"minimum": "requirements"})),
                recommended_requirements: Some(json!({"recommended": "requirements"})),
                review_score: Some(90),
                review_count: Some(16699),
                review_score_description: Some("Very Positive".to_string()),
                price: Some(json!({"currency": "USD", "final": 6999})),
                stats: vec![SteamStat {
                    name: "accuracy".to_string(),
                    value: json!(98),
                }],
                multiplayer: Some(false),
                single_player: Some(true),
                cloud: Some(true),
                categories: vec![
                    SteamNamedValue {
                        id: Some(1),
                        value: "Steam Achievements".to_string(),
                    },
                    SteamNamedValue {
                        id: Some(1),
                        value: "Steam Achievements".to_string(),
                    },
                ],
                ..SteamGameDetails::default()
            }),
        };

        repository
            .sync_steam_games(std::slice::from_ref(&game), false, 10, "1700000010")
            .expect("save complete Steam details");
        let connection = state.connection.lock().expect("database lock");
        let row: (
            Option<i64>,
            Option<i64>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<i64>,
            Option<i64>,
            Option<i64>,
        ) = connection
            .query_row(
                "SELECT steam_review_score, steam_review_count,
                    steam_review_score_description, steam_controller_support,
                    steam_release_date, steam_description, steam_short_description,
                    steam_min_requirements_json, steam_recommended_requirements_json,
                    steam_price_json, steam_stats_json, steam_multiplayer,
                    steam_single_player, steam_cloud
                 FROM game_details WHERE steam_app_id = 3768760",
                [],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                        row.get(6)?,
                        row.get(7)?,
                        row.get(8)?,
                        row.get(9)?,
                        row.get(10)?,
                        row.get(11)?,
                        row.get(12)?,
                        row.get(13)?,
                    ))
                },
            )
            .expect("metadata columns");

        assert_eq!(row.0, Some(90));
        assert_eq!(row.1, Some(16699));
        assert_eq!(row.2.as_deref(), Some("Very Positive"));
        assert_eq!(row.3.as_deref(), Some("full"));
        assert_eq!(row.4.as_deref(), Some("May 26, 2026"));
        assert_eq!(row.5.as_deref(), Some("<p>Full description</p>"));
        assert_eq!(row.6.as_deref(), Some("Short description"));
        assert!(row
            .7
            .as_deref()
            .is_some_and(|value| value.contains("requirements")));
        assert!(row
            .8
            .as_deref()
            .is_some_and(|value| value.contains("requirements")));
        assert!(row.9.as_deref().is_some_and(|value| value.contains("6999")));
        assert!(row
            .10
            .as_deref()
            .is_some_and(|value| value.contains("accuracy")));
        assert_eq!(row.11, Some(0));
        assert_eq!(row.12, Some(1));
        assert_eq!(row.13, Some(1));

        let category_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM steam_game_categories WHERE game_id = (SELECT game_id FROM game_details WHERE steam_app_id = 3768760)",
                [],
                |row| row.get(0),
            )
            .expect("deduplicated category");
        assert_eq!(category_count, 1);

        drop(connection);
        drop(state);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn steamgriddb_credential_lifecycle_is_masked_and_provider_remains() {
        let directory = std::env::temp_dir().join(format!(
            "lumadeck-steamgriddb-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let state =
            DatabaseState::open(DataDirectoryResolver::for_app_data(&directory)).expect("database");
        let repository = SettingsRepository::new(&state);

        let initial = repository
            .get_steamgriddb_configuration()
            .expect("initial configuration");
        assert_eq!(initial.status, "not-configured");
        assert!(!initial.api_key_configured);

        let configured = repository
            .save_steamgriddb_api_key("ABCDEFGHIJKLMNOP")
            .expect("save API key");
        assert_eq!(configured.status, "configured");
        assert_eq!(
            configured.api_key_masked.as_deref(),
            Some("••••••••••••MNOP")
        );
        assert!(configured.credential_available);

        let connection = state.connection.lock().expect("database lock");
        let encrypted: Vec<u8> = connection
            .query_row(
                "SELECT encrypted_value FROM provider_credentials
                 WHERE provider_account_id = 'steamgriddb-default'",
                [],
                |row| row.get(0),
            )
            .expect("encrypted credential");
        assert!(!String::from_utf8_lossy(&encrypted).contains("ABCDEFGHIJKLMNOP"));
        drop(connection);

        let deleted = repository
            .delete_steamgriddb_api_key()
            .expect("delete API key");
        assert_eq!(deleted.status, "not-configured");
        let connection = state.connection.lock().expect("database lock");
        let provider_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM providers WHERE id = 'steamgriddb'",
                [],
                |row| row.get(0),
            )
            .expect("provider count");
        assert_eq!(provider_count, 1);
        drop(connection);
        drop(state);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn display_profile_and_pending_restore_are_upserted_and_recoverable() {
        let directory = std::env::temp_dir().join(format!(
            "lumadeck-display-profile-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let state =
            DatabaseState::open(DataDirectoryResolver::for_app_data(&directory)).expect("database");
        {
            let connection = state.connection.lock().expect("database lock");
            connection
                .execute(
                    "INSERT INTO games(id, title, sort_title, provider, platform, created_at, updated_at)
                     VALUES ('display-game', 'Display Game', 'Display Game', 'Steam', 'Windows', '0', '0')",
                    [],
                )
                .expect("game");
        }
        let repository = SettingsRepository::new(&state);
        let profile = repository
            .save_display_profile(&DisplayProfile {
                game_id: "display-game".to_string(),
                enabled: true,
                display_id: Some(r"\\.\DISPLAY1".to_string()),
                device_name: Some("Test monitor".to_string()),
                width: Some(2560),
                height: Some(1440),
                refresh_rate: Some(60),
                restore_on_exit: true,
                updated_at: None,
                resolution_mode: DisplayResolutionMode::Custom,
                refresh_rate_mode: DisplayRefreshRateMode::Custom,
                hdr_mode: DisplayHdrMode::System,
                rtx_hdr_preset: None,
                rtx_hdr_peak_nits: crate::rtx_hdr::RTX_HDR_PEAK_NITS_DEFAULT,
            })
            .expect("profile");
        assert!(profile.enabled);
        assert_eq!(profile.width, Some(2560));
        let profile_count: i64 = state
            .connection
            .lock()
            .expect("database lock")
            .query_row(
                "SELECT COUNT(*) FROM game_display_profiles WHERE game_id = 'display-game'",
                [],
                |row| row.get(0),
            )
            .expect("profile count");
        assert_eq!(profile_count, 1);

        repository
            .save_pending_display_restore(&PendingDisplayRestore {
                display_id: r"\\.\DISPLAY1".to_string(),
                width: 3840,
                height: 2160,
                refresh_rate: 60,
                created_at: "1".to_string(),
            })
            .expect("pending restore");
        assert_eq!(
            repository
                .get_pending_display_restore()
                .expect("pending read")
                .expect("pending")
                .width,
            3840
        );
        repository
            .clear_pending_display_restore()
            .expect("pending clear");
        assert!(repository
            .get_pending_display_restore()
            .expect("pending read")
            .is_none());
        repository
            .save_pending_display_profile_restore(&PendingDisplayProfileRestore {
                session_id: "session-1".to_string(),
                game_id: "display-game".to_string(),
                snapshot: DisplayProfileSnapshot {
                    display_id: r"\\.\DISPLAY1".to_string(),
                    width: 1920,
                    height: 1080,
                    refresh_rate: 60,
                    hdr_enabled: false,
                    captured_at: "2".to_string(),
                },
                changed_resolution: true,
                changed_refresh_rate: false,
                changed_hdr: true,
                rtx_hdr_snapshot: None,
                auto_hdr_snapshot: None,
                rtx_hdr_executable: None,
                changed_rtx_hdr: false,
                changed_auto_hdr: false,
            })
            .expect("profile pending restore");
        let profile_pending = repository
            .get_pending_display_profile_restore()
            .expect("profile pending read")
            .expect("profile pending exists");
        assert_eq!(profile_pending.session_id, "session-1");
        assert!(profile_pending.changed_resolution);
        assert!(profile_pending.changed_hdr);
        repository
            .clear_pending_display_profile_restore()
            .expect("profile pending clear");
        assert!(repository
            .get_pending_display_profile_restore()
            .expect("profile pending empty")
            .is_none());
        drop(state);
        let _ = fs::remove_dir_all(directory);
    }

    #[test]
    fn review_consensus_is_upserted_and_recoverable_without_duplicates() {
        let directory = std::env::temp_dir().join(format!(
            "lumadeck-review-consensus-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let state =
            DatabaseState::open(DataDirectoryResolver::for_app_data(&directory)).expect("database");
        {
            let connection = state.connection.lock().expect("database lock");
            connection
                .execute(
                    "INSERT INTO games(id, title, sort_title, provider, platform, created_at, updated_at)
                     VALUES ('consensus-game', 'Consensus Game', 'Consensus Game', 'Steam', 'Windows', '0', '0')",
                    [],
                )
                .expect("game");
        }

        let repository = SettingsRepository::new(&state);
        let consensus = |conclusion: &str, fingerprint: &str| GameReviewConsensus {
            game_id: "consensus-game".to_string(),
            overall_rating: Some(4.0),
            agreement: "high".to_string(),
            agreement_label: "Alto acuerdo".to_string(),
            strengths: vec!["Combate".to_string()],
            weaknesses: vec!["Ritmo".to_string()],
            conclusion: conclusion.to_string(),
            sources: ConsensusSources {
                metacritic_included: true,
                opencritic_included: true,
                steam_included: true,
                critic_review_count: Some(20),
                player_review_count: Some(100),
                sampled_steam_reviews: 6,
            },
            generated_at: "2026-01-01T00:00:00Z".to_string(),
            prompt_version: 1,
            provider_id: "openrouter".to_string(),
            model_id: Some("google/gemini-2.5-flash".to_string()),
            input_fingerprint: fingerprint.to_string(),
        };

        repository
            .save_game_review_consensus(&consensus("Primera conclusión", "fingerprint-a"))
            .expect("save consensus");
        assert_eq!(
            repository
                .get_game_review_consensus("consensus-game")
                .expect("read consensus")
                .expect("stored consensus")
                .conclusion,
            "Primera conclusión"
        );

        repository
            .save_game_review_consensus(&consensus("Conclusión actualizada", "fingerprint-b"))
            .expect("replace consensus");
        let stored = repository
            .get_game_review_consensus("consensus-game")
            .expect("read replacement")
            .expect("replacement");
        assert_eq!(stored.conclusion, "Conclusión actualizada");
        assert_eq!(stored.input_fingerprint, "fingerprint-b");
        let count: i64 = state
            .connection
            .lock()
            .expect("database lock")
            .query_row(
                "SELECT COUNT(*) FROM game_review_consensus WHERE game_id = 'consensus-game'",
                [],
                |row| row.get(0),
            )
            .expect("consensus count");
        assert_eq!(count, 1);

        drop(state);
        let _ = fs::remove_dir_all(directory);
    }
}
