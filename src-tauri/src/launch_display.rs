use crate::{
    auto_hdr, display, game_capabilities,
    graphics_profile::{self, HdrModeRecommendation},
    hardware_capabilities, rtx_hdr, settings,
};
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DisplayApplyResult {
    pub changed: bool,
    pub warnings: Vec<String>,
}

pub fn resolve_cached_hdr_recommendation(
    database: &settings::DatabaseState,
    profile: &display::DisplayProfile,
) -> Result<Option<HdrModeRecommendation>, String> {
    if profile.hdr_mode != display::DisplayHdrMode::Auto {
        return Ok(None);
    }
    let display_id = profile
        .display_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "DISPLAY_PROFILE_TARGET_REQUIRED".to_string())?;
    let current_mode = display::current_mode(Some(display_id))?;
    let modes = display::enumerate_modes_for_display(display_id)?;
    let hdr = display::get_hdr_state(display_id)?;
    let game_capabilities =
        game_capabilities::resolve_cached_for_launch(database, &profile.game_id)?;
    let hardware = hardware_capabilities::cached_for_launch();
    let supported_resolutions = modes
        .iter()
        .map(|mode| graphics_profile::DisplayResolution {
            width: mode.width,
            height: mode.height,
        })
        .collect::<Vec<_>>();
    let supported_refresh_rates = modes
        .iter()
        .map(|mode| mode.refresh_rate)
        .collect::<Vec<_>>();
    let recommendation = graphics_profile::resolve(&graphics_profile::GraphicsProfileInput {
        game_id: profile.game_id.clone(),
        game_capabilities,
        hardware,
        display: graphics_profile::DisplayCapabilities {
            display_id: display_id.to_string(),
            current_resolution: Some(graphics_profile::DisplayResolution {
                width: current_mode.width,
                height: current_mode.height,
            }),
            supported_resolutions,
            current_refresh_rate: Some(current_mode.refresh_rate),
            supported_refresh_rates,
            hdr_supported: hdr.supported,
            hdr_enabled: hdr.enabled,
        },
    })
    .display
    .hdr_mode;
    database.log(
        "display-profile",
        "display_profile.auto_hdr.resolved",
        &format!(
            "gameId={};displayId={};recommendation={recommendation:?};source=cache;httpRequests=0",
            profile.game_id, display_id
        ),
    );
    Ok(Some(recommendation))
}

