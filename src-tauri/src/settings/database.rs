use super::models::StorageMigrationStatus;
use crate::data_directory::DataDirectoryResolver;
use crate::steamgriddb::SteamGridDbQueryCache;
use rusqlite::{Connection, OpenFlags};
use std::{
    error::Error as StdError,
    fs::{self, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize},
        Arc, Mutex,
    },
};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DatabaseError {
    #[error("database path could not be created")]
    Path(#[source] std::io::Error),
    #[error("database operation failed")]
    Sqlite(#[from] rusqlite::Error),
    #[error("credential operation failed")]
    Credential(#[from] super::crypto::CredentialError),
    #[error("invalid SteamID64")]
    InvalidSteamId,
    #[error("invalid Steam Web API key")]
    InvalidApiKey,
    #[error("invalid Google Cloud Translation API key")]
    InvalidTranslationApiKey,
    #[error("invalid Steam library synchronization scope")]
    InvalidSteamSyncScope,
    #[error("Steam account is not configured")]
    AccountNotConfigured,
    #[error("game was not found in the Steam library")]
    GameNotFound,
    #[error("Steam metadata is unavailable for this game")]
    SteamMetadataUnavailable,
    #[error("unsupported provider operation")]
    UnsupportedProvider,
    #[error("invalid AI provider")]
    InvalidAIProvider,
    #[error("invalid AI model")]
    InvalidAIModel,
    #[error("invalid AI API key")]
    InvalidAIApiKey,
    #[error("stored review consensus is invalid")]
    ConsensusDataInvalid,
}

impl DatabaseError {
    pub(crate) fn variant_name(&self) -> &'static str {
        match self {
            Self::Path(_) => "Path",
            Self::Sqlite(_) => "Sqlite",
            Self::Credential(_) => "Credential",
            Self::InvalidSteamId => "InvalidSteamId",
            Self::InvalidApiKey => "InvalidApiKey",
            Self::InvalidTranslationApiKey => "InvalidTranslationApiKey",
            Self::InvalidSteamSyncScope => "InvalidSteamSyncScope",
            Self::AccountNotConfigured => "AccountNotConfigured",
            Self::GameNotFound => "GameNotFound",
            Self::SteamMetadataUnavailable => "SteamMetadataUnavailable",
            Self::UnsupportedProvider => "UnsupportedProvider",
            Self::InvalidAIProvider => "InvalidAIProvider",
            Self::InvalidAIModel => "InvalidAIModel",
            Self::InvalidAIApiKey => "InvalidAIApiKey",
            Self::ConsensusDataInvalid => "ConsensusDataInvalid",
        }
    }

    pub(crate) fn source_chain(&self) -> String {
        let mut chain = Vec::new();
        let mut current: Option<&(dyn StdError + 'static)> = Some(self);
        while let Some(error) = current {
            chain.push(error.to_string());
            current = error.source();
        }
        chain.join(" -> ")
    }

    pub(crate) fn sqlite_diagnostic(&self) -> Option<String> {
        let Self::Sqlite(error) = self else {
            return None;
        };
        match error {
            rusqlite::Error::SqliteFailure(code, message) => Some(format!(
                "code={:?}; extended_code={}; message={}",
                code.code,
                code.extended_code,
                message.as_deref().unwrap_or("<none>")
            )),
            other => Some(format!("variant={other:?}")),
        }
    }
}

pub struct DatabaseState {
    pub(crate) connection: Mutex<Connection>,
    pub(crate) path: PathBuf,
    pub(crate) data_directory: DataDirectoryResolver,
    pub(crate) steam_sync_running: Arc<AtomicBool>,
    pub(crate) steam_sync_cancel_requested: Arc<AtomicBool>,
    pub(crate) steam_sync_progress: Arc<AtomicUsize>,
    pub(crate) steam_sync_total: Arc<AtomicUsize>,
    pub(crate) steam_metadata_sync_running: Arc<AtomicBool>,
    pub(crate) steam_image_sync_running: Arc<AtomicBool>,
    pub(crate) steam_image_sync_cancel_requested: Arc<AtomicBool>,
    pub(crate) steam_image_sync_progress: Arc<AtomicUsize>,
    pub(crate) steam_image_sync_total: Arc<AtomicUsize>,
    pub(crate) steam_achievement_sync_running: Arc<AtomicBool>,
    pub(crate) steam_achievement_sync_cancel_requested: Arc<AtomicBool>,
    pub(crate) hltb_sync_running: Arc<AtomicBool>,
    pub(crate) hltb_sync_cancel_requested: Arc<AtomicBool>,
    pub(crate) hltb_sync_progress: Arc<AtomicUsize>,
    pub(crate) hltb_sync_total: Arc<AtomicUsize>,
    pub(crate) storage_migration_running: Arc<AtomicBool>,
    pub(crate) storage_migration_status: Arc<Mutex<StorageMigrationStatus>>,
    pub(crate) steamgriddb_query_cache: Arc<Mutex<SteamGridDbQueryCache>>,
    pub(crate) steamgriddb_search_generation: Arc<AtomicU64>,
}

impl DatabaseState {
    pub fn open(data_directory: DataDirectoryResolver) -> Result<Self, DatabaseError> {
        match Self::open_at(data_directory.clone()) {
            Ok(state) => {
                let _ = state.data_directory.confirm_pending_migration();
                Ok(state)
            }
            Err(primary_error) => {
                let Some(recovery_directory) = data_directory.recovery_resolver() else {
                    return Err(primary_error);
                };
                Self::open_at(recovery_directory)
            }
        }
    }

    fn open_at(data_directory: DataDirectoryResolver) -> Result<Self, DatabaseError> {
        data_directory.ensure_root().map_err(DatabaseError::Path)?;
        let path = data_directory.database_path();
        let connection = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
        )?;
        connection.pragma_update(None, "foreign_keys", true)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        if data_directory.read_pending_migration().is_some() {
            connection.query_row(
                "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations' LIMIT 1",
                [],
                |row| row.get::<_, i64>(0),
            )?;
        }
        run_migrations(&connection)?;
        let mode_name = data_directory.mode_name().to_string();
        let root_display = data_directory.root().display().to_string();
        let state = Self {
            connection: Mutex::new(connection),
            path,
            data_directory,
            steam_sync_running: Arc::new(AtomicBool::new(false)),
            steam_sync_cancel_requested: Arc::new(AtomicBool::new(false)),
            steam_sync_progress: Arc::new(AtomicUsize::new(0)),
            steam_sync_total: Arc::new(AtomicUsize::new(0)),
            steam_metadata_sync_running: Arc::new(AtomicBool::new(false)),
            steam_image_sync_running: Arc::new(AtomicBool::new(false)),
            steam_image_sync_cancel_requested: Arc::new(AtomicBool::new(false)),
            steam_image_sync_progress: Arc::new(AtomicUsize::new(0)),
            steam_image_sync_total: Arc::new(AtomicUsize::new(0)),
            steam_achievement_sync_running: Arc::new(AtomicBool::new(false)),
            steam_achievement_sync_cancel_requested: Arc::new(AtomicBool::new(false)),
            hltb_sync_running: Arc::new(AtomicBool::new(false)),
            hltb_sync_cancel_requested: Arc::new(AtomicBool::new(false)),
            hltb_sync_progress: Arc::new(AtomicUsize::new(0)),
            hltb_sync_total: Arc::new(AtomicUsize::new(0)),
            storage_migration_running: Arc::new(AtomicBool::new(false)),
            storage_migration_status: Arc::new(Mutex::new(StorageMigrationStatus::idle(
                mode_name,
                root_display,
            ))),
            steamgriddb_query_cache: Arc::new(Mutex::new(SteamGridDbQueryCache::default())),
            steamgriddb_search_generation: Arc::new(AtomicU64::new(0)),
        };
        Ok(state)
    }

    pub(crate) fn log(&self, correlation_id: &str, checkpoint: &str, details: &str) {
        write_diagnostic_line(
            self.data_directory.logs_directory().as_path(),
            correlation_id,
            checkpoint,
            details,
        );
    }

    pub(crate) fn log_runtime_context(&self, correlation_id: &str) {
        let result = (|| -> Result<String, rusqlite::Error> {
            let connection = self
                .connection
                .lock()
                .map_err(|_| rusqlite::Error::InvalidQuery)?;
            let foreign_keys: i64 =
                connection.pragma_query_value(None, "foreign_keys", |row| row.get(0))?;
            let journal_mode: String =
                connection.pragma_query_value(None, "journal_mode", |row| row.get(0))?;
            let busy_timeout: i64 =
                connection.pragma_query_value(None, "busy_timeout", |row| row.get(0))?;
            let migration_version: i64 = connection.query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )?;
            let provider_exists: bool = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM providers WHERE id = 'steam')",
                [],
                |row| row.get(0),
            )?;
            Ok(format!(
                    "path={}; pid={}; build={}; foreign_keys={}; journal_mode={}; busy_timeout_ms={}; migration_version={}; provider_steam_exists={}",
                    self.path.display(),
                    std::process::id(),
                    option_env!("GIT_COMMIT").unwrap_or("dev-unavailable"),
                    foreign_keys,
                    journal_mode,
                    busy_timeout,
                    migration_version,
                    provider_exists
                ))
        })();
        match result {
            Ok(details) => self.log(correlation_id, "DATABASE_RUNTIME_CONTEXT", &details),
            Err(error) => self.log(
                correlation_id,
                "DATABASE_RUNTIME_CONTEXT_FAILED",
                &format!("error={error}; debug={error:?}"),
            ),
        }
    }
}

