use super::{database::DatabaseError, repositories::resolve_local_asset_path, DatabaseState};
use crate::{artwork::PreparedArtwork, steamgriddb::ArtworkSlot};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone)]
pub(crate) struct ArtworkEnrichmentGame {
    pub id: String,
    pub title: String,
    pub platform: String,
    pub source: String,
    pub provider: String,
    pub title_id: Option<String>,
    pub steam_app_id: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ArtworkSlotState {
    Missing,
    Existing,
    UserLocked,
    Manual,
    Automatic,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtworkApplyResult {
    pub game_id: String,
    pub slot: ArtworkSlot,
    pub cached_path: String,
    pub cache_key: String,
    pub checksum: String,
    pub width: u32,
    pub height: u32,
    pub cached_mime_type: String,
    pub file_reused: bool,
}

pub struct ArtworkRepository<'a> {
    state: &'a DatabaseState,
}

impl<'a> ArtworkRepository<'a> {
    pub fn new(state: &'a DatabaseState) -> Self {
        Self { state }
    }

    pub fn persist_selection(
        &self,
        artwork: &PreparedArtwork,
    ) -> Result<ArtworkApplyResult, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.unchecked_transaction()?;
        let game_exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM games WHERE id = ?1)",
            params![artwork.game_id],
            |row| row.get(0),
        )?;
        if !game_exists {
            return Err(DatabaseError::GameNotFound);
        }
        let asset_id = transaction
            .query_row(
                "SELECT id FROM artwork_assets WHERE cache_key = ?1",
                params![artwork.cache_key],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        let asset_id = match asset_id {
            Some(id) => id,
            None => {
                let now = timestamp();
                transaction.execute(
                    "INSERT INTO artwork_assets(
                        source, external_asset_id, external_game_id, kind, grid_style,
                        width, height, source_mime_type, cached_mime_type, cache_key,
                        cached_path, checksum, byte_size, downloaded_at, created_at, updated_at,
                        source_url_hash, cached_width, cached_height, selected_automatically, user_locked
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21)",
                    params![
                        "steamgriddb",
                        artwork.external_asset_id,
                        artwork.external_game_id,
                        kind_name(artwork.kind),
                        artwork.grid_style,
                        i64::from(artwork.width),
                        i64::from(artwork.height),
                        artwork.source_mime_type,
                        artwork.cached_mime_type,
                        artwork.cache_key,
                        artwork.cached_path,
                        artwork.checksum,
                        i64::try_from(artwork.byte_size).unwrap_or(i64::MAX),
                        now,
                        now,
                        now,
                        artwork.source_url_hash,
                        i64::from(artwork.cached_width),
                        i64::from(artwork.cached_height),
                        0,
                        1,
                    ],
                )?;
                transaction.last_insert_rowid()
            }
        };
        let now = timestamp();
        transaction.execute(
            "INSERT INTO game_artwork_selections(
                game_id, slot, artwork_asset_id, selection_source, provenance, user_locked,
                selected_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(game_id, slot) DO UPDATE SET
                artwork_asset_id = excluded.artwork_asset_id,
                selection_source = excluded.selection_source,
                provenance = excluded.provenance,
                user_locked = excluded.user_locked,
                selected_at = excluded.selected_at,
                updated_at = excluded.updated_at",
            params![
                artwork.game_id,
                slot_name(artwork.slot),
                asset_id,
                "steamgriddb_manual",
                "steamgriddb_manual",
                1,
                now,
                now,
            ],
        )?;
        transaction.commit()?;
        Ok(ArtworkApplyResult {
            game_id: artwork.game_id.clone(),
            slot: artwork.slot,
            cached_path: artwork.cached_path.clone(),
            cache_key: artwork.cache_key.clone(),
            checksum: artwork.checksum.clone(),
            width: artwork.width,
            height: artwork.height,
            cached_mime_type: artwork.cached_mime_type.clone(),
            file_reused: artwork.file_reused,
        })
    }

    pub(crate) fn get_enrichment_games(
        &self,
        only_non_steam: bool,
    ) -> Result<Vec<ArtworkEnrichmentGame>, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let mut statement = connection.prepare(
            "SELECT g.id, g.title, g.platform, COALESCE(g.source, ''), g.provider, g.title_id,
                    (SELECT external_id FROM game_provider_links
                     WHERE game_id = g.id AND provider_id = 'steam' LIMIT 1)
             FROM games g
             WHERE (?1 = 0 OR COALESCE(g.source, '') <> 'steam')
             ORDER BY g.sort_title",
        )?;
        let games = statement
            .query_map(params![if only_non_steam { 1 } else { 0 }], |row| {
                let steam_app_id = row
                    .get::<_, Option<String>>(6)?
                    .and_then(|value| value.parse::<i64>().ok());
                Ok(ArtworkEnrichmentGame {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    platform: row.get(2)?,
                    source: row.get(3)?,
                    provider: row.get(4)?,
                    title_id: row.get(5)?,
                    steam_app_id,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(games)
    }

    pub(crate) fn get_slot_state(
        &self,
        game_id: &str,
        slot: ArtworkSlot,
    ) -> Result<ArtworkSlotState, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let selection = connection
            .query_row(
                "SELECT selection_source, provenance, user_locked
                 FROM game_artwork_selections WHERE game_id = ?1 AND slot = ?2",
                params![game_id, slot_name(slot)],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?;
        if let Some((source, provenance, user_locked)) = selection {
            if user_locked != 0 || provenance == "user" {
                return Ok(ArtworkSlotState::UserLocked);
            }
            if source == "steamgriddb_manual" || provenance == "steamgriddb_manual" {
                return Ok(ArtworkSlotState::Manual);
            }
            if source == "steamgriddb_auto" || provenance == "steamgriddb_auto" {
                return Ok(ArtworkSlotState::Automatic);
            }
            return Ok(ArtworkSlotState::Existing);
        }
        Ok(if has_fallback_artwork(&connection, game_id, slot)? {
            ArtworkSlotState::Existing
        } else {
            ArtworkSlotState::Missing
        })
    }

    pub(crate) fn persist_automatic_selection(
        &self,
        artwork: &PreparedArtwork,
    ) -> Result<bool, DatabaseError> {
        let mut connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let transaction = connection.transaction()?;
        let game_exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM games WHERE id = ?1)",
            params![artwork.game_id],
            |row| row.get(0),
        )?;
        if !game_exists {
            return Err(DatabaseError::GameNotFound);
        }
        let already_selected: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM game_artwork_selections WHERE game_id = ?1 AND slot = ?2)",
            params![artwork.game_id, slot_name(artwork.slot)],
            |row| row.get(0),
        )?;
        if already_selected || has_fallback_artwork(&transaction, &artwork.game_id, artwork.slot)? {
            transaction.commit()?;
            return Ok(false);
        }
        let asset_id = insert_artwork_asset(&transaction, artwork, true, false)?;
        let now = timestamp();
        transaction.execute(
            "INSERT INTO game_artwork_selections(
                game_id, slot, artwork_asset_id, selection_source, provenance, user_locked,
                selected_at, updated_at
             ) VALUES (?1, ?2, ?3, 'steamgriddb_auto', 'steamgriddb_auto', 0, ?4, ?4)
             ON CONFLICT(game_id, slot) DO NOTHING",
            params![artwork.game_id, slot_name(artwork.slot), asset_id, now],
        )?;
        let inserted = transaction.changes() > 0;
        transaction.commit()?;
        Ok(inserted)
    }

    pub(crate) fn negative_cache_valid(
        &self,
        game_id: &str,
        slot: ArtworkSlot,
    ) -> Result<bool, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let expires_at: Option<i64> = connection
            .query_row(
                "SELECT expires_at FROM artwork_negative_cache
                 WHERE game_id = ?1 AND provider = 'steamgriddb' AND slot = ?2",
                params![game_id, slot_name(slot)],
                |row| row.get(0),
            )
            .optional()?;
        Ok(expires_at.is_some_and(|value| value > epoch_seconds()))
    }

    pub(crate) fn store_negative_cache(
        &self,
        game_id: &str,
        slot: ArtworkSlot,
        ttl_seconds: i64,
    ) -> Result<(), DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let now = epoch_seconds();
        connection.execute(
            "INSERT INTO artwork_negative_cache(game_id, provider, slot, status, timestamp, expires_at)
             VALUES (?1, 'steamgriddb', ?2, 'not_found', ?3, ?4)
             ON CONFLICT(game_id, provider, slot) DO UPDATE SET
                status = excluded.status, timestamp = excluded.timestamp, expires_at = excluded.expires_at",
            params![game_id, slot_name(slot), now, now + ttl_seconds],
        )?;
        Ok(())
    }

    pub(crate) fn save_enrichment_run(
        &self,
        summary_json: &str,
        status: &str,
    ) -> Result<(), DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        connection.execute(
            "INSERT INTO artwork_enrichment_runs(started_at, completed_at, status, summary_json)
             VALUES (?1, ?1, ?2, ?3)",
            params![timestamp(), status, summary_json],
        )?;
        Ok(())
    }

    pub(crate) fn get_last_enrichment_run(&self) -> Result<Option<String>, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        connection
            .query_row(
                "SELECT summary_json FROM artwork_enrichment_runs ORDER BY id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(DatabaseError::from)
    }

    pub fn clear_selection(&self, game_id: &str, slot: ArtworkSlot) -> Result<(), DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        connection.execute(
            "DELETE FROM game_artwork_selections WHERE game_id = ?1 AND slot = ?2",
            params![game_id, slot_name(slot)],
        )?;
        Ok(())
    }

    pub fn get_current_asset(
        &self,
        game_id: &str,
        slot: ArtworkSlot,
    ) -> Result<Option<String>, DatabaseError> {
        let connection = self
            .state
            .connection
            .lock()
            .map_err(|_| rusqlite::Error::InvalidQuery)?;
        let asset_type = steam_asset_type(slot);
        let (value, fallback): (String, String) = connection.query_row(
            "SELECT COALESCE(
                (SELECT a.cached_path
                 FROM game_artwork_selections s
                 JOIN artwork_assets a ON a.id = s.artwork_asset_id
                 WHERE s.game_id = ?1 AND s.slot = ?2),
                (SELECT local_path FROM steam_game_assets
                 WHERE game_id = ?1 AND asset_type = ?3
                 ORDER BY updated_at DESC LIMIT 1),
                (SELECT source_url FROM steam_game_assets
                 WHERE game_id = ?1 AND asset_type = ?3
                 ORDER BY updated_at DESC LIMIT 1),
                ''
            ),
            COALESCE(
                (SELECT source_url FROM steam_game_assets
                 WHERE game_id = ?1 AND asset_type = ?3
                 ORDER BY updated_at DESC LIMIT 1),
                ''
            )",
            rusqlite::params![game_id, slot_name(slot), asset_type],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let resolved = resolve_local_asset_path(self.state, &value, &fallback);
        Ok((!resolved.is_empty()).then_some(resolved))
    }
}