pub fn apply_profile(
    database: &settings::DatabaseState,
    session_id: &str,
    game_id: &str,
    profile: &display::DisplayProfile,
    recommendation: Option<HdrModeRecommendation>,
    executable: Option<&Path>,
) -> Result<DisplayApplyResult, String> {
    if let Some(pending) = settings::get_pending_display_profile_restore(database)
        .map_err(|error| error.to_string())?
    {
        return Err(format!(
            "DISPLAY_PROFILE_OTHER_SESSION_ACTIVE:{}",
            pending.session_id
        ));
    }

    let rtx_preset = requested_rtx_preset(profile, recommendation);
    let auto_hdr_required = rtx_preset.is_some();
    if rtx_preset.is_some() && executable.is_none() {
        return Err("RTX_HDR_EXECUTABLE_REQUIRED".to_string());
    }
    if !profile_requires_display(profile) {
        log_event(
            database,
            session_id,
            "DISPLAY_PROFILE_NOOP",
            "reason=system-modes",
        );
        return Ok(DisplayApplyResult::default());
    }

    let display_id = profile
        .display_id
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "DISPLAY_PROFILE_TARGET_REQUIRED".to_string())?;
    let current_mode = display::current_mode(Some(display_id))?;
    let hdr_state = display::get_hdr_state(display_id)?;
    let current_hdr = hdr_state
        .enabled
        .ok_or_else(|| "DISPLAY_HDR_UNAVAILABLE".to_string())?;
    let target_mode = target_mode(profile, &current_mode, display_id)?;
    let mut warnings = Vec::new();
    let target_hdr = resolve_hdr_target(
        profile.hdr_mode,
        recommendation,
        hdr_state.supported == Some(true),
        &mut warnings,
    );
    if rtx_preset.is_some() && target_hdr != Some(true) {
        return Err("RTX_HDR_REQUIRES_WINDOWS_HDR".to_string());
    }
    let changed_resolution = profile.resolution_mode == display::DisplayResolutionMode::Custom
        && (target_mode.width != current_mode.width || target_mode.height != current_mode.height);
    let changed_refresh_rate = profile.refresh_rate_mode == display::DisplayRefreshRateMode::Custom
        && target_mode.refresh_rate != current_mode.refresh_rate;
    let changed_hdr = target_hdr.is_some_and(|value| value != current_hdr);
    let rtx_snapshot = match (rtx_preset, executable) {
        (Some(_), Some(path)) => Some(rtx_hdr::capture_for_launch(path)?),
        _ => None,
    };
    let auto_hdr_snapshot = if auto_hdr_required {
        Some(auto_hdr::capture(executable.ok_or_else(|| {
            "AUTO_HDR_EXECUTABLE_REQUIRED".to_string()
        })?)?)
    } else {
        None
    };

    if !changed_resolution
        && !changed_refresh_rate
        && !changed_hdr
        && rtx_preset.is_none()
        && !auto_hdr_required
    {
        log_event(
            database,
            session_id,
            "DISPLAY_PROFILE_NOOP",
            "reason=already-applied",
        );
        return Ok(DisplayApplyResult {
            changed: false,
            warnings,
        });
    }

    let pending = settings::PendingDisplayProfileRestore {
        session_id: session_id.to_string(),
        game_id: game_id.to_string(),
        snapshot: display::DisplayProfileSnapshot {
            display_id: display_id.to_string(),
            width: current_mode.width,
            height: current_mode.height,
            refresh_rate: current_mode.refresh_rate,
            hdr_enabled: current_hdr,
            captured_at: timestamp(),
        },
        changed_resolution,
        changed_refresh_rate,
        changed_hdr,
        rtx_hdr_snapshot: rtx_snapshot
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| format!("RTX_HDR_SNAPSHOT_SERIALIZE_FAILED:{error}"))?,
        auto_hdr_snapshot: auto_hdr_snapshot
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| format!("AUTO_HDR_SNAPSHOT_SERIALIZE_FAILED:{error}"))?,
        rtx_hdr_executable: executable.map(|path| path.display().to_string()),
        changed_rtx_hdr: rtx_preset.is_some(),
        changed_auto_hdr: auto_hdr_required,
    };
    log_event(
        database,
        session_id,
        "DISPLAY_PROFILE_SNAPSHOT_CAPTURED",
        &format!("displayId={display_id}"),
    );
    settings::save_pending_display_profile_restore(database, &pending)
        .map_err(|error| error.to_string())?;
    log_event(
        database,
        session_id,
        "DISPLAY_PROFILE_JOURNAL_WRITTEN",
        &format!("gameId={game_id};displayId={display_id}"),
    );

    if changed_resolution || changed_refresh_rate {
        log_event(
            database,
            session_id,
            "DISPLAY_PROFILE_APPLY_START",
            &format!("displayId={display_id}"),
        );
        if let Err(error) = display::apply_mode(&target_mode)
            .and_then(|applied| verify_mode(&applied, &target_mode))
        {
            if error.contains("DISPLAY_MODE_TEST_REJECTED") {
                log_event(
                    database,
                    session_id,
                    "DISPLAY_PROFILE_MODE_TEST_REJECTED",
                    &error,
                );
            }
            return Err(fail_after_apply(database, session_id, &pending, error));
        }
        log_event(
            database,
            session_id,
            "DISPLAY_PROFILE_MODE_APPLIED",
            &format!(
                "displayId={display_id};resolution={}x{};refreshRate={}",
                target_mode.width, target_mode.height, target_mode.refresh_rate
            ),
        );
        log_event(
            database,
            session_id,
            "DISPLAY_PROFILE_VERIFY_SUCCESS",
            &format!("displayId={display_id};property=mode"),
        );
    }

    if changed_hdr {
        match display::set_hdr_enabled(display_id, target_hdr.expect("changed HDR has target")) {
            Ok(state) if state.enabled == target_hdr => {
                log_event(
                    database,
                    session_id,
                    "DISPLAY_PROFILE_HDR_APPLIED",
                    &format!(
                        "displayId={display_id};enabled={}",
                        target_hdr.unwrap_or(false)
                    ),
                );
                log_event(
                    database,
                    session_id,
                    "DISPLAY_PROFILE_VERIFY_SUCCESS",
                    &format!("displayId={display_id};property=hdr"),
                );
            }
            Ok(_) => {
                if rtx_preset.is_some() {
                    return Err(fail_after_apply(
                        database,
                        session_id,
                        &pending,
                        "DISPLAY_HDR_VERIFY_FAILED".to_string(),
                    ));
                }
                handle_hdr_failure(
                    database,
                    session_id,
                    &pending,
                    "DISPLAY_HDR_VERIFY_FAILED".to_string(),
                    &mut warnings,
                );
            }
            Err(error) => {
                if rtx_preset.is_some() {
                    return Err(fail_after_apply(
                        database,
                        session_id,
                        &pending,
                        format!("DISPLAY_HDR_APPLY_FAILED:{error}"),
                    ));
                }
                handle_hdr_failure(
                    database,
                    session_id,
                    &pending,
                    format!("DISPLAY_HDR_APPLY_WARNING:{error}"),
                    &mut warnings,
                );
            }
        }
    }

    if auto_hdr_required {
        if let Err(error) =
            auto_hdr::disable(executable.ok_or_else(|| "AUTO_HDR_EXECUTABLE_REQUIRED".to_string())?)
        {
            return Err(fail_after_apply(database, session_id, &pending, error));
        }
        log_event(
            database,
            session_id,
            "DISPLAY_PROFILE_AUTO_HDR_DISABLED",
            "verified=true",
        );
    }

    if let Some(preset) = rtx_preset {
        let path = executable.ok_or_else(|| "RTX_HDR_EXECUTABLE_REQUIRED".to_string())?;
        if let Err(error) = rtx_hdr::apply_for_launch(path, preset, profile.rtx_hdr_peak_nits) {
            return Err(fail_after_apply(
                database,
                session_id,
                &pending,
                format!("RTX_HDR_APPLY_FAILED:{error};VERIFY_NVIDIA_APP_OVERLAY_GAME_FILTERS"),
            ));
        }
        log_event(
            database,
            session_id,
            "DISPLAY_PROFILE_RTX_HDR_APPLIED",
            &format!("preset={preset:?};peakNits={}", profile.rtx_hdr_peak_nits),
        );
    }

    Ok(DisplayApplyResult {
        changed: changed_resolution
            || changed_refresh_rate
            || changed_hdr
            || rtx_preset.is_some()
            || auto_hdr_required,
        warnings,
    })
}

