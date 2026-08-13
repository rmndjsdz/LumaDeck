use crate::game_capabilities::{
    GameCapabilitySource, GameCapabilityValue, ResolvedCapability, ResolvedGameCapabilities,
};
#[cfg(test)]
use crate::hardware_capabilities::HardwareFeatureSupport;
pub use crate::hardware_capabilities::{FeatureSupport, HardwareCapabilities, HardwareVendor};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayResolution {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayCapabilities {
    pub display_id: String,
    pub current_resolution: Option<DisplayResolution>,
    pub supported_resolutions: Vec<DisplayResolution>,
    pub current_refresh_rate: Option<u32>,
    pub supported_refresh_rates: Vec<u32>,
    pub hdr_supported: Option<bool>,
    pub hdr_enabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphicsProfileInput {
    pub game_id: String,
    pub game_capabilities: ResolvedGameCapabilities,
    pub hardware: HardwareCapabilities,
    pub display: DisplayCapabilities,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HdrModeRecommendation {
    Off,
    Native,
    RtxHdrNatural,
    System,
    Auto,
    AlternativeAvailable,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UpscalingModeRecommendation {
    Recommended,
    Auto,
    None,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FrameGenerationModeRecommendation {
    Native,
    Off,
    AlternativeAvailable,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LosslessScalingRecommendation {
    NotRecommended,
    NotAvailable,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RecommendationConfidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GraphicsProfileDataSource {
    Pcgamingwiki,
    NvidiaOps,
    LocalHardware,
    LocalDisplay,
    LumadeckRule,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendedGraphicsProfileProvenance {
    pub resolution: GraphicsProfileDataSource,
    pub refresh_rate: GraphicsProfileDataSource,
    pub hdr: GraphicsProfileDataSource,
    pub upscaling: GraphicsProfileDataSource,
    pub frame_generation: GraphicsProfileDataSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendedTechnology {
    pub name: String,
    pub version: Option<String>,
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendedDisplay {
    pub display_id: String,
    pub resolution: Option<DisplayResolution>,
    pub refresh_rate: Option<u32>,
    pub hdr_mode: HdrModeRecommendation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendedUpscaling {
    pub mode: UpscalingModeRecommendation,
    pub technology: Option<RecommendedTechnology>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendedFrameGeneration {
    pub mode: FrameGenerationModeRecommendation,
    pub technology: Option<RecommendedTechnology>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendedLosslessScaling {
    pub recommendation: LosslessScalingRecommendation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendedGraphicsProfile {
    pub game_id: String,
    pub display: RecommendedDisplay,
    pub upscaling: RecommendedUpscaling,
    pub frame_generation: RecommendedFrameGeneration,
    pub lossless_scaling: RecommendedLosslessScaling,
    pub confidence: RecommendationConfidence,
    pub reasons: Vec<String>,
    pub warnings: Vec<String>,
    pub provenance: RecommendedGraphicsProfileProvenance,
}

#[tauri::command]
pub fn resolve_graphics_profile(
    input: GraphicsProfileInput,
) -> Result<RecommendedGraphicsProfile, String> {
    if input.game_id.trim().is_empty() {
        return Err("GRAPHICS_PROFILE_INVALID_GAME_ID".to_string());
    }
    Ok(resolve(&input))
}

pub fn resolve(input: &GraphicsProfileInput) -> RecommendedGraphicsProfile {
    let mut reasons = Vec::new();
    let mut warnings = Vec::new();
    let hdr_mode = resolve_hdr(
        &input.game_capabilities.native_hdr,
        &input.display,
        &input.hardware,
        &mut reasons,
        &mut warnings,
    );
    let upscaling = resolve_upscaling(
        &input.game_capabilities.high_fidelity_upscaling,
        &input.hardware,
        &mut reasons,
        &mut warnings,
    );
    let frame_generation = resolve_frame_generation(
        &input.game_capabilities.frame_generation,
        &input.hardware,
        &mut reasons,
        &mut warnings,
    );

    reasons.push(format!(
        "Display resolution and refresh preserved: {} / {}.",
        format_resolution(input.display.current_resolution.as_ref()),
        input
            .display
            .current_refresh_rate
            .map_or_else(|| "unknown".to_string(), |value| format!("{value} Hz"))
    ));
    if let Some(enabled) = input.display.hdr_enabled {
        reasons.push(format!(
            "Current display HDR state observed as {}; no display setting was changed.",
            if enabled { "enabled" } else { "disabled" }
        ));
    }

    let confidence = confidence_for(&input.game_capabilities, &input.hardware, &input.display);
    let provenance = RecommendedGraphicsProfileProvenance {
        resolution: GraphicsProfileDataSource::LocalDisplay,
        refresh_rate: GraphicsProfileDataSource::LocalDisplay,
        hdr: if hdr_mode == HdrModeRecommendation::Native {
            GraphicsProfileDataSource::Pcgamingwiki
        } else if hdr_mode == HdrModeRecommendation::RtxHdrNatural {
            GraphicsProfileDataSource::LumadeckRule
        } else {
            GraphicsProfileDataSource::Pcgamingwiki
        },
        upscaling: if input.game_capabilities.high_fidelity_upscaling.value
            == GameCapabilityValue::Unknown
        {
            GraphicsProfileDataSource::Pcgamingwiki
        } else {
            GraphicsProfileDataSource::LumadeckRule
        },
        frame_generation: if input.game_capabilities.frame_generation.value
            == GameCapabilityValue::Unknown
        {
            GraphicsProfileDataSource::Pcgamingwiki
        } else {
            GraphicsProfileDataSource::LumadeckRule
        },
    };
    RecommendedGraphicsProfile {
        game_id: input.game_id.clone(),
        display: RecommendedDisplay {
            display_id: input.display.display_id.clone(),
            resolution: input.display.current_resolution.clone(),
            refresh_rate: input.display.current_refresh_rate,
            hdr_mode,
        },
        upscaling,
        frame_generation,
        lossless_scaling: RecommendedLosslessScaling {
            recommendation: resolve_lossless_scaling(
                &input.game_capabilities.frame_generation,
                &mut reasons,
            ),
        },
        confidence,
        reasons,
        warnings,
        provenance,
    }
}

fn resolve_hdr(
    capability: &ResolvedCapability,
    display: &DisplayCapabilities,
    hardware: &HardwareCapabilities,
    reasons: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> HdrModeRecommendation {
    if display.hdr_supported == Some(false) {
        reasons.push("Display does not support HDR; recommendation is SDR/OFF.".to_string());
        if capability.value == GameCapabilityValue::Yes {
            warnings.push(
                "Native HDR is reported by the game, but this display does not support HDR."
                    .to_string(),
            );
        }
        return HdrModeRecommendation::Off;
    }
    match capability.value {
        GameCapabilityValue::Yes if display.hdr_supported == Some(true) => {
            reasons.push(source_reason(
                capability,
                "Game reports native HDR and the display supports HDR; recommend NATIVE.",
            ));
            HdrModeRecommendation::Native
        }
        GameCapabilityValue::Yes => {
            reasons.push(source_reason(
                capability,
                "Game reports native HDR, but display HDR support is unknown.",
            ));
            warnings
                .push("Display HDR support is unknown; native HDR was not selected.".to_string());
            HdrModeRecommendation::System
        }
        GameCapabilityValue::No
            if display.hdr_supported == Some(true)
                && crate::rtx_hdr::is_compatible_hardware(hardware) =>
        {
            reasons.push(source_reason(
                capability,
                "Native HDR is not reported; compatible NVIDIA hardware selects RTX HDR Natural before Auto HDR.",
            ));
            HdrModeRecommendation::RtxHdrNatural
        }
        GameCapabilityValue::No if capability.alternative_available == GameCapabilityValue::Yes => {
            reasons.push(source_reason(
                capability,
                "Native HDR is not reported; PCGamingWiki preserves an alternative/workaround note.",
            ));
            warnings.push(
                "HDR alternative is available, but no workaround will be executed.".to_string(),
            );
            HdrModeRecommendation::AlternativeAvailable
        }
        GameCapabilityValue::No => {
            reasons.push(source_reason(
                capability,
                "Native HDR is not reported; recommend OFF.",
            ));
            HdrModeRecommendation::Off
        }
        GameCapabilityValue::Unknown => {
            reasons.push(source_reason(
                capability,
                "Native HDR support is unknown; no HDR action is recommended.",
            ));
            warnings.push(
                "Game HDR support is unknown; review before changing display settings.".to_string(),
            );
            HdrModeRecommendation::System
        }
    }
}

fn resolve_upscaling(
    capability: &ResolvedCapability,
    hardware: &HardwareCapabilities,
    reasons: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> RecommendedUpscaling {
    match capability.value {
        GameCapabilityValue::No => {
            reasons.push(source_reason(
                capability,
                "Game does not report high-fidelity upscaling.",
            ));
            RecommendedUpscaling {
                mode: UpscalingModeRecommendation::None,
                technology: None,
            }
        }
        GameCapabilityValue::Unknown => {
            reasons.push(source_reason(
                capability,
                "Game upscaling support is unknown.",
            ));
            warnings.push(
                "Upscaling recommendation remains UNKNOWN without game evidence.".to_string(),
            );
            RecommendedUpscaling {
                mode: UpscalingModeRecommendation::Unknown,
                technology: None,
            }
        }
        GameCapabilityValue::Yes => {
            let candidates = priority_for(hardware.vendor);
            match choose_technology(&capability.technologies, hardware, &candidates) {
                Some(technology) => {
                    reasons.push(format!(
                        "{} supports the game-reported technology {}; selected conservatively by vendor priority.",
                        source_label(capability),
                        technology.label
                    ));
                    RecommendedUpscaling {
                        mode: UpscalingModeRecommendation::Recommended,
                        technology: Some(technology),
                    }
                }
                None if matches!(hardware.vendor, HardwareVendor::Unknown) => {
                    reasons.push(source_reason(
                        capability,
                        "Game supports upscaling, but GPU vendor is unknown; no technology was selected.",
                    ));
                    warnings.push(
                        "GPU capability is unknown; upscaling remains AUTO/UNKNOWN.".to_string(),
                    );
                    RecommendedUpscaling {
                        mode: UpscalingModeRecommendation::Auto,
                        technology: None,
                    }
                }
                None => {
                    reasons.push(source_reason(
                        capability,
                        "Game supports upscaling, but no compatible hardware feature is confirmed.",
                    ));
                    warnings.push(
                        "No compatible upscaling feature is confirmed; review before applying."
                            .to_string(),
                    );
                    RecommendedUpscaling {
                        mode: UpscalingModeRecommendation::Auto,
                        technology: None,
                    }
                }
            }
        }
    }
}

fn resolve_frame_generation(
    capability: &ResolvedCapability,
    hardware: &HardwareCapabilities,
    reasons: &mut Vec<String>,
    warnings: &mut Vec<String>,
) -> RecommendedFrameGeneration {
    match capability.value {
        GameCapabilityValue::No if capability.alternative_available == GameCapabilityValue::Yes => {
            reasons.push(source_reason(
                capability,
                "Native frame generation is not reported; PCGamingWiki preserves an alternative/workaround note.",
            ));
            warnings.push(
                "Frame-generation alternative is available; no workaround will be executed."
                    .to_string(),
            );
            RecommendedFrameGeneration {
                mode: FrameGenerationModeRecommendation::Off,
                technology: None,
            }
        }
        GameCapabilityValue::No => {
            reasons.push(source_reason(
                capability,
                "Native frame generation is not reported.",
            ));
            RecommendedFrameGeneration {
                mode: FrameGenerationModeRecommendation::Off,
                technology: None,
            }
        }
        GameCapabilityValue::Unknown => {
            reasons.push(source_reason(
                capability,
                "Frame-generation support is unknown.",
            ));
            warnings.push("Frame generation remains UNKNOWN without game evidence.".to_string());
            RecommendedFrameGeneration {
                mode: FrameGenerationModeRecommendation::Unknown,
                technology: None,
            }
        }
        GameCapabilityValue::Yes => {
            let candidates = priority_for(hardware.vendor);
            match choose_frame_generation(&capability.technologies, hardware, &candidates) {
                Some(technology) => {
                    reasons.push(format!(
                        "{} confirms hardware compatibility for native frame generation via {}.",
                        source_label(capability),
                        technology.label
                    ));
                    RecommendedFrameGeneration {
                        mode: FrameGenerationModeRecommendation::Native,
                        technology: Some(technology),
                    }
                }
                None if has_known_frame_generation_incompatibility(
                    &capability.technologies,
                    hardware,
                ) =>
                {
                    reasons.push(source_reason(
                        capability,
                        "Game reports frame generation, but the explicit hardware matrix is incompatible.",
                    ));
                    warnings.push("Frame-generation hardware compatibility is incompatible; recommendation is OFF.".to_string());
                    RecommendedFrameGeneration {
                        mode: FrameGenerationModeRecommendation::Off,
                        technology: None,
                    }
                }
                None => {
                    reasons.push(source_reason(
                        capability,
                        "Game reports frame generation, but hardware compatibility is not confirmed.",
                    ));
                    warnings.push(
                        "Frame-generation compatibility is unknown; no technology was selected."
                            .to_string(),
                    );
                    RecommendedFrameGeneration {
                        mode: FrameGenerationModeRecommendation::Unknown,
                        technology: None,
                    }
                }
            }
        }
    }
}

fn resolve_lossless_scaling(
    capability: &ResolvedCapability,
    reasons: &mut Vec<String>,
) -> LosslessScalingRecommendation {
    if capability.value == GameCapabilityValue::No
        && capability.alternative_available == GameCapabilityValue::Yes
    {
        reasons.push("Alternative/workaround evidence is preserved for a later layer; Lossless Scaling is not recommended automatically.".to_string());
        return LosslessScalingRecommendation::NotRecommended;
    }
    if capability.value == GameCapabilityValue::Unknown {
        return LosslessScalingRecommendation::Unknown;
    }
    LosslessScalingRecommendation::NotAvailable
}

fn priority_for(vendor: HardwareVendor) -> Vec<TechnologyKind> {
    match vendor {
        HardwareVendor::Nvidia => vec![
            TechnologyKind::Dlss,
            TechnologyKind::Xess,
            TechnologyKind::Fsr,
            TechnologyKind::Tsr,
            TechnologyKind::Nis,
        ],
        HardwareVendor::Amd => vec![
            TechnologyKind::Fsr,
            TechnologyKind::Xess,
            TechnologyKind::Tsr,
        ],
        HardwareVendor::Intel => vec![
            TechnologyKind::Xess,
            TechnologyKind::Fsr,
            TechnologyKind::Tsr,
        ],
        HardwareVendor::Other | HardwareVendor::Unknown => vec![
            TechnologyKind::Fsr,
            TechnologyKind::Xess,
            TechnologyKind::Tsr,
        ],
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TechnologyKind {
    Dlss,
    Xess,
    Fsr,
    Tsr,
    Nis,
}

fn choose_technology(
    labels: &[String],
    hardware: &HardwareCapabilities,
    candidates: &[TechnologyKind],
) -> Option<RecommendedTechnology> {
    candidates.iter().find_map(|kind| {
        let label = labels
            .iter()
            .find(|label| technology_kind(label) == Some(*kind))?;
        if hardware_support(hardware, *kind) == FeatureSupport::Supported {
            Some(to_technology(label, *kind))
        } else {
            None
        }
    })
}

fn choose_frame_generation(
    labels: &[String],
    hardware: &HardwareCapabilities,
    candidates: &[TechnologyKind],
) -> Option<RecommendedTechnology> {
    candidates.iter().find_map(|kind| {
        let label = labels.iter().find(|label| {
            technology_kind(label) == Some(*kind) && is_frame_generation_label(label)
        })?;
        if frame_generation_support(hardware, *kind) == FeatureSupport::Supported {
            Some(to_technology(label, *kind))
        } else {
            None
        }
    })
}

fn has_known_frame_generation_incompatibility(
    labels: &[String],
    hardware: &HardwareCapabilities,
) -> bool {
    labels
        .iter()
        .filter_map(|label| technology_kind(label))
        .any(|kind| {
            labels.iter().any(|label| {
                technology_kind(label) == Some(kind)
                    && is_frame_generation_label(label)
                    && frame_generation_support(hardware, kind) == FeatureSupport::Unsupported
            })
        })
}

fn technology_kind(label: &str) -> Option<TechnologyKind> {
    let normalized = label.to_ascii_lowercase();
    if normalized.contains("dlss") {
        Some(TechnologyKind::Dlss)
    } else if normalized.contains("xess") {
        Some(TechnologyKind::Xess)
    } else if normalized.contains("fsr") {
        Some(TechnologyKind::Fsr)
    } else if normalized.contains("tsr") {
        Some(TechnologyKind::Tsr)
    } else if normalized.contains("nis") {
        Some(TechnologyKind::Nis)
    } else {
        None
    }
}

fn is_frame_generation_label(label: &str) -> bool {
    let normalized = label.to_ascii_lowercase();
    normalized.contains("frame generation")
        || normalized.contains("frame-generation")
        || normalized.contains("framegen")
        || normalized.ends_with(" fg")
}

fn hardware_support(hardware: &HardwareCapabilities, kind: TechnologyKind) -> FeatureSupport {
    match kind {
        TechnologyKind::Dlss => hardware.feature_support.supports_dlss,
        TechnologyKind::Xess => hardware.feature_support.supports_xess,
        TechnologyKind::Fsr => hardware.feature_support.supports_fsr,
        TechnologyKind::Tsr => hardware.feature_support.supports_tsr,
        TechnologyKind::Nis => hardware.feature_support.supports_nis,
    }
}

fn frame_generation_support(
    hardware: &HardwareCapabilities,
    kind: TechnologyKind,
) -> FeatureSupport {
    match kind {
        TechnologyKind::Dlss => {
            if hardware.vendor != HardwareVendor::Nvidia {
                FeatureSupport::Unsupported
            } else {
                hardware.feature_support.supports_dlss_frame_generation
            }
        }
        TechnologyKind::Xess => hardware.feature_support.supports_xess_frame_generation,
        TechnologyKind::Fsr => hardware.feature_support.supports_fsr_frame_generation,
        TechnologyKind::Tsr | TechnologyKind::Nis => FeatureSupport::Unsupported,
    }
}

fn to_technology(label: &str, kind: TechnologyKind) -> RecommendedTechnology {
    let name = match kind {
        TechnologyKind::Dlss => "DLSS",
        TechnologyKind::Xess => "XeSS",
        TechnologyKind::Fsr => "FSR",
        TechnologyKind::Tsr => "TSR",
        TechnologyKind::Nis => "NIS",
    };
    let version = label
        .split_whitespace()
        .skip(1)
        .collect::<Vec<_>>()
        .join(" ");
    RecommendedTechnology {
        name: name.to_string(),
        version: (!version.is_empty()).then_some(version),
        label: label.to_string(),
    }
}

fn source_reason(capability: &ResolvedCapability, message: &str) -> String {
    format!("{} Source: {}.", message, source_label(capability))
}

fn source_label(capability: &ResolvedCapability) -> &'static str {
    match capability.source {
        GameCapabilitySource::Pcgamingwiki => "PCGamingWiki",
        GameCapabilitySource::UserOverride => "USER_OVERRIDE",
        GameCapabilitySource::None => "NONE",
    }
}

fn format_resolution(resolution: Option<&DisplayResolution>) -> String {
    resolution.map_or_else(
        || "unknown".to_string(),
        |value| format!("{}x{}", value.width, value.height),
    )
}

fn confidence_for(
    capabilities: &ResolvedGameCapabilities,
    hardware: &HardwareCapabilities,
    display: &DisplayCapabilities,
) -> RecommendationConfidence {
    if capabilities.native_hdr.value == GameCapabilityValue::Unknown
        || capabilities.high_fidelity_upscaling.value == GameCapabilityValue::Unknown
        || capabilities.frame_generation.value == GameCapabilityValue::Unknown
        || hardware.vendor == HardwareVendor::Unknown
        || display.hdr_supported.is_none()
    {
        return RecommendationConfidence::Low;
    }
    if capabilities.native_hdr.stale
        || capabilities.high_fidelity_upscaling.stale
        || capabilities.frame_generation.stale
        || (capabilities.high_fidelity_upscaling.value == GameCapabilityValue::Yes
            && capabilities.high_fidelity_upscaling.technologies.is_empty())
    {
        return RecommendationConfidence::Medium;
    }
    RecommendationConfidence::High
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_capabilities::{
        GameCapabilityConfidence, GameCapabilityKind, GameCapabilitySource,
    };

    fn capability(
        kind: GameCapabilityKind,
        value: GameCapabilityValue,
        technologies: &[&str],
        alternative_available: GameCapabilityValue,
    ) -> ResolvedCapability {
        ResolvedCapability {
            kind,
            value,
            confidence: GameCapabilityConfidence::High,
            source: GameCapabilitySource::Pcgamingwiki,
            technologies: technologies
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            alternative_available,
            source_note: None,
            evidence: None,
            other_evidence: Vec::new(),
            resolved_at: 1,
            stale: false,
            has_conflict: false,
        }
    }

    fn game(
        hdr: ResolvedCapability,
        upscaling: ResolvedCapability,
        frame_generation: ResolvedCapability,
    ) -> ResolvedGameCapabilities {
        ResolvedGameCapabilities {
            game_id: "qa-game".to_string(),
            native_hdr: hdr,
            high_fidelity_upscaling: upscaling,
            frame_generation,
            four_k: capability(
                GameCapabilityKind::FourK,
                GameCapabilityValue::Unknown,
                &[],
                GameCapabilityValue::Unknown,
            ),
            sixty_fps: capability(
                GameCapabilityKind::SixtyFps,
                GameCapabilityValue::Unknown,
                &[],
                GameCapabilityValue::Unknown,
            ),
            high_refresh_120_fps: capability(
                GameCapabilityKind::HighRefresh120Fps,
                GameCapabilityValue::Unknown,
                &[],
                GameCapabilityValue::Unknown,
            ),
            resolved_at: 1,
            provider_status: None,
            provider_error: None,
        }
    }

    fn display(hdr_supported: Option<bool>) -> DisplayCapabilities {
        DisplayCapabilities {
            display_id: "display-1".to_string(),
            current_resolution: Some(DisplayResolution {
                width: 2560,
                height: 1440,
            }),
            supported_resolutions: vec![DisplayResolution {
                width: 2560,
                height: 1440,
            }],
            current_refresh_rate: Some(144),
            supported_refresh_rates: vec![144],
            hdr_supported,
            hdr_enabled: Some(false),
        }
    }

    fn nvidia() -> HardwareCapabilities {
        HardwareCapabilities {
            vendor: HardwareVendor::Nvidia,
            feature_support: HardwareFeatureSupport {
                supports_dlss: FeatureSupport::Supported,
                supports_dlss_frame_generation: FeatureSupport::Supported,
                supports_xess: FeatureSupport::Supported,
                supports_fsr: FeatureSupport::Supported,
                supports_tsr: FeatureSupport::Supported,
                supports_nis: FeatureSupport::Supported,
                ..HardwareFeatureSupport::default()
            },
            ..HardwareCapabilities::default()
        }
    }

    #[test]
    fn unknown_native_hdr_resolves_to_system_without_rtx_fallback() {
        let result = resolve(&GraphicsProfileInput {
            game_id: "unknown-hdr".to_string(),
            game_capabilities: game(
                capability(
                    GameCapabilityKind::NativeHdr,
                    GameCapabilityValue::Unknown,
                    &[],
                    GameCapabilityValue::Unknown,
                ),
                capability(
                    GameCapabilityKind::HighFidelityUpscaling,
                    GameCapabilityValue::No,
                    &[],
                    GameCapabilityValue::Unknown,
                ),
                capability(
                    GameCapabilityKind::FrameGeneration,
                    GameCapabilityValue::No,
                    &[],
                    GameCapabilityValue::Unknown,
                ),
            ),
            hardware: nvidia(),
            display: display(Some(true)),
        });
        assert_eq!(result.display.hdr_mode, HdrModeRecommendation::System);
    }

    #[test]
    fn marvel_nvidia_prefers_dlss_and_selects_rtx_hdr_before_auto_hdr() {
        let result = resolve(&GraphicsProfileInput {
            game_id: "marvel-tokon".to_string(),
            game_capabilities: game(
                capability(
                    GameCapabilityKind::NativeHdr,
                    GameCapabilityValue::No,
                    &[],
                    GameCapabilityValue::Yes,
                ),
                capability(
                    GameCapabilityKind::HighFidelityUpscaling,
                    GameCapabilityValue::Yes,
                    &["TSR", "DLSS 4", "NIS", "FSR 4", "XeSS 2"],
                    GameCapabilityValue::Unknown,
                ),
                capability(
                    GameCapabilityKind::FrameGeneration,
                    GameCapabilityValue::No,
                    &[],
                    GameCapabilityValue::Yes,
                ),
            ),
            hardware: nvidia(),
            display: display(Some(true)),
        });
        assert_eq!(
            result.display.hdr_mode,
            HdrModeRecommendation::RtxHdrNatural
        );
        assert_eq!(
            result
                .upscaling
                .technology
                .as_ref()
                .map(|value| value.label.as_str()),
            Some("DLSS 4")
        );
        assert_eq!(
            result
                .upscaling
                .technology
                .as_ref()
                .and_then(|value| value.version.as_deref()),
            Some("4")
        );
        assert_eq!(
            result.frame_generation.mode,
            FrameGenerationModeRecommendation::Off
        );
        assert_eq!(
            result.lossless_scaling.recommendation,
            LosslessScalingRecommendation::NotRecommended
        );
        assert_eq!(
            result.provenance.resolution,
            GraphicsProfileDataSource::LocalDisplay
        );
        assert_eq!(
            result.provenance.refresh_rate,
            GraphicsProfileDataSource::LocalDisplay
        );
        assert_eq!(
            result.provenance.hdr,
            GraphicsProfileDataSource::LumadeckRule
        );
        assert!(result
            .warnings
            .iter()
            .any(|value| value.contains("no workaround")));
    }

    #[test]
    fn unsupported_display_forces_hdr_off_with_warning() {
        let result = resolve(&GraphicsProfileInput {
            game_id: "hdr-game".to_string(),
            game_capabilities: game(
                capability(
                    GameCapabilityKind::NativeHdr,
                    GameCapabilityValue::Yes,
                    &[],
                    GameCapabilityValue::Unknown,
                ),
                capability(
                    GameCapabilityKind::HighFidelityUpscaling,
                    GameCapabilityValue::No,
                    &[],
                    GameCapabilityValue::Unknown,
                ),
                capability(
                    GameCapabilityKind::FrameGeneration,
                    GameCapabilityValue::No,
                    &[],
                    GameCapabilityValue::Unknown,
                ),
            ),
            hardware: nvidia(),
            display: display(Some(false)),
        });
        assert_eq!(result.display.hdr_mode, HdrModeRecommendation::Off);
        assert!(result
            .warnings
            .iter()
            .any(|value| value.contains("does not support HDR")));
    }

    #[test]
    fn vendor_priority_is_conservative_and_unknown_gpu_does_not_select() {
        let game_capabilities = game(
            capability(
                GameCapabilityKind::NativeHdr,
                GameCapabilityValue::No,
                &[],
                GameCapabilityValue::Unknown,
            ),
            capability(
                GameCapabilityKind::HighFidelityUpscaling,
                GameCapabilityValue::Yes,
                &["DLSS 4", "FSR 4", "XeSS 2"],
                GameCapabilityValue::Unknown,
            ),
            capability(
                GameCapabilityKind::FrameGeneration,
                GameCapabilityValue::Unknown,
                &[],
                GameCapabilityValue::Unknown,
            ),
        );
        let unknown = resolve(&GraphicsProfileInput {
            game_id: "unknown-gpu".to_string(),
            game_capabilities: game_capabilities.clone(),
            hardware: HardwareCapabilities::default(),
            display: display(Some(false)),
        });
        assert_eq!(unknown.upscaling.mode, UpscalingModeRecommendation::Auto);
        assert!(unknown.upscaling.technology.is_none());

        let amd = resolve(&GraphicsProfileInput {
            game_id: "amd".to_string(),
            game_capabilities: game_capabilities.clone(),
            hardware: HardwareCapabilities {
                vendor: HardwareVendor::Amd,
                feature_support: HardwareFeatureSupport {
                    supports_fsr: FeatureSupport::Supported,
                    supports_xess: FeatureSupport::Supported,
                    supports_tsr: FeatureSupport::Supported,
                    ..HardwareFeatureSupport::default()
                },
                ..HardwareCapabilities::default()
            },
            display: display(Some(false)),
        });
        assert_eq!(
            amd.upscaling
                .technology
                .as_ref()
                .map(|value| value.label.as_str()),
            Some("FSR 4")
        );

        let intel = resolve(&GraphicsProfileInput {
            game_id: "intel".to_string(),
            game_capabilities: game_capabilities.clone(),
            hardware: HardwareCapabilities {
                vendor: HardwareVendor::Intel,
                feature_support: HardwareFeatureSupport {
                    supports_fsr: FeatureSupport::Supported,
                    supports_xess: FeatureSupport::Supported,
                    supports_tsr: FeatureSupport::Supported,
                    ..HardwareFeatureSupport::default()
                },
                ..HardwareCapabilities::default()
            },
            display: display(Some(false)),
        });
        assert_eq!(
            intel
                .upscaling
                .technology
                .as_ref()
                .map(|value| value.label.as_str()),
            Some("XeSS 2")
        );
    }

    #[test]
    fn frame_generation_requires_explicit_compatible_feature() {
        let result = resolve(&GraphicsProfileInput {
            game_id: "fg-incompatible".to_string(),
            game_capabilities: game(
                capability(
                    GameCapabilityKind::NativeHdr,
                    GameCapabilityValue::No,
                    &[],
                    GameCapabilityValue::Unknown,
                ),
                capability(
                    GameCapabilityKind::HighFidelityUpscaling,
                    GameCapabilityValue::No,
                    &[],
                    GameCapabilityValue::Unknown,
                ),
                capability(
                    GameCapabilityKind::FrameGeneration,
                    GameCapabilityValue::Yes,
                    &["DLSS Frame Generation"],
                    GameCapabilityValue::Unknown,
                ),
            ),
            hardware: HardwareCapabilities {
                vendor: HardwareVendor::Amd,
                feature_support: HardwareFeatureSupport {
                    supports_dlss_frame_generation: FeatureSupport::Supported,
                    ..HardwareFeatureSupport::default()
                },
                ..HardwareCapabilities::default()
            },
            display: display(Some(false)),
        });
        assert_eq!(
            result.frame_generation.mode,
            FrameGenerationModeRecommendation::Off
        );
        assert!(result
            .warnings
            .iter()
            .any(|value| value.contains("incompatible")));
    }

    #[test]
    fn native_hdr_is_selected_only_when_game_and_display_support_it() {
        let result = resolve(&GraphicsProfileInput {
            game_id: "hdr-success".to_string(),
            game_capabilities: game(
                capability(
                    GameCapabilityKind::NativeHdr,
                    GameCapabilityValue::Yes,
                    &[],
                    GameCapabilityValue::Unknown,
                ),
                capability(
                    GameCapabilityKind::HighFidelityUpscaling,
                    GameCapabilityValue::No,
                    &[],
                    GameCapabilityValue::Unknown,
                ),
                capability(
                    GameCapabilityKind::FrameGeneration,
                    GameCapabilityValue::No,
                    &[],
                    GameCapabilityValue::Unknown,
                ),
            ),
            hardware: nvidia(),
            display: display(Some(true)),
        });
        assert_eq!(result.display.hdr_mode, HdrModeRecommendation::Native);
    }

    #[test]
    fn carrion_unknown_capabilities_do_not_invent_recommendations() {
        let result = resolve(&GraphicsProfileInput {
            game_id: "carrion".to_string(),
            game_capabilities: game(
                capability(
                    GameCapabilityKind::NativeHdr,
                    GameCapabilityValue::No,
                    &[],
                    GameCapabilityValue::Unknown,
                ),
                capability(
                    GameCapabilityKind::HighFidelityUpscaling,
                    GameCapabilityValue::Unknown,
                    &[],
                    GameCapabilityValue::Unknown,
                ),
                capability(
                    GameCapabilityKind::FrameGeneration,
                    GameCapabilityValue::Unknown,
                    &[],
                    GameCapabilityValue::Unknown,
                ),
            ),
            hardware: HardwareCapabilities::default(),
            display: display(Some(false)),
        });
        assert_eq!(result.display.hdr_mode, HdrModeRecommendation::Off);
        assert_eq!(result.upscaling.mode, UpscalingModeRecommendation::Unknown);
        assert_eq!(
            result.frame_generation.mode,
            FrameGenerationModeRecommendation::Unknown
        );
        assert_eq!(result.confidence, RecommendationConfidence::Low);
    }

    #[test]
    fn reason_trace_identifies_user_override_source() {
        let mut native_hdr = capability(
            GameCapabilityKind::NativeHdr,
            GameCapabilityValue::Yes,
            &[],
            GameCapabilityValue::Unknown,
        );
        native_hdr.source = GameCapabilitySource::UserOverride;
        let result = resolve(&GraphicsProfileInput {
            game_id: "override-source".to_string(),
            game_capabilities: game(
                native_hdr,
                capability(
                    GameCapabilityKind::HighFidelityUpscaling,
                    GameCapabilityValue::No,
                    &[],
                    GameCapabilityValue::Unknown,
                ),
                capability(
                    GameCapabilityKind::FrameGeneration,
                    GameCapabilityValue::No,
                    &[],
                    GameCapabilityValue::Unknown,
                ),
            ),
            hardware: nvidia(),
            display: display(Some(true)),
        });
        assert!(result
            .reasons
            .iter()
            .any(|reason| reason.contains("USER_OVERRIDE")));
    }
}