fn insert_artwork_asset(
    transaction: &rusqlite::Transaction<'_>,
    artwork: &PreparedArtwork,
    selected_automatically: bool,
    user_locked: bool,
) -> Result<i64, rusqlite::Error> {
    if let Some(id) = transaction
        .query_row(
            "SELECT id FROM artwork_assets WHERE cache_key = ?1",
            params![artwork.cache_key],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
    {
        return Ok(id);
    }
    let now = timestamp();
    transaction.execute(
        "INSERT INTO artwork_assets(
            source, external_asset_id, external_game_id, kind, grid_style,
            width, height, source_mime_type, cached_mime_type, cache_key,
            cached_path, checksum, byte_size, downloaded_at, created_at, updated_at,
            source_url_hash, cached_width, cached_height, selected_automatically, user_locked
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14, ?14, ?15, ?16, ?17, ?18, ?19)",
        params![
            "steamgriddb",
            artwork.external_asset_id,
            artwork.external_game_id,
            kind_name(artwork.kind),
            artwork.grid_style,
            i64::from(artwork.width),
            i64::from(artwork.height),
            artwork.source_mime_type,
            artwork.cached_mime_type,
            artwork.cache_key,
            artwork.cached_path,
            artwork.checksum,
            i64::try_from(artwork.byte_size).unwrap_or(i64::MAX),
            now,
            artwork.source_url_hash,
            i64::from(artwork.cached_width),
            i64::from(artwork.cached_height),
            i64::from(selected_automatically),
            i64::from(user_locked),
        ],
    )?;
    Ok(transaction.last_insert_rowid())
}