pub fn restore(
    database: &settings::DatabaseState,
    expected_session_id: Option<&str>,
) -> Result<(), String> {
    let Some(pending) = settings::get_pending_display_profile_restore(database)
        .map_err(|error| error.to_string())?
    else {
        return Ok(());
    };
    if let Some(expected) = expected_session_id {
        if pending.session_id != expected {
            return Err("DISPLAY_PROFILE_OTHER_SESSION_ACTIVE".to_string());
        }
    }

    if pending.changed_rtx_hdr {
        let snapshot_json = pending
            .rtx_hdr_snapshot
            .as_deref()
            .ok_or_else(|| "RTX_HDR_SNAPSHOT_MISSING".to_string())?;
        let snapshot: rtx_hdr::RtxHdrProfileSnapshot = serde_json::from_str(snapshot_json)
            .map_err(|error| format!("RTX_HDR_SNAPSHOT_INVALID:{error}"))?;
        let readback = rtx_hdr::restore_for_launch(&snapshot)?;
        if !readback.supported {
            return Err("RTX_HDR_RESTORE_VERIFY_FAILED".to_string());
        }
    }

    if pending.changed_auto_hdr {
        let snapshot_json = pending
            .auto_hdr_snapshot
            .as_deref()
            .ok_or_else(|| "AUTO_HDR_SNAPSHOT_MISSING".to_string())?;
        let snapshot: auto_hdr::AutoHdrSnapshot = serde_json::from_str(snapshot_json)
            .map_err(|error| format!("AUTO_HDR_SNAPSHOT_INVALID:{error}"))?;
        auto_hdr::restore(&snapshot)?;
    }

    let display_id = &pending.snapshot.display_id;
    let current_mode = display::current_mode(Some(display_id))?;
    if pending.changed_resolution || pending.changed_refresh_rate {
        let snapshot_mode = display::DisplayMode {
            display_id: display_id.clone(),
            device_name: current_mode.device_name.clone(),
            width: pending.snapshot.width,
            height: pending.snapshot.height,
            refresh_rate: pending.snapshot.refresh_rate,
        };
        if current_mode != snapshot_mode {
            let restored = display::apply_mode(&snapshot_mode)?;
            verify_mode(&restored, &snapshot_mode)?;
        }
        let verified = display::current_mode(Some(display_id))?;
        if verified.width != pending.snapshot.width
            || verified.height != pending.snapshot.height
            || verified.refresh_rate != pending.snapshot.refresh_rate
        {
            return Err("DISPLAY_PROFILE_MODE_RESTORE_VERIFY_FAILED".to_string());
        }
    }

    if pending.changed_hdr {
        let current_hdr = display::get_hdr_state(display_id)?;
        let enabled = current_hdr
            .enabled
            .ok_or_else(|| "DISPLAY_HDR_UNAVAILABLE".to_string())?;
        if enabled != pending.snapshot.hdr_enabled {
            let restored = display::set_hdr_enabled(display_id, pending.snapshot.hdr_enabled)?;
            if restored.enabled != Some(pending.snapshot.hdr_enabled) {
                return Err("DISPLAY_PROFILE_HDR_RESTORE_VERIFY_FAILED".to_string());
            }
        }
    }

    settings::clear_pending_display_profile_restore(database).map_err(|error| error.to_string())?;
    log_event(
        database,
        expected_session_id.unwrap_or(&pending.session_id),
        "DISPLAY_PROFILE_RESTORED",
        &format!("displayId={display_id};gameId={}", pending.game_id),
    );
    Ok(())
}

