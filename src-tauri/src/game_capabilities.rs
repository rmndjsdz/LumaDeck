use crate::{
    pcgamingwiki::{
        self, PcgamingwikiCapabilitiesRequest, PcgamingwikiConfidence, PcgamingwikiNormalizedValue,
        PcgamingwikiResolutionStatus,
    },
    settings::DatabaseState,
};
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GameCapabilityKind {
    NativeHdr,
    HighFidelityUpscaling,
    FrameGeneration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GameCapabilityValue {
    Yes,
    No,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GameCapabilityConfidence {
    High,
    Medium,
    Low,
    UserDefined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GameCapabilitySource {
    Pcgamingwiki,
    UserOverride,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GameCapabilityOverrideState {
    NoOverride,
    ForceYes,
    ForceNo,
    ForceUnknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameCapabilityEvidence {
    pub game_id: String,
    pub capability: GameCapabilityKind,
    pub value: GameCapabilityValue,
    pub source: GameCapabilitySource,
    pub source_value: Option<String>,
    pub alternative_available: GameCapabilityValue,
    pub source_note: Option<String>,
    pub confidence: GameCapabilityConfidence,
    pub technologies: Vec<String>,
    pub observed_at: String,
    pub source_reference: Option<String>,
    pub provider_version: Option<i64>,
    pub stale: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameCapabilityOverride {
    pub game_id: String,
    pub capability: GameCapabilityKind,
    pub state: GameCapabilityOverrideState,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedCapability {
    pub kind: GameCapabilityKind,
    pub value: GameCapabilityValue,
    pub confidence: GameCapabilityConfidence,
    pub source: GameCapabilitySource,
    pub technologies: Vec<String>,
    pub alternative_available: GameCapabilityValue,
    pub source_note: Option<String>,
    pub evidence: Option<GameCapabilityEvidence>,
    pub other_evidence: Vec<GameCapabilityEvidence>,
    pub resolved_at: i64,
    pub stale: bool,
    pub has_conflict: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedGameCapabilities {
    pub game_id: String,
    pub native_hdr: ResolvedCapability,
    pub high_fidelity_upscaling: ResolvedCapability,
    pub frame_generation: ResolvedCapability,
    pub resolved_at: i64,
    pub provider_status: Option<PcgamingwikiResolutionStatus>,
    pub provider_error: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameCapabilitiesRequest {
    pub game_id: String,
    pub steam_app_id: Option<i64>,
    pub gog_product_id: Option<String>,
}

#[tauri::command]
pub async fn get_game_capabilities(
    state: tauri::State<'_, DatabaseState>,
    game_id: String,
    steam_app_id: Option<i64>,
    gog_product_id: Option<String>,
) -> Result<ResolvedGameCapabilities, String> {
    resolve_with_provider(
        &state,
        GameCapabilitiesRequest {
            game_id,
            steam_app_id,
            gog_product_id,
        },
        false,
    )
    .await
}

#[tauri::command]
pub async fn refresh_game_capabilities(
    state: tauri::State<'_, DatabaseState>,
    game_id: String,
    steam_app_id: Option<i64>,
    gog_product_id: Option<String>,
) -> Result<ResolvedGameCapabilities, String> {
    resolve_with_provider(
        &state,
        GameCapabilitiesRequest {
            game_id,
            steam_app_id,
            gog_product_id,
        },
        true,
    )
    .await
}

#[tauri::command]
pub fn set_game_capability_override(
    state: tauri::State<'_, DatabaseState>,
    game_id: String,
    capability: GameCapabilityKind,
    override_state: GameCapabilityOverrideState,
) -> Result<ResolvedGameCapabilities, String> {
    if override_state == GameCapabilityOverrideState::NoOverride {
        return clear_override_internal(&state, &game_id, capability);
    }
    set_override_internal(&state, &game_id, capability, override_state)
}

fn set_override_internal(
    state: &DatabaseState,
    game_id: &str,
    capability: GameCapabilityKind,
    override_state: GameCapabilityOverrideState,
) -> Result<ResolvedGameCapabilities, String> {
    let now = now_seconds();
    let connection = state.connection.lock().map_err(|_| "database poisoned")?;
    connection
        .execute(
            "INSERT INTO game_capability_overrides(game_id, capability, override_state, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)
             ON CONFLICT(game_id, capability) DO UPDATE SET override_state=excluded.override_state, updated_at=excluded.updated_at",
            params![
                game_id,
                capability_name(capability),
                override_state_name(override_state),
                now
            ],
        )
        .map_err(|error| error.to_string())?;
    drop(connection);
    state.log(
        "game-capabilities",
        "capabilities.override.set",
        &format!(
            "game_id={game_id}; capability={}; state={}",
            capability_name(capability),
            override_state_name(override_state)
        ),
    );
    resolve_from_storage(state, game_id)
}

#[tauri::command]
pub fn clear_game_capability_override(
    state: tauri::State<'_, DatabaseState>,
    game_id: String,
    capability: GameCapabilityKind,
) -> Result<ResolvedGameCapabilities, String> {
    clear_override_internal(&state, &game_id, capability)
}

async fn resolve_with_provider(
    state: &DatabaseState,
    request: GameCapabilitiesRequest,
    force_refresh: bool,
) -> Result<ResolvedGameCapabilities, String> {
    if request.game_id.trim().is_empty() {
        return Err("GAME_CAPABILITIES_INVALID_GAME_ID".to_string());
    }
    state.log(
        "game-capabilities",
        if force_refresh {
            "capabilities.refresh.start"
        } else {
            "capabilities.resolve.start"
        },
        &format!("game_id={}", request.game_id),
    );
    let provider_response = pcgamingwiki::get_capabilities(
        state,
        PcgamingwikiCapabilitiesRequest {
            game_id: request.game_id.clone(),
            steam_app_id: request.steam_app_id,
            gog_product_id: request.gog_product_id,
            force_refresh,
            cross_check_identities: false,
        },
    )
    .await?;

    let mut evidence = provider_response
        .capabilities
        .as_ref()
        .map(|capabilities| evidence_from_provider(&request.game_id, capabilities))
        .unwrap_or_else(|| load_provider_evidence(state, &request.game_id).unwrap_or_default());
    if provider_response.stale {
        for item in &mut evidence {
            item.stale = true;
        }
    }
    let overrides = load_overrides(state, &request.game_id)?;
    let mut resolved = resolve_game_capabilities(&request.game_id, evidence, overrides);
    resolved.provider_status = Some(provider_response.status.clone());
    resolved.provider_error = provider_response.error.clone();
    let event = if force_refresh {
        "capabilities.refresh.complete"
    } else {
        "capabilities.resolve.result"
    };
    state.log(
        "game-capabilities",
        event,
        &format!(
            "game_id={}; hdr={:?}; upscaling={:?}; framegen={:?}",
            request.game_id,
            resolved.native_hdr.value,
            resolved.high_fidelity_upscaling.value,
            resolved.frame_generation.value
        ),
    );
    if resolved.native_hdr.has_conflict
        || resolved.high_fidelity_upscaling.has_conflict
        || resolved.frame_generation.has_conflict
    {
        state.log(
            "game-capabilities",
            "capabilities.conflict",
            &format!("game_id={}", request.game_id),
        );
    }
    Ok(resolved)
}

pub fn resolve_game_capabilities(
    game_id: &str,
    evidence: Vec<GameCapabilityEvidence>,
    overrides: Vec<GameCapabilityOverride>,
) -> ResolvedGameCapabilities {
    let override_map = overrides
        .into_iter()
        .map(|item| (item.capability, item))
        .collect::<HashMap<_, _>>();
    let resolved_at = now_seconds();
    ResolvedGameCapabilities {
        game_id: game_id.to_string(),
        native_hdr: resolve_one(
            game_id,
            GameCapabilityKind::NativeHdr,
            &evidence,
            override_map.get(&GameCapabilityKind::NativeHdr),
            resolved_at,
        ),
        high_fidelity_upscaling: resolve_one(
            game_id,
            GameCapabilityKind::HighFidelityUpscaling,
            &evidence,
            override_map.get(&GameCapabilityKind::HighFidelityUpscaling),
            resolved_at,
        ),
        frame_generation: resolve_one(
            game_id,
            GameCapabilityKind::FrameGeneration,
            &evidence,
            override_map.get(&GameCapabilityKind::FrameGeneration),
            resolved_at,
        ),
        resolved_at,
        provider_status: None,
        provider_error: None,
    }
}

fn resolve_one(
    game_id: &str,
    kind: GameCapabilityKind,
    evidence: &[GameCapabilityEvidence],
    override_item: Option<&GameCapabilityOverride>,
    resolved_at: i64,
) -> ResolvedCapability {
    let mut candidates = evidence
        .iter()
        .filter(|item| item.game_id == game_id && item.capability == kind)
        .cloned()
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        left.stale
            .cmp(&right.stale)
            .then_with(|| confidence_rank(right.confidence).cmp(&confidence_rank(left.confidence)))
    });
    let provider = candidates.first().cloned();
    let provider_values = candidates.iter().map(|item| item.value).collect::<Vec<_>>();
    let provider_conflict = provider_values
        .first()
        .is_some_and(|first| provider_values.iter().any(|value| value != first));
    let override_value = override_item.and_then(|item| override_value(item.state));
    let value = override_value
        .or_else(|| provider.as_ref().map(|item| item.value))
        .unwrap_or(GameCapabilityValue::Unknown);
    let source = if override_value.is_some() {
        GameCapabilitySource::UserOverride
    } else {
        provider
            .as_ref()
            .map(|_| GameCapabilitySource::Pcgamingwiki)
            .unwrap_or(GameCapabilitySource::None)
    };
    let technologies = if override_value == Some(GameCapabilityValue::Yes)
        && provider
            .as_ref()
            .is_some_and(|item| item.value == GameCapabilityValue::Yes)
    {
        provider
            .as_ref()
            .map(|item| item.technologies.clone())
            .unwrap_or_default()
    } else if override_value.is_none() {
        provider
            .as_ref()
            .map(|item| item.technologies.clone())
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let winning_evidence = override_value
        .map(|override_value| GameCapabilityEvidence {
            game_id: game_id.to_string(),
            capability: kind,
            value: override_value,
            source: GameCapabilitySource::UserOverride,
            source_value: Some(value_name(override_value).to_string()),
            alternative_available: provider
                .as_ref()
                .map(|item| item.alternative_available)
                .unwrap_or(GameCapabilityValue::Unknown),
            source_note: provider.as_ref().and_then(|item| item.source_note.clone()),
            confidence: GameCapabilityConfidence::UserDefined,
            technologies: Vec::new(),
            observed_at: resolved_at.to_string(),
            source_reference: None,
            provider_version: None,
            stale: false,
        })
        .or(provider.clone());
    ResolvedCapability {
        kind,
        value,
        confidence: if override_value.is_some() {
            GameCapabilityConfidence::UserDefined
        } else {
            provider
                .as_ref()
                .map(|item| item.confidence)
                .unwrap_or(GameCapabilityConfidence::Low)
        },
        source,
        technologies,
        alternative_available: provider
            .as_ref()
            .map(|item| item.alternative_available)
            .unwrap_or(GameCapabilityValue::Unknown),
        source_note: provider.as_ref().and_then(|item| item.source_note.clone()),
        evidence: winning_evidence,
        other_evidence: if override_value.is_some() {
            candidates
        } else {
            candidates.into_iter().skip(1).collect()
        },
        resolved_at,
        stale: override_value.is_none() && provider.as_ref().is_some_and(|item| item.stale),
        has_conflict: provider_conflict
            || override_value.is_some_and(|override_value| {
                provider
                    .as_ref()
                    .is_some_and(|item| item.value != override_value)
            }),
    }
}

fn clear_override_internal(
    state: &DatabaseState,
    game_id: &str,
    capability: GameCapabilityKind,
) -> Result<ResolvedGameCapabilities, String> {
    let connection = state.connection.lock().map_err(|_| "database poisoned")?;
    connection
        .execute(
            "DELETE FROM game_capability_overrides WHERE game_id = ?1 AND capability = ?2",
            params![game_id, capability_name(capability)],
        )
        .map_err(|error| error.to_string())?;
    drop(connection);
    state.log(
        "game-capabilities",
        "capabilities.override.clear",
        &format!(
            "game_id={game_id}; capability={}",
            capability_name(capability)
        ),
    );
    resolve_from_storage(state, game_id)
}

fn resolve_from_storage(
    state: &DatabaseState,
    game_id: &str,
) -> Result<ResolvedGameCapabilities, String> {
    let evidence = load_provider_evidence(state, game_id)?;
    let overrides = load_overrides(state, game_id)?;
    let resolved = resolve_game_capabilities(game_id, evidence, overrides);
    state.log(
        "game-capabilities",
        "capabilities.resolve.result",
        &format!("game_id={game_id}; source=storage"),
    );
    Ok(resolved)
}

/// Resolve only from persisted evidence and overrides for a launch. This path
/// is intentionally synchronous and never contacts PCGamingWiki.
pub fn resolve_cached_for_launch(
    state: &DatabaseState,
    game_id: &str,
) -> Result<ResolvedGameCapabilities, String> {
    resolve_from_storage(state, game_id)
}

fn load_overrides(
    state: &DatabaseState,
    game_id: &str,
) -> Result<Vec<GameCapabilityOverride>, String> {
    let connection = state.connection.lock().map_err(|_| "database poisoned")?;
    let mut statement = connection
        .prepare(
            "SELECT capability, override_state, created_at, updated_at
             FROM game_capability_overrides WHERE game_id = ?1",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([game_id], |row| {
            let capability: String = row.get(0)?;
            let override_state: String = row.get(1)?;
            Ok(GameCapabilityOverride {
                game_id: game_id.to_string(),
                capability: parse_capability(&capability).ok_or_else(|| {
                    rusqlite::Error::InvalidParameterName("capability".to_string())
                })?,
                state: parse_override_state(&override_state).ok_or_else(|| {
                    rusqlite::Error::InvalidParameterName("override_state".to_string())
                })?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn load_provider_evidence(
    state: &DatabaseState,
    game_id: &str,
) -> Result<Vec<GameCapabilityEvidence>, String> {
    let connection = state.connection.lock().map_err(|_| "database poisoned")?;
    let mut statement = connection
        .prepare(
            "SELECT capability, normalized_value, source_value, alternative_available, source_note,
                    technologies_json, source, source_page, source_field, confidence, observed_at, provider_version, stale
             FROM pcgamingwiki_capability_evidence WHERE game_id = ?1",
        )
        .map_err(|error| error.to_string())?;
    let rows = statement
        .query_map([game_id], |row| {
            let source: String = row.get(6)?;
            let confidence: String = row.get(9)?;
            let technologies_json: String = row.get(5)?;
            Ok(GameCapabilityEvidence {
                game_id: game_id.to_string(),
                capability: parse_capability(&row.get::<_, String>(0)?).ok_or_else(|| {
                    rusqlite::Error::InvalidParameterName("capability".to_string())
                })?,
                value: parse_value(&row.get::<_, String>(1)?).ok_or_else(|| {
                    rusqlite::Error::InvalidParameterName("normalized_value".to_string())
                })?,
                source: if source == "PCGAMINGWIKI" {
                    GameCapabilitySource::Pcgamingwiki
                } else {
                    GameCapabilitySource::None
                },
                source_value: row.get(2)?,
                alternative_available: parse_value(&row.get::<_, String>(3)?)
                    .unwrap_or(GameCapabilityValue::Unknown),
                source_note: row.get(4)?,
                confidence: parse_confidence(&confidence).unwrap_or(GameCapabilityConfidence::Low),
                technologies: serde_json::from_str(&technologies_json).unwrap_or_default(),
                observed_at: row.get(10)?,
                source_reference: row.get(7)?,
                provider_version: row.get(11)?,
                stale: row.get::<_, i64>(12)? != 0,
            })
        })
        .map_err(|error| error.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn evidence_from_provider(
    game_id: &str,
    capabilities: &pcgamingwiki::PcgamingwikiCapabilities,
) -> Vec<GameCapabilityEvidence> {
    [
        &capabilities.native_hdr,
        &capabilities.high_fidelity_upscaling,
        &capabilities.frame_generation,
    ]
    .into_iter()
    .map(|item| GameCapabilityEvidence {
        game_id: game_id.to_string(),
        capability: map_capability(item.capability.clone()),
        value: map_value(item.normalized_value.clone()),
        source: GameCapabilitySource::Pcgamingwiki,
        source_value: item.source_value.clone(),
        alternative_available: map_value(item.alternative_available.clone()),
        source_note: item.source_note.clone(),
        confidence: map_confidence(item.confidence.clone()),
        technologies: item.technologies.clone(),
        observed_at: item.observed_at.clone(),
        source_reference: Some(item.source_page.clone()),
        provider_version: Some(item.provider_version),
        stale: item.stale,
    })
    .collect()
}

fn map_capability(value: pcgamingwiki::PcgamingwikiCapability) -> GameCapabilityKind {
    match value {
        pcgamingwiki::PcgamingwikiCapability::NativeHdr => GameCapabilityKind::NativeHdr,
        pcgamingwiki::PcgamingwikiCapability::HighFidelityUpscaling => {
            GameCapabilityKind::HighFidelityUpscaling
        }
        pcgamingwiki::PcgamingwikiCapability::FrameGeneration => {
            GameCapabilityKind::FrameGeneration
        }
    }
}

fn map_value(value: PcgamingwikiNormalizedValue) -> GameCapabilityValue {
    match value {
        PcgamingwikiNormalizedValue::Yes => GameCapabilityValue::Yes,
        PcgamingwikiNormalizedValue::No => GameCapabilityValue::No,
        PcgamingwikiNormalizedValue::Unknown => GameCapabilityValue::Unknown,
    }
}

fn map_confidence(value: PcgamingwikiConfidence) -> GameCapabilityConfidence {
    match value {
        PcgamingwikiConfidence::High => GameCapabilityConfidence::High,
        PcgamingwikiConfidence::Medium => GameCapabilityConfidence::Medium,
        PcgamingwikiConfidence::Low => GameCapabilityConfidence::Low,
    }
}

fn confidence_rank(value: GameCapabilityConfidence) -> u8 {
    match value {
        GameCapabilityConfidence::High => 3,
        GameCapabilityConfidence::Medium => 2,
        GameCapabilityConfidence::Low => 1,
        GameCapabilityConfidence::UserDefined => 4,
    }
}

fn override_value(value: GameCapabilityOverrideState) -> Option<GameCapabilityValue> {
    match value {
        GameCapabilityOverrideState::NoOverride => None,
        GameCapabilityOverrideState::ForceYes => Some(GameCapabilityValue::Yes),
        GameCapabilityOverrideState::ForceNo => Some(GameCapabilityValue::No),
        GameCapabilityOverrideState::ForceUnknown => Some(GameCapabilityValue::Unknown),
    }
}

fn parse_capability(value: &str) -> Option<GameCapabilityKind> {
    match value {
        "NATIVE_HDR" => Some(GameCapabilityKind::NativeHdr),
        "HIGH_FIDELITY_UPSCALING" => Some(GameCapabilityKind::HighFidelityUpscaling),
        "FRAME_GENERATION" => Some(GameCapabilityKind::FrameGeneration),
        _ => None,
    }
}

fn parse_value(value: &str) -> Option<GameCapabilityValue> {
    match value {
        "YES" => Some(GameCapabilityValue::Yes),
        "NO" => Some(GameCapabilityValue::No),
        "UNKNOWN" => Some(GameCapabilityValue::Unknown),
        _ => None,
    }
}

fn parse_confidence(value: &str) -> Option<GameCapabilityConfidence> {
    match value {
        "HIGH" => Some(GameCapabilityConfidence::High),
        "MEDIUM" => Some(GameCapabilityConfidence::Medium),
        "LOW" => Some(GameCapabilityConfidence::Low),
        _ => None,
    }
}

fn parse_override_state(value: &str) -> Option<GameCapabilityOverrideState> {
    match value {
        "NO_OVERRIDE" => Some(GameCapabilityOverrideState::NoOverride),
        "FORCE_YES" => Some(GameCapabilityOverrideState::ForceYes),
        "FORCE_NO" => Some(GameCapabilityOverrideState::ForceNo),
        "FORCE_UNKNOWN" => Some(GameCapabilityOverrideState::ForceUnknown),
        _ => None,
    }
}

fn capability_name(value: GameCapabilityKind) -> &'static str {
    match value {
        GameCapabilityKind::NativeHdr => "NATIVE_HDR",
        GameCapabilityKind::HighFidelityUpscaling => "HIGH_FIDELITY_UPSCALING",
        GameCapabilityKind::FrameGeneration => "FRAME_GENERATION",
    }
}

fn override_state_name(value: GameCapabilityOverrideState) -> &'static str {
    match value {
        GameCapabilityOverrideState::NoOverride => "NO_OVERRIDE",
        GameCapabilityOverrideState::ForceYes => "FORCE_YES",
        GameCapabilityOverrideState::ForceNo => "FORCE_NO",
        GameCapabilityOverrideState::ForceUnknown => "FORCE_UNKNOWN",
    }
}

fn value_name(value: GameCapabilityValue) -> &'static str {
    match value {
        GameCapabilityValue::Yes => "YES",
        GameCapabilityValue::No => "NO",
        GameCapabilityValue::Unknown => "UNKNOWN",
    }
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn evidence(
        capability: GameCapabilityKind,
        value: GameCapabilityValue,
        technologies: &[&str],
        stale: bool,
    ) -> GameCapabilityEvidence {
        GameCapabilityEvidence {
            game_id: "marvel".to_string(),
            capability,
            value,
            source: GameCapabilitySource::Pcgamingwiki,
            source_value: Some(value_name(value).to_string()),
            alternative_available: GameCapabilityValue::Unknown,
            source_note: None,
            confidence: GameCapabilityConfidence::High,
            technologies: technologies
                .iter()
                .map(|item| (*item).to_string())
                .collect(),
            observed_at: "1".to_string(),
            source_reference: Some("Marvel Tōkon: Fighting Souls".to_string()),
            provider_version: Some(1),
            stale,
        }
    }

    fn override_item(
        capability: GameCapabilityKind,
        state: GameCapabilityOverrideState,
    ) -> GameCapabilityOverride {
        GameCapabilityOverride {
            game_id: "marvel".to_string(),
            capability,
            state,
            created_at: 1,
            updated_at: 1,
        }
    }

    #[test]
    fn provider_evidence_wins_without_override() {
        let resolved = resolve_game_capabilities(
            "marvel",
            vec![evidence(
                GameCapabilityKind::NativeHdr,
                GameCapabilityValue::Yes,
                &[],
                false,
            )],
            Vec::new(),
        );
        assert_eq!(resolved.native_hdr.value, GameCapabilityValue::Yes);
        assert_eq!(
            resolved.native_hdr.source,
            GameCapabilitySource::Pcgamingwiki
        );
        assert!(!resolved.native_hdr.has_conflict);
    }

    #[test]
    fn override_wins_and_keeps_conflicting_provider_evidence() {
        let resolved = resolve_game_capabilities(
            "marvel",
            vec![evidence(
                GameCapabilityKind::NativeHdr,
                GameCapabilityValue::No,
                &[],
                false,
            )],
            vec![override_item(
                GameCapabilityKind::NativeHdr,
                GameCapabilityOverrideState::ForceYes,
            )],
        );
        assert_eq!(resolved.native_hdr.value, GameCapabilityValue::Yes);
        assert_eq!(
            resolved.native_hdr.source,
            GameCapabilitySource::UserOverride
        );
        assert!(resolved.native_hdr.has_conflict);
        assert_eq!(resolved.native_hdr.other_evidence.len(), 1);
    }

    #[test]
    fn override_no_clears_supporting_technologies() {
        let resolved = resolve_game_capabilities(
            "marvel",
            vec![evidence(
                GameCapabilityKind::HighFidelityUpscaling,
                GameCapabilityValue::Yes,
                &["DLSS 4", "FSR 4"],
                false,
            )],
            vec![override_item(
                GameCapabilityKind::HighFidelityUpscaling,
                GameCapabilityOverrideState::ForceNo,
            )],
        );
        assert_eq!(
            resolved.high_fidelity_upscaling.value,
            GameCapabilityValue::No
        );
        assert!(resolved.high_fidelity_upscaling.technologies.is_empty());
    }

    #[test]
    fn unknown_and_missing_evidence_remain_distinct_from_provider_failure() {
        let resolved = resolve_game_capabilities(
            "carrion",
            vec![evidence(
                GameCapabilityKind::HighFidelityUpscaling,
                GameCapabilityValue::Unknown,
                &[],
                false,
            )],
            Vec::new(),
        );
        assert_eq!(
            resolved.high_fidelity_upscaling.value,
            GameCapabilityValue::Unknown
        );
        assert_eq!(resolved.native_hdr.source, GameCapabilitySource::None);
        assert_eq!(resolved.native_hdr.value, GameCapabilityValue::Unknown);
    }

    #[test]
    fn stale_provider_evidence_can_still_resolve() {
        let resolved = resolve_game_capabilities(
            "marvel",
            vec![evidence(
                GameCapabilityKind::NativeHdr,
                GameCapabilityValue::Yes,
                &[],
                true,
            )],
            Vec::new(),
        );
        assert_eq!(resolved.native_hdr.value, GameCapabilityValue::Yes);
        assert!(resolved.native_hdr.stale);
    }

    #[test]
    fn preserves_alternatives_and_notes_when_native_value_is_no() {
        let mut item = evidence(
            GameCapabilityKind::NativeHdr,
            GameCapabilityValue::No,
            &[],
            false,
        );
        item.alternative_available = GameCapabilityValue::Yes;
        item.source_note = Some("See the engine page for native HDR alternatives.".to_string());
        let resolved = resolve_game_capabilities("marvel", vec![item], Vec::new());
        assert_eq!(resolved.native_hdr.value, GameCapabilityValue::No);
        assert_eq!(
            resolved.native_hdr.alternative_available,
            GameCapabilityValue::Yes
        );
        assert_eq!(
            resolved.native_hdr.source_note.as_deref(),
            Some("See the engine page for native HDR alternatives.")
        );
    }

    #[test]
    fn override_persists_across_database_reopen_and_clear_removes_it() {
        let root =
            std::env::temp_dir().join(format!("lumadeck-game-capabilities-{}", std::process::id()));
        let state = DatabaseState::open(
            crate::data_directory::DataDirectoryResolver::for_app_data(&root),
        )
        .expect("database");
        {
            let connection = state.connection.lock().expect("connection");
            connection
                .execute(
                    "INSERT INTO games(id, title, sort_title, provider, platform, created_at, updated_at)
                     VALUES ('override-game', 'Override Game', 'override game', 'local', 'pc', '1', '1')",
                    [],
                )
                .expect("game row");
        }
        let set = set_override_internal(
            &state,
            "override-game",
            GameCapabilityKind::NativeHdr,
            GameCapabilityOverrideState::ForceYes,
        )
        .expect("set override");
        assert_eq!(set.native_hdr.value, GameCapabilityValue::Yes);
        assert_eq!(set.native_hdr.source, GameCapabilitySource::UserOverride);
        let _ = set_override_internal(
            &state,
            "override-game",
            GameCapabilityKind::HighFidelityUpscaling,
            GameCapabilityOverrideState::ForceNo,
        )
        .expect("second capability override");
        {
            let connection = state.connection.lock().expect("connection");
            connection
                .execute(
                    "INSERT INTO games(id, title, sort_title, provider, platform, created_at, updated_at)
                     VALUES ('other-override-game', 'Other Override Game', 'other override game', 'local', 'pc', '1', '1')",
                    [],
                )
                .expect("second game row");
        }
        let _ = set_override_internal(
            &state,
            "other-override-game",
            GameCapabilityKind::FrameGeneration,
            GameCapabilityOverrideState::ForceYes,
        )
        .expect("other game override");
        drop(state);

        let reopened = DatabaseState::open(
            crate::data_directory::DataDirectoryResolver::for_app_data(&root),
        )
        .expect("reopened database");
        let persisted = resolve_from_storage(&reopened, "override-game").expect("persisted");
        assert_eq!(persisted.native_hdr.value, GameCapabilityValue::Yes);
        assert_eq!(
            persisted.high_fidelity_upscaling.value,
            GameCapabilityValue::No
        );
        let other_persisted =
            resolve_from_storage(&reopened, "other-override-game").expect("other persisted");
        assert_eq!(
            other_persisted.frame_generation.value,
            GameCapabilityValue::Yes
        );
        let cleared =
            clear_override_internal(&reopened, "override-game", GameCapabilityKind::NativeHdr)
                .expect("clear override");
        assert_eq!(cleared.native_hdr.value, GameCapabilityValue::Unknown);
        assert_eq!(cleared.native_hdr.source, GameCapabilitySource::None);
        drop(reopened);
        std::fs::remove_dir_all(root).expect("temporary database cleanup");
    }

    #[test]
    #[ignore = "real PCGamingWiki Game Capabilities QA; requires network access"]
    fn real_game_capabilities_qa_marvel_and_carrion() {
        tauri::async_runtime::block_on(async {
            let root = std::env::temp_dir().join(format!(
                "lumadeck-game-capabilities-qa-{}",
                std::process::id()
            ));
            let state = DatabaseState::open(
                crate::data_directory::DataDirectoryResolver::for_app_data(&root),
            )
            .expect("database");
            {
                let connection = state.connection.lock().expect("connection");
                connection
                    .execute(
                        "INSERT INTO games(id, title, sort_title, provider, platform, created_at, updated_at)
                         VALUES ('qa-marvel', 'Marvel Tōkon: Fighting Souls', 'marvel tokon', 'steam', 'pc', '1', '1'),
                                ('qa-carrion', 'Carrion', 'carrion', 'gog', 'pc', '1', '1')",
                        [],
                    )
                    .expect("QA games");
            }

            let marvel = resolve_with_provider(
                &state,
                GameCapabilitiesRequest {
                    game_id: "qa-marvel".to_string(),
                    steam_app_id: Some(3_787_240),
                    gog_product_id: None,
                },
                true,
            )
            .await
            .expect("Marvel capabilities");
            assert_eq!(marvel.native_hdr.value, GameCapabilityValue::No);
            assert_eq!(
                marvel.native_hdr.alternative_available,
                GameCapabilityValue::Yes
            );
            assert!(marvel
                .native_hdr
                .source_note
                .as_deref()
                .is_some_and(|note| note.contains("native HDR")));
            assert_eq!(
                marvel.high_fidelity_upscaling.value,
                GameCapabilityValue::Yes
            );
            assert_eq!(
                marvel.high_fidelity_upscaling.technologies,
                ["TSR", "DLSS 4", "NIS", "FSR 4", "XeSS 2"]
            );
            assert_eq!(marvel.frame_generation.value, GameCapabilityValue::No);
            assert_eq!(
                marvel.frame_generation.alternative_available,
                GameCapabilityValue::Yes
            );
            assert!(marvel
                .frame_generation
                .source_note
                .as_deref()
                .is_some_and(|note| note.contains("workarounds")));
            let log_path = state.logs_directory().join("settings-runtime.log");
            let before_override = std::fs::read_to_string(&log_path)
                .expect("QA diagnostics")
                .matches("checkpoint=pcgw.http")
                .count();

            let overridden = set_override_internal(
                &state,
                "qa-marvel",
                GameCapabilityKind::NativeHdr,
                GameCapabilityOverrideState::ForceYes,
            )
            .expect("Marvel override");
            assert_eq!(overridden.native_hdr.value, GameCapabilityValue::Yes);
            assert_eq!(
                overridden.native_hdr.source,
                GameCapabilitySource::UserOverride
            );
            assert!(overridden.native_hdr.has_conflict);
            let cleared =
                clear_override_internal(&state, "qa-marvel", GameCapabilityKind::NativeHdr)
                    .expect("Marvel clear override");
            assert_eq!(cleared.native_hdr.value, GameCapabilityValue::No);
            assert_eq!(
                cleared.native_hdr.source,
                GameCapabilitySource::Pcgamingwiki
            );
            let after_override = std::fs::read_to_string(&log_path)
                .expect("QA diagnostics after override")
                .matches("checkpoint=pcgw.http")
                .count();
            assert_eq!(before_override, after_override);

            let carrion = resolve_with_provider(
                &state,
                GameCapabilitiesRequest {
                    game_id: "qa-carrion".to_string(),
                    steam_app_id: None,
                    gog_product_id: Some("1785384169".to_string()),
                },
                true,
            )
            .await
            .expect("Carrion capabilities");
            assert_eq!(carrion.native_hdr.value, GameCapabilityValue::No);
            assert_eq!(
                carrion.high_fidelity_upscaling.value,
                GameCapabilityValue::Unknown
            );
            assert_eq!(carrion.frame_generation.value, GameCapabilityValue::Unknown);

            drop(state);
            std::fs::remove_dir_all(root).expect("temporary QA database cleanup");
        });
    }
}