fn has_fallback_artwork(
    connection: &rusqlite::Connection,
    game_id: &str,
    slot: ArtworkSlot,
) -> Result<bool, rusqlite::Error> {
    let (asset_type, detail_column): (&str, &str) = match slot {
        ArtworkSlot::GridHorizontal => ("horizontal_cover", "steam_header_url"),
        ArtworkSlot::GridVertical => ("vertical_cover", ""),
        ArtworkSlot::GridSquare => ("icon", ""),
        ArtworkSlot::Hero => ("hero", "steam_background_url"),
        ArtworkSlot::Logo => ("logo", "steam_logo_url"),
        ArtworkSlot::Icon => ("icon", "steam_icon_url"),
    };
    if !detail_column.is_empty()
        && connection.query_row(
            &format!(
                "SELECT EXISTS(SELECT 1 FROM game_details WHERE game_id = ?1 AND COALESCE({detail_column}, '') <> '')"
            ),
            params![game_id],
            |row| row.get(0),
        )?
    {
        return Ok(true);
    }
    connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM steam_game_assets
         WHERE game_id = ?1 AND asset_type = ?2
           AND COALESCE(NULLIF(local_path, ''), NULLIF(source_url, '')) IS NOT NULL)",
        params![game_id, asset_type],
        |row| row.get(0),
    )
}

fn kind_name(kind: crate::steamgriddb::ArtworkKind) -> &'static str {
    match kind {
        crate::steamgriddb::ArtworkKind::Grid => "grid",
        crate::steamgriddb::ArtworkKind::Hero => "hero",
        crate::steamgriddb::ArtworkKind::Logo => "logo",
        crate::steamgriddb::ArtworkKind::Icon => "icon",
    }
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

fn steam_asset_type(slot: ArtworkSlot) -> &'static str {
    match slot {
        ArtworkSlot::GridHorizontal => "horizontal_cover",
        ArtworkSlot::GridVertical => "vertical_cover",
        ArtworkSlot::GridSquare | ArtworkSlot::Icon => "icon",
        ArtworkSlot::Hero => "hero",
        ArtworkSlot::Logo => "logo",
    }
}

fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

fn epoch_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| i64::try_from(duration.as_secs()).unwrap_or(i64::MAX))
        .unwrap_or_default()
}