pub fn recover_pending(database: &settings::DatabaseState) -> Result<bool, String> {
    let pending = settings::get_pending_display_profile_restore(database)
        .map_err(|error| error.to_string())?;
    if pending.is_none() {
        return Ok(false);
    }
    let session_id = pending
        .as_ref()
        .map(|value| value.session_id.as_str())
        .unwrap_or("recovery");
    log_event(
        database,
        session_id,
        "DISPLAY_PROFILE_RECOVERY_STARTED",
        "startup=true",
    );
    restore(database, None)?;
    Ok(true)
}

fn profile_requires_display(profile: &display::DisplayProfile) -> bool {
    profile.resolution_mode == display::DisplayResolutionMode::Custom
        || profile.refresh_rate_mode == display::DisplayRefreshRateMode::Custom
        || profile.hdr_mode != display::DisplayHdrMode::System
        || profile.rtx_hdr_preset.is_some()
}

fn requested_rtx_preset(
    profile: &display::DisplayProfile,
    recommendation: Option<HdrModeRecommendation>,
) -> Option<rtx_hdr::RtxHdrPreset> {
    profile
        .rtx_hdr_preset
        .or(match (profile.hdr_mode, recommendation) {
            (display::DisplayHdrMode::Auto, Some(HdrModeRecommendation::RtxHdrNatural)) => {
                Some(rtx_hdr::RtxHdrPreset::Natural)
            }
            _ => None,
        })
}

