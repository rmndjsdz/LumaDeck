use super::{database::DatabaseError, repositories::resolve_local_asset_path, DatabaseState};
use crate::{artwork::PreparedArtwork, steamgriddb::ArtworkSlot};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

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
                        cached_path, checksum, byte_size, downloaded_at, created_at, updated_at
                    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)",
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
                    ],
                )?;
                transaction.last_insert_rowid()
            }
        };
        let now = timestamp();
        transaction.execute(
            "INSERT INTO game_artwork_selections(
                game_id, slot, artwork_asset_id, selection_source, selected_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(game_id, slot) DO UPDATE SET
                artwork_asset_id = excluded.artwork_asset_id,
                selection_source = excluded.selection_source,
                selected_at = excluded.selected_at,
                updated_at = excluded.updated_at",
            params![
                artwork.game_id,
                slot_name(artwork.slot),
                asset_id,
                "steamgriddb",
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