fn write_diagnostic_line(
    logs_directory: &std::path::Path,
    correlation_id: &str,
    checkpoint: &str,
    details: &str,
) {
    let line =
        format!("[settings] correlationId={correlation_id} checkpoint={checkpoint} {details}\n");
    #[cfg(debug_assertions)]
    eprint!("{line}");
    if fs::create_dir_all(logs_directory).is_err() {
        return;
    }
    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(logs_directory.join("settings-runtime.log"))
    {
        let _ = file.write_all(line.as_bytes());
    }
}

fn run_migrations(connection: &Connection) -> Result<(), DatabaseError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        );",
    )?;
    let applied: i64 = connection.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )?;
    if applied < 1 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS providers (
                id TEXT PRIMARY KEY,
                display_name TEXT NOT NULL,
                enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS provider_accounts (
                id TEXT PRIMARY KEY,
                provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
                external_account_id TEXT,
                display_name TEXT,
                enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
                configuration_status TEXT NOT NULL,
                last_sync_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS provider_credentials (
                id TEXT PRIMARY KEY,
                provider_account_id TEXT NOT NULL REFERENCES provider_accounts(id) ON DELETE CASCADE,
                credential_type TEXT NOT NULL,
                encrypted_value BLOB NOT NULL,
                protection_method TEXT NOT NULL,
                masked_suffix TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(provider_account_id, credential_type)
            );
            CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY,
                value_json TEXT NOT NULL,
                schema_version INTEGER NOT NULL,
                updated_at TEXT NOT NULL
            );
            INSERT OR IGNORE INTO providers(id, display_name, enabled, created_at, updated_at)
                VALUES ('steam', 'Steam', 1, datetime('now'), datetime('now'));
            INSERT INTO schema_migrations(version, applied_at) VALUES (1, datetime('now'));",
        )?;
        transaction.commit()?;
    }
    if applied < 2 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS games (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                sort_title TEXT NOT NULL,
                provider TEXT NOT NULL,
                platform TEXT NOT NULL,
                favorite INTEGER NOT NULL DEFAULT 0 CHECK (favorite IN (0, 1)),
                installed INTEGER NOT NULL DEFAULT 0 CHECK (installed IN (0, 1)),
                progress REAL NOT NULL DEFAULT 0,
                status TEXT NOT NULL DEFAULT 'not-started',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS game_provider_links (
                game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
                provider_id TEXT NOT NULL REFERENCES providers(id) ON DELETE CASCADE,
                external_id TEXT NOT NULL,
                is_owned INTEGER NOT NULL DEFAULT 1 CHECK (is_owned IN (0, 1)),
                last_synced_at TEXT,
                PRIMARY KEY (provider_id, external_id),
                UNIQUE (game_id, provider_id)
            );
            CREATE TABLE IF NOT EXISTS game_details (
                game_id TEXT PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE,
                steam_app_id INTEGER UNIQUE,
                steam_name TEXT,
                steam_total_playtime_minutes INTEGER,
                steam_playtime_2weeks_minutes INTEGER,
                steam_last_played_at TEXT,
                steam_installed INTEGER CHECK (steam_installed IN (0, 1) OR steam_installed IS NULL),
                steam_owned INTEGER CHECK (steam_owned IN (0, 1) OR steam_owned IS NULL),
                steam_hidden INTEGER CHECK (steam_hidden IN (0, 1) OR steam_hidden IS NULL),
                steam_acquired_at TEXT,
                steam_achievement_total INTEGER,
                steam_achievement_unlocked INTEGER,
                steam_achievement_progress REAL,
                steam_review_score INTEGER,
                steam_review_count INTEGER,
                steam_review_score_description TEXT,
                steam_controller_support TEXT,
                steam_release_date TEXT,
                steam_description TEXT,
                steam_short_description TEXT,
                steam_website TEXT,
                steam_min_requirements_json TEXT,
                steam_recommended_requirements_json TEXT,
                steam_header_url TEXT,
                steam_background_url TEXT,
                steam_price_json TEXT,
                steam_stats_json TEXT,
                steam_early_access INTEGER CHECK (steam_early_access IN (0, 1) OR steam_early_access IS NULL),
                steam_adult_content INTEGER CHECK (steam_adult_content IN (0, 1) OR steam_adult_content IS NULL),
                steam_multiplayer INTEGER CHECK (steam_multiplayer IN (0, 1) OR steam_multiplayer IS NULL),
                steam_single_player INTEGER CHECK (steam_single_player IN (0, 1) OR steam_single_player IS NULL),
                steam_cloud INTEGER CHECK (steam_cloud IN (0, 1) OR steam_cloud IS NULL),
                steam_trading_cards INTEGER CHECK (steam_trading_cards IN (0, 1) OR steam_trading_cards IS NULL),
                steam_workshop INTEGER CHECK (steam_workshop IN (0, 1) OR steam_workshop IS NULL),
                steam_family_sharing INTEGER CHECK (steam_family_sharing IN (0, 1) OR steam_family_sharing IS NULL),
                steam_has_community_visible_stats INTEGER CHECK (steam_has_community_visible_stats IN (0, 1) OR steam_has_community_visible_stats IS NULL),
                steam_icon_url TEXT,
                steam_logo_url TEXT,
                steam_updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS steam_game_tags (
                game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
                value TEXT NOT NULL,
                PRIMARY KEY (game_id, value)
            );
            CREATE TABLE IF NOT EXISTS steam_game_genres (
                game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
                value TEXT NOT NULL,
                PRIMARY KEY (game_id, value)
            );
            CREATE TABLE IF NOT EXISTS steam_game_categories (
                game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
                category_id INTEGER,
                value TEXT NOT NULL,
                PRIMARY KEY (game_id, value)
            );
            CREATE TABLE IF NOT EXISTS steam_game_developers (
                game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
                value TEXT NOT NULL,
                PRIMARY KEY (game_id, value)
            );
            CREATE TABLE IF NOT EXISTS steam_game_publishers (
                game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
                value TEXT NOT NULL,
                PRIMARY KEY (game_id, value)
            );
            CREATE TABLE IF NOT EXISTS steam_game_languages (
                game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
                value TEXT NOT NULL,
                PRIMARY KEY (game_id, value)
            );
            CREATE TABLE IF NOT EXISTS steam_game_platforms (
                game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
                value TEXT NOT NULL,
                PRIMARY KEY (game_id, value)
            );
            CREATE TABLE IF NOT EXISTS steam_game_achievements (
                game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
                api_name TEXT NOT NULL,
                display_name TEXT,
                description TEXT,
                achieved INTEGER NOT NULL DEFAULT 0 CHECK (achieved IN (0, 1)),
                unlock_time TEXT,
                PRIMARY KEY (game_id, api_name)
            );
            CREATE TABLE IF NOT EXISTS steam_game_stats (
                game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
                name TEXT NOT NULL,
                value_json TEXT NOT NULL,
                PRIMARY KEY (game_id, name)
            );
            CREATE TABLE IF NOT EXISTS steam_game_media (
                game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
                media_type TEXT NOT NULL,
                external_id TEXT NOT NULL,
                name TEXT,
                thumbnail_url TEXT,
                full_url TEXT,
                metadata_json TEXT,
                PRIMARY KEY (game_id, media_type, external_id)
            );
            CREATE TABLE IF NOT EXISTS steam_game_dlc (
                game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
                dlc_app_id INTEGER NOT NULL,
                PRIMARY KEY (game_id, dlc_app_id)
            );
            CREATE TABLE IF NOT EXISTS steam_sync_state (
                account_id TEXT PRIMARY KEY REFERENCES provider_accounts(id) ON DELETE CASCADE,
                status TEXT NOT NULL,
                found_count INTEGER NOT NULL DEFAULT 0,
                created_count INTEGER NOT NULL DEFAULT 0,
                updated_count INTEGER NOT NULL DEFAULT 0,
                progress_completed INTEGER NOT NULL DEFAULT 0,
                progress_total INTEGER NOT NULL DEFAULT 0,
                duration_ms INTEGER,
                started_at TEXT,
                completed_at TEXT,
                current_app_id INTEGER,
                error_message TEXT
            );
            INSERT INTO schema_migrations(version, applied_at) VALUES (2, datetime('now'));",
        )?;
        transaction.commit()?;
    }
    if applied < 3 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS steam_game_assets (
                game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
                asset_type TEXT NOT NULL,
                external_id TEXT NOT NULL,
                source_url TEXT NOT NULL,
                local_path TEXT,
                mime_type TEXT,
                width INTEGER,
                height INTEGER,
                byte_size INTEGER,
                downloaded_at TEXT,
                updated_at TEXT NOT NULL,
                PRIMARY KEY (game_id, asset_type, external_id)
            );
            CREATE INDEX IF NOT EXISTS idx_steam_game_assets_game_type
                ON steam_game_assets(game_id, asset_type);
            CREATE TABLE IF NOT EXISTS steam_image_sync_state (
                account_id TEXT PRIMARY KEY REFERENCES provider_accounts(id) ON DELETE CASCADE,
                status TEXT NOT NULL,
                found_count INTEGER NOT NULL DEFAULT 0,
                downloaded_count INTEGER NOT NULL DEFAULT 0,
                skipped_count INTEGER NOT NULL DEFAULT 0,
                progress_completed INTEGER NOT NULL DEFAULT 0,
                progress_total INTEGER NOT NULL DEFAULT 0,
                duration_ms INTEGER,
                started_at TEXT,
                completed_at TEXT,
                current_app_id INTEGER,
                error_message TEXT
            );
            INSERT INTO schema_migrations(version, applied_at) VALUES (3, datetime('now'));",
        )?;
        transaction.commit()?;
    }
    if applied < 4 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "ALTER TABLE game_details ADD COLUMN steam_active_players INTEGER;
             ALTER TABLE game_details ADD COLUMN steam_metrics_updated_at TEXT;
             INSERT INTO schema_migrations(version, applied_at) VALUES (4, datetime('now'));",
        )?;
        transaction.commit()?;
    }
    if applied < 5 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS hltb_game_times (
                game_id TEXT PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE,
                hltb_id TEXT,
                matched_title TEXT,
                main_story_minutes INTEGER,
                main_extra_minutes INTEGER,
                completionist_minutes INTEGER,
                match_confidence REAL,
                match_type TEXT,
                last_synced_at TEXT,
                source TEXT NOT NULL,
                status TEXT NOT NULL,
                last_error TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_hltb_game_times_status ON hltb_game_times(status);
            CREATE TABLE IF NOT EXISTS hltb_sync_state (
                id TEXT PRIMARY KEY,
                status TEXT NOT NULL,
                processed_count INTEGER NOT NULL DEFAULT 0,
                total_count INTEGER NOT NULL DEFAULT 0,
                found_count INTEGER NOT NULL DEFAULT 0,
                unmatched_count INTEGER NOT NULL DEFAULT 0,
                exact_match_count INTEGER NOT NULL DEFAULT 0,
                approximate_match_count INTEGER NOT NULL DEFAULT 0,
                error_count INTEGER NOT NULL DEFAULT 0,
                duration_ms INTEGER,
                started_at TEXT,
                completed_at TEXT,
                last_error TEXT
            );
            INSERT OR IGNORE INTO hltb_sync_state(id, status) VALUES ('hltb-default', 'idle');
            INSERT INTO app_settings(key, value_json, schema_version, updated_at)
                VALUES ('hltb.integration', '{\"enabled\":true,\"syncWithSteam\":true,\"showMainStory\":true,\"showMainExtra\":true,\"showCompletionist\":true}', 1, datetime('now'))
                ON CONFLICT(key) DO NOTHING;
            INSERT OR IGNORE INTO providers(id, display_name, enabled, created_at, updated_at)
                VALUES ('hltb', 'HowLongToBeat', 1, datetime('now'), datetime('now'));
            INSERT INTO schema_migrations(version, applied_at) VALUES (5, datetime('now'));",
        )?;
        transaction.commit()?;
    }
    if applied < 6 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS hltb_match_overrides (
                game_id TEXT PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE,
                alias_query TEXT,
                hltb_id TEXT,
                matched_title TEXT,
                main_story_minutes INTEGER,
                main_extra_minutes INTEGER,
                completionist_minutes INTEGER,
                resolution_status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_hltb_match_overrides_status
                ON hltb_match_overrides(resolution_status);
            INSERT INTO schema_migrations(version, applied_at) VALUES (6, datetime('now'));",
        )?;
        transaction.commit()?;
    }
    if applied < 7 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "INSERT OR IGNORE INTO providers(id, display_name, enabled, created_at, updated_at)
                VALUES ('steamgriddb', 'SteamGridDB', 1, datetime('now'), datetime('now'));
            INSERT OR IGNORE INTO provider_accounts(
                id, provider_id, external_account_id, display_name, enabled,
                configuration_status, created_at, updated_at
            ) VALUES (
                'steamgriddb-default', 'steamgriddb', NULL, 'SteamGridDB', 1,
                'not-configured', datetime('now'), datetime('now')
            );
            INSERT INTO schema_migrations(version, applied_at) VALUES (7, datetime('now'));",
        )?;
        transaction.commit()?;
    }
    if applied < 8 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS artwork_assets (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                source TEXT NOT NULL,
                external_asset_id INTEGER NOT NULL,
                external_game_id INTEGER,
                kind TEXT NOT NULL,
                grid_style TEXT,
                width INTEGER NOT NULL,
                height INTEGER NOT NULL,
                source_mime_type TEXT NOT NULL,
                cached_mime_type TEXT NOT NULL,
                cache_key TEXT NOT NULL UNIQUE,
                cached_path TEXT NOT NULL,
                checksum TEXT NOT NULL,
                byte_size INTEGER NOT NULL,
                downloaded_at TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_artwork_assets_checksum
                ON artwork_assets(checksum);
            CREATE TABLE IF NOT EXISTS game_artwork_selections (
                game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
                slot TEXT NOT NULL,
                artwork_asset_id INTEGER NOT NULL REFERENCES artwork_assets(id),
                selection_source TEXT NOT NULL,
                selected_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                PRIMARY KEY (game_id, slot)
            );
            CREATE INDEX IF NOT EXISTS idx_game_artwork_selections_asset
                ON game_artwork_selections(artwork_asset_id);
            INSERT INTO schema_migrations(version, applied_at) VALUES (8, datetime('now'));",
        )?;
        transaction.commit()?;
    }
    if applied < 9 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS game_sessions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
                started_at TEXT NOT NULL,
                ended_at TEXT,
                duration_seconds INTEGER,
                status TEXT NOT NULL CHECK (status IN ('active', 'completed', 'interrupted')),
                source TEXT NOT NULL DEFAULT 'lumadeck',
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_game_sessions_game_started
                ON game_sessions(game_id, started_at DESC);
            CREATE TABLE IF NOT EXISTS game_activity_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
                event_type TEXT NOT NULL,
                occurred_at TEXT NOT NULL,
                title TEXT NOT NULL,
                description TEXT,
                value_json TEXT,
                source TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_game_activity_events_game_occurred
                ON game_activity_events(game_id, occurred_at DESC);
            INSERT INTO schema_migrations(version, applied_at) VALUES (9, datetime('now'));",
        )?;
        transaction.commit()?;
    }
    if applied < 10 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS game_display_profiles (
                game_id TEXT PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE,
                enabled INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0, 1)),
                display_id TEXT,
                device_name TEXT,
                width INTEGER CHECK (width IS NULL OR width > 0),
                height INTEGER CHECK (height IS NULL OR height > 0),
                refresh_rate INTEGER CHECK (refresh_rate IS NULL OR refresh_rate > 0),
                restore_on_exit INTEGER NOT NULL DEFAULT 1 CHECK (restore_on_exit IN (0, 1)),
                updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS pending_display_restore (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                display_id TEXT NOT NULL,
                width INTEGER NOT NULL CHECK (width > 0),
                height INTEGER NOT NULL CHECK (height > 0),
                refresh_rate INTEGER NOT NULL CHECK (refresh_rate > 0),
                created_at TEXT NOT NULL
            );
            INSERT INTO schema_migrations(version, applied_at) VALUES (10, datetime('now'));",
        )?;
        transaction.commit()?;
    }
    if applied < 11 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS game_frame_generation_profiles (
                game_id TEXT PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE,
                provider TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0, 1)),
                mode TEXT NOT NULL DEFAULT 'FIXED',
                multiplier INTEGER NOT NULL DEFAULT 2 CHECK (multiplier IN (2, 3, 4)),
                auto_scale INTEGER NOT NULL DEFAULT 1 CHECK (auto_scale IN (0, 1)),
                auto_scale_delay INTEGER NOT NULL DEFAULT 0 CHECK (auto_scale_delay >= 0),
                target_executable TEXT,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_frame_generation_profiles_target
                ON game_frame_generation_profiles(target_executable);
            INSERT INTO schema_migrations(version, applied_at) VALUES (11, datetime('now'));",
        )?;
        transaction.commit()?;
    }
    if applied < 12 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "ALTER TABLE steam_game_achievements ADD COLUMN hidden INTEGER NOT NULL DEFAULT 0 CHECK (hidden IN (0, 1));
             ALTER TABLE steam_game_achievements ADD COLUMN unlock_percentage REAL;
             ALTER TABLE steam_game_achievements ADD COLUMN rarity TEXT NOT NULL DEFAULT 'common';
             ALTER TABLE steam_game_achievements ADD COLUMN virtual_tier TEXT NOT NULL DEFAULT 'bronze';
             ALTER TABLE steam_game_achievements ADD COLUMN icon_unlocked TEXT;
             ALTER TABLE steam_game_achievements ADD COLUMN icon_locked TEXT;
             ALTER TABLE steam_game_achievements ADD COLUMN local_icon_unlocked TEXT;
             ALTER TABLE steam_game_achievements ADD COLUMN local_icon_locked TEXT;
             ALTER TABLE steam_game_achievements ADD COLUMN updated_at TEXT NOT NULL DEFAULT '';
             CREATE INDEX IF NOT EXISTS idx_steam_game_achievements_unlocked
                 ON steam_game_achievements(game_id, achieved, unlock_time);
             CREATE TABLE IF NOT EXISTS steam_achievement_sync_state (
                 game_id TEXT PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE,
                 steam_app_id INTEGER NOT NULL,
                 status TEXT NOT NULL DEFAULT 'idle',
                 schema_version INTEGER NOT NULL DEFAULT 1,
                 source_hash TEXT,
                 last_synced_at TEXT,
                 last_attempted_at TEXT,
                 error_message TEXT
             );
             CREATE INDEX IF NOT EXISTS idx_steam_achievement_sync_app
                 ON steam_achievement_sync_state(steam_app_id);
             INSERT INTO schema_migrations(version, applied_at) VALUES (12, datetime('now'));",
        )?;
        transaction.commit()?;
    }
    if applied < 13 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS news_items (
                id TEXT PRIMARY KEY,
                provider_id TEXT NOT NULL,
                external_id TEXT NOT NULL,
                game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
                external_game_id TEXT,
                category TEXT NOT NULL CHECK (category IN (
                    'official', 'update', 'event', 'community', 'media',
                    'dlc', 'maintenance', 'other'
                )),
                source_url TEXT NOT NULL,
                canonical_url TEXT,
                published_at TEXT NOT NULL,
                updated_at TEXT,
                first_seen_at TEXT NOT NULL,
                source_language TEXT NOT NULL,
                original_title TEXT NOT NULL,
                original_summary TEXT,
                original_content TEXT,
                content_format TEXT NOT NULL CHECK (content_format IN (
                    'plain_text', 'html', 'markdown', 'unknown'
                )),
                source_content_hash TEXT NOT NULL,
                provider_metadata TEXT,
                created_at TEXT NOT NULL,
                persisted_updated_at TEXT NOT NULL,
                UNIQUE(provider_id, external_id)
            );
            CREATE INDEX IF NOT EXISTS idx_news_items_game_published
                ON news_items(game_id, published_at DESC);
            CREATE INDEX IF NOT EXISTS idx_news_items_category
                ON news_items(category);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_news_items_provider_canonical
                ON news_items(provider_id, canonical_url)
                WHERE canonical_url IS NOT NULL AND canonical_url <> '';

            CREATE TABLE IF NOT EXISTS news_translations (
                id TEXT PRIMARY KEY,
                news_item_id TEXT NOT NULL REFERENCES news_items(id) ON DELETE CASCADE,
                source_language TEXT NOT NULL,
                target_language TEXT NOT NULL,
                translated_title TEXT,
                translated_summary TEXT,
                translated_content TEXT,
                status TEXT NOT NULL CHECK (status IN (
                    'pending', 'translating', 'translated', 'failed', 'stale'
                )),
                provider_id TEXT NOT NULL,
                provider_version TEXT NOT NULL DEFAULT '',
                glossary_version TEXT NOT NULL DEFAULT '',
                source_content_hash TEXT NOT NULL,
                translated_content_hash TEXT,
                translated_at TEXT,
                last_attempt_at TEXT,
                error_code TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(
                    news_item_id, source_language, target_language, provider_id,
                    provider_version, glossary_version, source_content_hash
                )
            );
            CREATE INDEX IF NOT EXISTS idx_news_translations_news_target
                ON news_translations(news_item_id, target_language);
            CREATE INDEX IF NOT EXISTS idx_news_translations_reusable
                ON news_translations(news_item_id, target_language, source_content_hash, status);

            CREATE TABLE IF NOT EXISTS news_sync_state (
                provider_id TEXT NOT NULL,
                game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
                last_successful_sync_at TEXT,
                last_attempt_at TEXT,
                last_error_code TEXT,
                cursor TEXT,
                is_stale INTEGER NOT NULL DEFAULT 0 CHECK (is_stale IN (0, 1)),
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                PRIMARY KEY(provider_id, game_id)
            );
            CREATE INDEX IF NOT EXISTS idx_news_sync_state_provider_game
                ON news_sync_state(provider_id, game_id);
            INSERT INTO schema_migrations(version, applied_at) VALUES (13, datetime('now'));",
        )?;
        transaction.commit()?;
    }
    if applied < 14 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "INSERT OR IGNORE INTO providers(id, display_name, enabled, created_at, updated_at)
                VALUES ('google-cloud-translation', 'Google Cloud Translation', 1, datetime('now'), datetime('now'));
             INSERT OR IGNORE INTO provider_accounts(
                id, provider_id, external_account_id, display_name, enabled,
                configuration_status, created_at, updated_at
             ) VALUES (
                'google-cloud-translation-default', 'google-cloud-translation', NULL,
                'Google Cloud Translation', 1, 'not-configured', datetime('now'), datetime('now')
             );
             INSERT INTO schema_migrations(version, applied_at) VALUES (14, datetime('now'));",
        )?;
        transaction.commit()?;
    }
    if applied < 15 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "INSERT OR IGNORE INTO providers(id, display_name, enabled, created_at, updated_at)
                VALUES ('rapidapi-reviews', 'OpenCritic / Metacritic', 1, datetime('now'), datetime('now'));
             INSERT OR IGNORE INTO provider_accounts(
                id, provider_id, external_account_id, display_name, enabled,
                configuration_status, created_at, updated_at
             ) VALUES (
                'rapidapi-reviews-default', 'rapidapi-reviews', NULL,
                'OpenCritic / Metacritic', 1, 'not-configured', datetime('now'), datetime('now')
             );
             INSERT INTO schema_migrations(version, applied_at) VALUES (15, datetime('now'));",
        )?;
        transaction.commit()?;
    }
    if applied < 16 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS game_reviews_cache (
                game_id TEXT PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE,
                steam_app_id INTEGER NOT NULL,
                metacritic_json TEXT,
                metacritic_updated_at TEXT,
                opencritic_json TEXT,
                opencritic_updated_at TEXT,
                steam_json TEXT,
                steam_updated_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            INSERT INTO schema_migrations(version, applied_at) VALUES (16, datetime('now'));",
        )?;
        transaction.commit()?;
    }
    if applied < 17 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY,
                value_json TEXT NOT NULL,
                schema_version INTEGER NOT NULL,
                updated_at TEXT NOT NULL
             );
             INSERT OR IGNORE INTO providers(id, display_name, enabled, created_at, updated_at)
                VALUES ('openrouter', 'OpenRouter', 1, datetime('now'), datetime('now'));
             INSERT OR IGNORE INTO provider_accounts(
                id, provider_id, external_account_id, display_name, enabled,
                configuration_status, created_at, updated_at
             ) VALUES (
                'openrouter-default', 'openrouter', NULL, 'OpenRouter', 1,
                'not-configured', datetime('now'), datetime('now')
             );
             INSERT OR IGNORE INTO app_settings(key, value_json, schema_version, updated_at)
                VALUES ('ai.configuration', '{\"providerId\":\"openrouter\",\"model\":\"google/gemini-2.5-flash\"}', 1, datetime('now'));
             INSERT INTO schema_migrations(version, applied_at) VALUES (17, datetime('now'));",
        )?;
        transaction.commit()?;
    }
    if applied < 18 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute_batch(
            "CREATE TABLE IF NOT EXISTS game_review_consensus (
                game_id TEXT PRIMARY KEY REFERENCES games(id) ON DELETE CASCADE,
                consensus_json TEXT NOT NULL,
                generated_at TEXT NOT NULL,
                prompt_version INTEGER NOT NULL,
                provider_id TEXT NOT NULL,
                model_id TEXT,
                input_fingerprint TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_game_review_consensus_fingerprint
                ON game_review_consensus(input_fingerprint);
            INSERT INTO schema_migrations(version, applied_at) VALUES (18, datetime('now'));",
        )?;
        transaction.commit()?;
    }
    if applied < 19 {
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "UPDATE app_settings
             SET value_json = REPLACE(value_json, 'openrouter/auto', 'google/gemini-2.5-flash'),
                 updated_at = datetime('now')
             WHERE key = 'ai.configuration'
               AND value_json LIKE '%openrouter/auto%'",
            [],
        )?;
        transaction.execute(
            "INSERT INTO schema_migrations(version, applied_at) VALUES (19, datetime('now'))",
            [],
        )?;
        transaction.commit()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::run_migrations;
    use rusqlite::Connection;
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn migration_is_idempotent_and_enables_foreign_keys() {
        let connection = Connection::open_in_memory().expect("in-memory database");
        connection
            .pragma_update(None, "foreign_keys", true)
            .expect("foreign keys");
        run_migrations(&connection).expect("first migration");
        run_migrations(&connection).expect("second migration");
        let migration_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("migration count");
        let provider_count: i64 = connection
            .query_row("SELECT COUNT(*) FROM providers", [], |row| row.get(0))
            .expect("provider count");
        assert_eq!(migration_count, 19);
        assert_eq!(provider_count, 6);
        let table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'game_details'",
                [],
                |row| row.get(0),
            )
            .expect("game details table");
        assert_eq!(table_count, 1);
        let asset_table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'steam_game_assets'",
                [],
                |row| row.get(0),
            )
            .expect("steam assets table");
        assert_eq!(asset_table_count, 1);
        let activity_table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'game_sessions'",
                [],
                |row| row.get(0),
            )
            .expect("game sessions table");
        assert_eq!(activity_table_count, 1);
        let profile_table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'game_display_profiles'",
                [],
                |row| row.get(0),
            )
            .expect("display profile table");
        assert_eq!(profile_table_count, 1);
        let frame_generation_table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'game_frame_generation_profiles'",
                [],
                |row| row.get(0),
            )
            .expect("frame generation profile table");
        assert_eq!(frame_generation_table_count, 1);
        for table in ["news_items", "news_translations", "news_sync_state"] {
            let count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .expect("news table");
            assert_eq!(count, 1, "missing news table {table}");
        }
        let reviews_cache_table_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'game_reviews_cache'",
                [],
                |row| row.get(0),
            )
            .expect("reviews cache table");
        assert_eq!(reviews_cache_table_count, 1);
    }

    #[test]
    fn migrates_production_11_achievements_to_12_and_preserves_rows() {
        let path = std::env::temp_dir().join(format!(
            "lumadeck-achievements-migration-{}.db",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let connection = Connection::open(&path).expect("production database");
        connection
            .execute_batch(
                "CREATE TABLE schema_migrations(version INTEGER PRIMARY KEY, applied_at TEXT NOT NULL);
                 CREATE TABLE providers(id TEXT PRIMARY KEY, display_name TEXT NOT NULL, enabled INTEGER NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
                 CREATE TABLE provider_accounts(id TEXT PRIMARY KEY, provider_id TEXT NOT NULL, external_account_id TEXT, display_name TEXT, enabled INTEGER NOT NULL, configuration_status TEXT NOT NULL, last_sync_at TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
                 CREATE TABLE games(id TEXT PRIMARY KEY);
                 CREATE TABLE steam_game_achievements(
                    game_id TEXT NOT NULL REFERENCES games(id) ON DELETE CASCADE,
                    api_name TEXT NOT NULL,
                    display_name TEXT,
                    description TEXT,
                    achieved INTEGER NOT NULL DEFAULT 0 CHECK (achieved IN (0, 1)),
                    unlock_time TEXT,
                    PRIMARY KEY(game_id, api_name)
                 );
                 INSERT INTO schema_migrations(version, applied_at) VALUES
                    (1, '2026-01-01'), (2, '2026-01-01'), (3, '2026-01-01'),
                    (4, '2026-01-01'), (5, '2026-01-01'), (6, '2026-01-01'),
                    (7, '2026-01-01'), (8, '2026-01-01'), (9, '2026-01-01'),
                    (10, '2026-01-01'), (11, '2026-01-01');
                 INSERT INTO games(id) VALUES ('game-001');
                 INSERT INTO steam_game_achievements(game_id, api_name, display_name, description, achieved, unlock_time)
                    VALUES ('game-001', 'ACH_LEGACY', 'Legacy', 'Preserve me', 1, '1700000000');",
            )
            .expect("version 11 fixture");
        run_migrations(&connection).expect("migration 12");
        let preserved: (String, i64, String) = connection
            .query_row(
                "SELECT display_name, achieved, unlock_time FROM steam_game_achievements WHERE api_name = 'ACH_LEGACY'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("preserved achievement");
        assert_eq!(
            preserved,
            ("Legacy".to_string(), 1, "1700000000".to_string())
        );
        let defaults: (i64, Option<f64>, String, String, String) = connection
            .query_row(
                "SELECT hidden, unlock_percentage, rarity, virtual_tier, updated_at FROM steam_game_achievements WHERE api_name = 'ACH_LEGACY'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .expect("migration defaults");
        assert_eq!(
            defaults,
            (
                0,
                None,
                "common".to_string(),
                "bronze".to_string(),
                String::new()
            )
        );
        let index_exists: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = 'idx_steam_game_achievements_unlocked'",
                [],
                |row| row.get(0),
            )
            .expect("achievement index");
        assert_eq!(index_exists, 1);
        let sync_table_exists: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'steam_achievement_sync_state'",
                [],
                |row| row.get(0),
            )
            .expect("sync state table");
        assert_eq!(sync_table_exists, 1);
        drop(connection);

        let reopened = Connection::open(&path).expect("reopen migrated database");
        run_migrations(&reopened).expect("second migration");
        let migration_version: i64 = reopened
            .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("migration version");
        assert_eq!(migration_version, 19);
        drop(reopened);
        let _ = fs::remove_file(path);
    }
}