fn target_mode(
    profile: &display::DisplayProfile,
    current: &display::DisplayMode,
    display_id: &str,
) -> Result<display::DisplayMode, String> {
    let (width, height) = match profile.resolution_mode {
        display::DisplayResolutionMode::System => (current.width, current.height),
        display::DisplayResolutionMode::Custom => (
            profile
                .width
                .ok_or_else(|| "DISPLAY_PROFILE_RESOLUTION_REQUIRED".to_string())?,
            profile
                .height
                .ok_or_else(|| "DISPLAY_PROFILE_RESOLUTION_REQUIRED".to_string())?,
        ),
    };
    let refresh_rate = match profile.refresh_rate_mode {
        display::DisplayRefreshRateMode::System => current.refresh_rate,
        display::DisplayRefreshRateMode::Custom => profile
            .refresh_rate
            .ok_or_else(|| "DISPLAY_PROFILE_REFRESH_RATE_REQUIRED".to_string())?,
    };
    let target = display::DisplayMode {
        display_id: display_id.to_string(),
        device_name: current.device_name.clone(),
        width,
        height,
        refresh_rate,
    };
    let supported = display::enumerate_modes_for_display(display_id)?;
    if !supported.iter().any(|mode| {
        mode.width == target.width
            && mode.height == target.height
            && mode.refresh_rate == target.refresh_rate
    }) {
        return Err("DISPLAY_MODE_UNAVAILABLE".to_string());
    }
    Ok(target)
}

fn resolve_hdr_target(
    mode: display::DisplayHdrMode,
    recommendation: Option<HdrModeRecommendation>,
    supported: bool,
    warnings: &mut Vec<String>,
) -> Option<bool> {
    let requested = match mode {
        display::DisplayHdrMode::System => return None,
        display::DisplayHdrMode::Off => Some(false),
        display::DisplayHdrMode::On => Some(true),
        display::DisplayHdrMode::Auto => match recommendation {
            Some(HdrModeRecommendation::Native) => Some(true),
            Some(HdrModeRecommendation::RtxHdrNatural) => Some(true),
            Some(HdrModeRecommendation::Off) => Some(false),
            Some(HdrModeRecommendation::AlternativeAvailable) => {
                warnings.push("DISPLAY_HDR_ALTERNATIVE_NOT_APPLIED".to_string());
                Some(false)
            }
            Some(
                HdrModeRecommendation::System
                | HdrModeRecommendation::Auto
                | HdrModeRecommendation::Unknown,
            )
            | None => {
                warnings.push("DISPLAY_HDR_AUTO_NO_DETERMINISTIC_RECOMMENDATION".to_string());
                None
            }
        },
    };
    if requested == Some(true) && !supported {
        warnings.push("DISPLAY_HDR_UNSUPPORTED_NOT_APPLIED".to_string());
        None
    } else {
        requested
    }
}

fn verify_mode(actual: &display::DisplayMode, target: &display::DisplayMode) -> Result<(), String> {
    if actual.width == target.width
        && actual.height == target.height
        && actual.refresh_rate == target.refresh_rate
    {
        Ok(())
    } else {
        Err("DISPLAY_MODE_VERIFY_FAILED".to_string())
    }
}

fn fail_after_apply(
    database: &settings::DatabaseState,
    session_id: &str,
    pending: &settings::PendingDisplayProfileRestore,
    error: String,
) -> String {
    match restore(database, Some(session_id)) {
        Ok(()) => error,
        Err(restore_error) => {
            log_event(
                database,
                session_id,
                "DISPLAY_PROFILE_ROLLBACK_FAILED",
                &format!("error={restore_error}"),
            );
            let _ = pending;
            format!("{error};DISPLAY_PROFILE_ROLLBACK_FAILED:{restore_error}")
        }
    }
}

fn handle_hdr_failure(
    database: &settings::DatabaseState,
    session_id: &str,
    pending: &settings::PendingDisplayProfileRestore,
    warning: String,
    warnings: &mut Vec<String>,
) {
    let display_id = &pending.snapshot.display_id;
    let restored = match display::get_hdr_state(display_id) {
        Ok(state) if state.enabled == Some(pending.snapshot.hdr_enabled) => true,
        Ok(_) => display::set_hdr_enabled(display_id, pending.snapshot.hdr_enabled)
            .is_ok_and(|state| state.enabled == Some(pending.snapshot.hdr_enabled)),
        Err(_) => false,
    };
    if restored {
        let mut updated = pending.clone();
        updated.changed_hdr = false;
        if let Err(error) = settings::save_pending_display_profile_restore(database, &updated) {
            warnings.push(format!(
                "{warning};DISPLAY_HDR_JOURNAL_UPDATE_FAILED:{error}"
            ));
        } else {
            warnings.push(format!("{warning};DISPLAY_HDR_RESTORED"));
            log_event(
                database,
                session_id,
                "DISPLAY_PROFILE_HDR_ROLLBACK_SUCCESS",
                &format!("displayId={display_id}"),
            );
        }
    } else {
        warnings.push(format!("{warning};DISPLAY_HDR_RESTORE_PENDING"));
        log_event(
            database,
            session_id,
            "DISPLAY_PROFILE_HDR_ROLLBACK_FAILED",
            &format!("displayId={display_id}"),
        );
    }
}

fn log_event(database: &settings::DatabaseState, session_id: &str, event: &str, details: &str) {
    database.log(
        "display-profile",
        event,
        &format!("sessionId={session_id};{details}"),
    );
}

fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

#[cfg(test)]
mod tests {
    use super::{profile_requires_display, resolve_hdr_target, target_mode};
    use crate::display::{DisplayHdrMode, DisplayMode, DisplayProfile, DisplayResolutionMode};
    use crate::graphics_profile::HdrModeRecommendation;

    fn profile() -> DisplayProfile {
        DisplayProfile {
            game_id: "test".to_string(),
            enabled: false,
            display_id: Some("DISPLAY1".to_string()),
            device_name: None,
            width: None,
            height: None,
            refresh_rate: None,
            restore_on_exit: true,
            updated_at: None,
            resolution_mode: DisplayResolutionMode::System,
            refresh_rate_mode: crate::display::DisplayRefreshRateMode::System,
            hdr_mode: DisplayHdrMode::System,
            rtx_hdr_preset: None,
            rtx_hdr_peak_nits: crate::rtx_hdr::RTX_HDR_PEAK_NITS_DEFAULT,
        }
    }

    #[test]
    fn system_profile_is_a_noop() {
        assert!(!profile_requires_display(&profile()));
    }

    #[test]
    fn hdr_on_is_not_applied_when_unsupported() {
        let mut warnings = Vec::new();
        assert_eq!(
            resolve_hdr_target(DisplayHdrMode::On, None, false, &mut warnings),
            None
        );
        assert_eq!(warnings, vec!["DISPLAY_HDR_UNSUPPORTED_NOT_APPLIED"]);
    }

    #[test]
    fn hdr_auto_uses_only_deterministic_recommendations() {
        let mut warnings = Vec::new();
        assert_eq!(
            resolve_hdr_target(
                DisplayHdrMode::Auto,
                Some(HdrModeRecommendation::AlternativeAvailable),
                true,
                &mut warnings,
            ),
            Some(false)
        );
        assert_eq!(warnings, vec!["DISPLAY_HDR_ALTERNATIVE_NOT_APPLIED"]);
    }

    #[test]
    fn target_mode_requires_exact_complete_resolution() {
        let mut value = profile();
        value.resolution_mode = DisplayResolutionMode::Custom;
        value.width = Some(1920);
        let current = DisplayMode {
            display_id: "DISPLAY1".to_string(),
            device_name: "DISPLAY1".to_string(),
            width: 2560,
            height: 1440,
            refresh_rate: 60,
        };
        assert_eq!(
            target_mode(&value, &current, "DISPLAY1").unwrap_err(),
            "DISPLAY_PROFILE_RESOLUTION_REQUIRED"
        );
    }
}

#[cfg(all(test, windows))]
mod real_display_qa {
    use crate::display;

    #[test]
    #[ignore = "mutates the active Windows display and restores it"]
    fn safe_supported_mode_transition_restores_original_mode() {
        let current = display::current_mode(None).expect("current display mode");
        println!(
            "current display mode: displayId={} deviceName={} {}x{}@{}",
            current.display_id,
            current.device_name,
            current.width,
            current.height,
            current.refresh_rate
        );
        println!(
            "current CDS_TEST: {:?}",
            display::test_current_display_mode(Some(&current.display_id))
        );
        let modes = display::enumerate_modes_for_display(&current.display_id)
            .expect("supported display modes");
        let same_resolution_refresh = modes
            .iter()
            .filter(|mode| {
                mode.width == current.width
                    && mode.height == current.height
                    && mode.refresh_rate != current.refresh_rate
            })
            .min_by_key(|mode| mode.refresh_rate.abs_diff(current.refresh_rate))
            .cloned();
        let different_resolution = modes
            .iter()
            .find(|mode| {
                (mode.width != current.width || mode.height != current.height)
                    && mode.refresh_rate == current.refresh_rate
            })
            .cloned();
        let combined = modes
            .iter()
            .find(|mode| {
                (mode.width != current.width || mode.height != current.height)
                    && mode.refresh_rate != current.refresh_rate
            })
            .cloned();

        qa_mode_case("same-resolution-refresh", &current, same_resolution_refresh);
        qa_mode_case("different-resolution", &current, different_resolution);
        qa_mode_case("combined-resolution-refresh", &current, combined);
    }

    fn qa_mode_case(
        label: &str,
        current: &display::DisplayMode,
        target: Option<display::DisplayMode>,
    ) {
        let Some(target) = target else {
            println!("{label}: SKIPPED no exact enumerated candidate");
            return;
        };
        match display::apply_mode(&target) {
            Ok(applied) => {
                println!("{label}: APPLIED {applied:?}");
                assert_eq!(applied, target);
                let restored = display::current_mode(Some(&current.display_id))
                    .and_then(|mode| {
                        if mode == *current {
                            Ok(mode)
                        } else {
                            display::apply_mode(current)
                        }
                    })
                    .expect("original mode restore");
                assert_eq!(restored, *current);
            }
            Err(error) => {
                println!("{label}: UNAVAILABLE no mutation committed: {error}");
                assert_eq!(
                    display::current_mode(Some(&current.display_id)).expect("current readback"),
                    *current
                );
            }
        }
    }

    #[test]
    #[ignore = "mutates Windows HDR on a compatible display and restores it"]
    fn hdr_transition_restores_original_state() {
        let current = display::current_mode(None).expect("current display mode");
        let state = display::get_hdr_state(&current.display_id).expect("HDR state");
        println!("HDR QA: {state:?}");
        let (Some(true), Some(original)) = (state.supported, state.enabled) else {
            println!("HDR QA: SKIPPED display does not report compatible HDR");
            return;
        };
        let target = !original;
        match display::set_hdr_enabled(&current.display_id, target) {
            Ok(applied) => {
                assert_eq!(applied.enabled, Some(target));
                let restored =
                    display::set_hdr_enabled(&current.display_id, original).expect("HDR restore");
                assert_eq!(restored.enabled, Some(original));
            }
            Err(error) => {
                println!("HDR QA: UNAVAILABLE no mutation committed: {error}");
                assert_eq!(
                    display::get_hdr_state(&current.display_id)
                        .expect("HDR readback")
                        .enabled,
                    Some(original)
                );
            }
        }
    }
}
