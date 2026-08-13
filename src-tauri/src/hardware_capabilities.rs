use serde::{Deserialize, Serialize};
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HardwareVendor {
    Nvidia,
    Amd,
    Intel,
    Other,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FeatureSupport {
    Supported,
    Unsupported,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HardwareConfidence {
    High,
    Medium,
    #[default]
    Low,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareFeatureSupport {
    pub supports_dlss: FeatureSupport,
    pub supports_dlss_frame_generation: FeatureSupport,
    pub supports_fsr: FeatureSupport,
    pub supports_fsr_frame_generation: FeatureSupport,
    pub supports_xess: FeatureSupport,
    pub supports_xess_frame_generation: FeatureSupport,
    pub preferred_xess: FeatureSupport,
    pub supports_tsr: FeatureSupport,
    pub supports_nis: FeatureSupport,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpuAdapter {
    pub gpu_id: String,
    pub vendor: HardwareVendor,
    pub vendor_id: Option<u32>,
    pub device_id: Option<u32>,
    pub model: String,
    pub dedicated_vram_mb: Option<u64>,
    pub architecture: Option<String>,
    pub driver_version: Option<String>,
    pub luid: Option<String>,
    pub is_software: bool,
    pub feature_support: HardwareFeatureSupport,
    pub confidence: HardwareConfidence,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HardwareCapabilities {
    pub gpu_id: Option<String>,
    pub vendor: HardwareVendor,
    pub model: Option<String>,
    pub dedicated_vram_mb: Option<u64>,
    pub architecture: Option<String>,
    pub driver_version: Option<String>,
    pub feature_support: HardwareFeatureSupport,
    pub adapters: Vec<GpuAdapter>,
    pub preferred_gaming_gpu: Option<GpuAdapter>,
    pub confidence: HardwareConfidence,
    pub diagnostic: Option<String>,
    pub observed_at: u64,
}

static HARDWARE_CACHE: OnceLock<Mutex<Option<HardwareCapabilities>>> = OnceLock::new();

#[tauri::command]
pub fn get_hardware_capabilities(
    state: tauri::State<'_, crate::settings::DatabaseState>,
) -> HardwareCapabilities {
    let snapshot = cached_snapshot(false);
    state.log(
        "hardware",
        "hardware.gpu.preferred",
        &format!(
            "vendor={:?} model={} adapters={} preferred={}",
            snapshot.vendor,
            snapshot.model.as_deref().unwrap_or("unknown"),
            snapshot.adapters.len(),
            snapshot
                .preferred_gaming_gpu
                .as_ref()
                .map(|gpu| gpu.gpu_id.as_str())
                .unwrap_or("none")
        ),
    );
    state.log(
        "hardware",
        "hardware.feature.normalized",
        &format!(
            "dlss={:?} dlss_fg={:?} fsr={:?} xess={:?}",
            snapshot.feature_support.supports_dlss,
            snapshot.feature_support.supports_dlss_frame_generation,
            snapshot.feature_support.supports_fsr,
            snapshot.feature_support.supports_xess
        ),
    );
    snapshot
}

#[tauri::command]
pub fn refresh_hardware_capabilities(
    state: tauri::State<'_, crate::settings::DatabaseState>,
) -> HardwareCapabilities {
    let snapshot = cached_snapshot(true);
    state.log(
        "hardware",
        "hardware.gpu.enumerate",
        &format!(
            "adapters={} diagnostic={:?}",
            snapshot.adapters.len(),
            snapshot.diagnostic
        ),
    );
    snapshot
}

fn cached_snapshot(force_refresh: bool) -> HardwareCapabilities {
    let cache = HARDWARE_CACHE.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = cache.lock() {
        if !force_refresh {
            if let Some(snapshot) = guard.as_ref() {
                return snapshot.clone();
            }
        }
        let snapshot = detect_hardware();
        *guard = Some(snapshot.clone());
        return snapshot;
    }
    unknown_snapshot(Some("HARDWARE_CACHE_UNAVAILABLE".to_string()))
}

pub fn cached_for_launch() -> HardwareCapabilities {
    cached_snapshot(false)
}

#[cfg(windows)]
fn detect_hardware() -> HardwareCapabilities {
    windows_impl::detect_hardware()
}

#[cfg(not(windows))]
fn detect_hardware() -> HardwareCapabilities {
    unknown_snapshot(Some("HARDWARE_WINDOWS_ONLY".to_string()))
}

fn unknown_snapshot(diagnostic: Option<String>) -> HardwareCapabilities {
    HardwareCapabilities {
        diagnostic,
        observed_at: now_millis(),
        ..HardwareCapabilities::default()
    }
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(windows)]
mod windows_impl {
    use super::{
        now_millis, FeatureSupport, GpuAdapter, HardwareCapabilities, HardwareConfidence,
        HardwareFeatureSupport, HardwareVendor,
    };
    use windows::Win32::Graphics::Dxgi::{
        CreateDXGIFactory1, IDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE,
    };

    const NVIDIA_VENDOR_ID: u32 = 0x10DE;
    const AMD_VENDOR_ID: u32 = 0x1002;
    const INTEL_VENDOR_ID: u32 = 0x8086;

    pub fn detect_hardware() -> HardwareCapabilities {
        let factory = match unsafe { CreateDXGIFactory1::<IDXGIFactory1>() } {
            Ok(factory) => factory,
            Err(error) => {
                return super::unknown_snapshot(Some(format!("DXGI_FACTORY_FAILED:{error}")));
            }
        };
        let mut adapters = Vec::new();
        let mut index = 0u32;
        loop {
            let adapter = match unsafe { factory.EnumAdapters1(index) } {
                Ok(adapter) => adapter,
                Err(_) => break,
            };
            if let Ok(description) = unsafe { adapter.GetDesc1() } {
                adapters.push(normalize_adapter(&description));
            }
            index = index.saturating_add(1);
        }
        if adapters.is_empty() {
            return super::unknown_snapshot(Some("DXGI_NO_ADAPTERS".to_string()));
        }
        let preferred = select_preferred(&adapters).cloned();
        let Some(preferred_gpu) = preferred.clone() else {
            return HardwareCapabilities {
                adapters,
                diagnostic: Some("DXGI_ONLY_SOFTWARE_ADAPTERS".to_string()),
                observed_at: now_millis(),
                ..HardwareCapabilities::default()
            };
        };
        HardwareCapabilities {
            gpu_id: Some(preferred_gpu.gpu_id.clone()),
            vendor: preferred_gpu.vendor,
            model: Some(preferred_gpu.model.clone()),
            dedicated_vram_mb: preferred_gpu.dedicated_vram_mb,
            architecture: preferred_gpu.architecture.clone(),
            driver_version: preferred_gpu.driver_version.clone(),
            feature_support: preferred_gpu.feature_support.clone(),
            adapters,
            preferred_gaming_gpu: Some(preferred_gpu),
            confidence: HardwareConfidence::High,
            diagnostic: None,
            observed_at: now_millis(),
        }
    }

    fn normalize_adapter(
        description: &windows::Win32::Graphics::Dxgi::DXGI_ADAPTER_DESC1,
    ) -> GpuAdapter {
        let model = decode_description(&description.Description);
        let vendor = normalize_vendor(description.VendorId, &model);
        let architecture = classify_architecture(vendor, &model);
        let is_software = description.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0
            || model
                .to_ascii_lowercase()
                .contains("microsoft basic render");
        let feature_support = normalize_features(vendor, architecture.as_deref(), is_software);
        GpuAdapter {
            gpu_id: format!(
                "{:04X}:{:04X}:{}:{}",
                description.VendorId,
                description.DeviceId,
                description.AdapterLuid.HighPart,
                description.AdapterLuid.LowPart
            ),
            vendor,
            vendor_id: Some(description.VendorId),
            device_id: Some(description.DeviceId),
            model,
            dedicated_vram_mb: memory_mb(description.DedicatedVideoMemory),
            architecture,
            driver_version: None,
            luid: Some(format!(
                "{}:{}",
                description.AdapterLuid.HighPart, description.AdapterLuid.LowPart
            )),
            is_software,
            feature_support,
            confidence: if is_software {
                HardwareConfidence::High
            } else {
                HardwareConfidence::Medium
            },
        }
    }

    fn select_preferred(adapters: &[GpuAdapter]) -> Option<&GpuAdapter> {
        adapters
            .iter()
            .filter(|adapter| !adapter.is_software)
            .max_by(|left, right| {
                left.dedicated_vram_mb
                    .unwrap_or_default()
                    .cmp(&right.dedicated_vram_mb.unwrap_or_default())
                    .then_with(|| {
                        u8::from(left.vendor != HardwareVendor::Unknown)
                            .cmp(&u8::from(right.vendor != HardwareVendor::Unknown))
                    })
                    .then_with(|| left.gpu_id.cmp(&right.gpu_id))
            })
    }

    fn normalize_vendor(vendor_id: u32, model: &str) -> HardwareVendor {
        match vendor_id {
            NVIDIA_VENDOR_ID => HardwareVendor::Nvidia,
            AMD_VENDOR_ID => HardwareVendor::Amd,
            INTEL_VENDOR_ID => HardwareVendor::Intel,
            _ => {
                let normalized = model.to_ascii_lowercase();
                if normalized.contains("nvidia") {
                    HardwareVendor::Nvidia
                } else if normalized.contains("amd") || normalized.contains("radeon") {
                    HardwareVendor::Amd
                } else if normalized.contains("intel") {
                    HardwareVendor::Intel
                } else {
                    HardwareVendor::Other
                }
            }
        }
    }

    fn classify_architecture(vendor: HardwareVendor, model: &str) -> Option<String> {
        let normalized = model.to_ascii_uppercase();
        match vendor {
            HardwareVendor::Nvidia if normalized.contains("RTX 50") => Some("RTX 50".to_string()),
            HardwareVendor::Nvidia if normalized.contains("RTX 40") => Some("RTX 40".to_string()),
            HardwareVendor::Nvidia if normalized.contains("RTX 30") => Some("RTX 30".to_string()),
            HardwareVendor::Nvidia if normalized.contains("RTX 20") => Some("RTX 20".to_string()),
            HardwareVendor::Nvidia if normalized.contains("GTX") => Some("GTX".to_string()),
            HardwareVendor::Nvidia => None,
            HardwareVendor::Intel if normalized.contains("ARC") => Some("Arc".to_string()),
            HardwareVendor::Intel if normalized.contains("UHD") => {
                Some("Integrated UHD".to_string())
            }
            HardwareVendor::Intel if normalized.contains("IRIS") => {
                Some("Integrated Iris".to_string())
            }
            HardwareVendor::Amd if normalized.contains("RADEON") => Some("Radeon".to_string()),
            _ => None,
        }
    }

    fn normalize_features(
        vendor: HardwareVendor,
        architecture: Option<&str>,
        is_software: bool,
    ) -> HardwareFeatureSupport {
        if is_software {
            return HardwareFeatureSupport {
                supports_dlss: FeatureSupport::Unsupported,
                supports_dlss_frame_generation: FeatureSupport::Unsupported,
                supports_fsr: FeatureSupport::Unsupported,
                supports_fsr_frame_generation: FeatureSupport::Unsupported,
                supports_xess: FeatureSupport::Unsupported,
                supports_xess_frame_generation: FeatureSupport::Unsupported,
                preferred_xess: FeatureSupport::Unsupported,
                supports_tsr: FeatureSupport::Unsupported,
                supports_nis: FeatureSupport::Unsupported,
            };
        }
        let dlss = match architecture {
            Some("RTX 20" | "RTX 30" | "RTX 40" | "RTX 50") => FeatureSupport::Supported,
            Some("GTX") => FeatureSupport::Unsupported,
            _ if vendor == HardwareVendor::Nvidia => FeatureSupport::Unknown,
            _ => FeatureSupport::Unsupported,
        };
        let dlss_fg = match architecture {
            Some("RTX 40") => FeatureSupport::Supported,
            Some("RTX 20" | "RTX 30" | "GTX") => FeatureSupport::Unsupported,
            Some("RTX 50") => FeatureSupport::Unknown,
            _ => FeatureSupport::Unknown,
        };
        let xess = match (vendor, architecture) {
            (HardwareVendor::Intel, Some("Arc")) => FeatureSupport::Supported,
            (HardwareVendor::Intel, _) => FeatureSupport::Unknown,
            (HardwareVendor::Nvidia | HardwareVendor::Amd, _) => FeatureSupport::Unknown,
            _ => FeatureSupport::Unknown,
        };
        let preferred_xess = match (vendor, architecture) {
            (HardwareVendor::Intel, Some("Arc")) => FeatureSupport::Supported,
            (HardwareVendor::Intel, _) => FeatureSupport::Unknown,
            _ => FeatureSupport::Unsupported,
        };
        HardwareFeatureSupport {
            supports_dlss: dlss,
            supports_dlss_frame_generation: dlss_fg,
            supports_fsr: FeatureSupport::Supported,
            supports_fsr_frame_generation: FeatureSupport::Unknown,
            supports_xess: xess,
            supports_xess_frame_generation: FeatureSupport::Unknown,
            preferred_xess,
            supports_tsr: FeatureSupport::Supported,
            supports_nis: if vendor == HardwareVendor::Nvidia {
                FeatureSupport::Supported
            } else {
                FeatureSupport::Unsupported
            },
        }
    }

    fn memory_mb(bytes: usize) -> Option<u64> {
        u64::try_from(bytes)
            .ok()
            .map(|value| value / (1024 * 1024))
            .filter(|value| *value > 0)
    }

    fn decode_description(value: &[u16; 128]) -> String {
        let length = value
            .iter()
            .position(|character| *character == 0)
            .unwrap_or(value.len());
        String::from_utf16_lossy(&value[..length])
            .trim()
            .to_string()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn nvidia_family_rules_are_conservative_and_testable() {
            assert_eq!(
                normalize_features(HardwareVendor::Nvidia, Some("GTX"), false).supports_dlss,
                FeatureSupport::Unsupported
            );
            assert_eq!(
                normalize_features(HardwareVendor::Nvidia, Some("RTX 20"), false)
                    .supports_dlss_frame_generation,
                FeatureSupport::Unsupported
            );
            assert_eq!(
                normalize_features(HardwareVendor::Nvidia, Some("RTX 30"), false).supports_dlss,
                FeatureSupport::Supported
            );
            assert_eq!(
                normalize_features(HardwareVendor::Nvidia, Some("RTX 40"), false)
                    .supports_dlss_frame_generation,
                FeatureSupport::Supported
            );
            assert_eq!(
                normalize_features(HardwareVendor::Nvidia, Some("RTX 50"), false)
                    .supports_dlss_frame_generation,
                FeatureSupport::Unknown
            );
        }

        #[test]
        fn fsr_and_xess_are_not_reduced_to_vendor_only_claims() {
            assert_eq!(
                normalize_features(HardwareVendor::Amd, Some("Radeon"), false).supports_fsr,
                FeatureSupport::Supported
            );
            assert_eq!(
                normalize_features(HardwareVendor::Intel, Some("Arc"), false).preferred_xess,
                FeatureSupport::Supported
            );
            assert_eq!(
                normalize_features(HardwareVendor::Intel, Some("Integrated UHD"), false)
                    .preferred_xess,
                FeatureSupport::Unknown
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter(
        vendor: HardwareVendor,
        model: &str,
        vram: Option<u64>,
        software: bool,
    ) -> GpuAdapter {
        GpuAdapter {
            gpu_id: model.to_string(),
            vendor,
            model: model.to_string(),
            dedicated_vram_mb: vram,
            is_software: software,
            ..GpuAdapter::default()
        }
    }

    #[test]
    fn preferred_gpu_ignores_software_and_uses_vram_as_deterministic_fallback() {
        let adapters = vec![
            adapter(
                HardwareVendor::Other,
                "Microsoft Basic Render Driver",
                Some(16),
                true,
            ),
            adapter(HardwareVendor::Intel, "Intel UHD", Some(512), false),
            adapter(
                HardwareVendor::Nvidia,
                "NVIDIA RTX 4060",
                Some(8 * 1024),
                false,
            ),
        ];
        let preferred = adapters
            .iter()
            .filter(|adapter| !adapter.is_software)
            .max_by_key(|adapter| adapter.dedicated_vram_mb.unwrap_or_default())
            .expect("real adapter");
        assert_eq!(preferred.vendor, HardwareVendor::Nvidia);
        assert_eq!(preferred.model, "NVIDIA RTX 4060");
    }

    #[test]
    fn unknown_snapshot_is_non_fatal_and_does_not_claim_features() {
        let snapshot = unknown_snapshot(Some("test".to_string()));
        assert_eq!(snapshot.vendor, HardwareVendor::Unknown);
        assert_eq!(
            snapshot.feature_support.supports_dlss,
            FeatureSupport::Unknown
        );
        assert!(snapshot.diagnostic.is_some());
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "physical DXGI hardware QA"]
    fn real_hardware_qa_reports_all_adapters_and_preferred_gpu() {
        let snapshot = detect_hardware();
        println!(
            "REAL HARDWARE QA: {}",
            serde_json::to_string_pretty(&snapshot).expect("serialize hardware snapshot")
        );
        assert!(
            !snapshot.adapters.is_empty(),
            "DXGI must enumerate adapters"
        );
        assert!(
            snapshot.adapters.iter().any(|adapter| !adapter.is_software),
            "a real hardware adapter must be available"
        );
        assert!(snapshot.preferred_gaming_gpu.is_some());
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "physical DXGI plus graphics resolver QA"]
    fn real_hardware_resolves_marvel_upscaling_without_http() {
        use crate::game_capabilities::{
            GameCapabilityConfidence, GameCapabilityKind, GameCapabilitySource,
            GameCapabilityValue, ResolvedCapability, ResolvedGameCapabilities,
        };
        use crate::graphics_profile::{
            resolve, DisplayCapabilities, DisplayResolution, GraphicsProfileInput,
        };

        let hardware = detect_hardware();
        let capability = |kind, value, technologies: Vec<&str>| ResolvedCapability {
            kind,
            value,
            confidence: GameCapabilityConfidence::High,
            source: GameCapabilitySource::Pcgamingwiki,
            technologies: technologies.into_iter().map(str::to_string).collect(),
            alternative_available: GameCapabilityValue::Unknown,
            source_note: None,
            evidence: None,
            other_evidence: Vec::new(),
            resolved_at: 1,
            stale: false,
            has_conflict: false,
        };
        let game_capabilities = ResolvedGameCapabilities {
            game_id: "marvel-tokon".to_string(),
            native_hdr: capability(
                GameCapabilityKind::NativeHdr,
                GameCapabilityValue::No,
                vec![],
            ),
            high_fidelity_upscaling: capability(
                GameCapabilityKind::HighFidelityUpscaling,
                GameCapabilityValue::Yes,
                vec!["TSR", "DLSS 4", "NIS", "FSR 4", "XeSS 2"],
            ),
            frame_generation: capability(
                GameCapabilityKind::FrameGeneration,
                GameCapabilityValue::No,
                vec![],
            ),
            four_k: capability(
                GameCapabilityKind::FourK,
                GameCapabilityValue::Unknown,
                vec![],
            ),
            sixty_fps: capability(
                GameCapabilityKind::SixtyFps,
                GameCapabilityValue::Unknown,
                vec![],
            ),
            high_refresh_120_fps: capability(
                GameCapabilityKind::HighRefresh120Fps,
                GameCapabilityValue::Unknown,
                vec![],
            ),
            resolved_at: 1,
            provider_status: None,
            provider_error: None,
        };
        let result = resolve(&GraphicsProfileInput {
            game_id: "marvel-tokon".to_string(),
            game_capabilities,
            hardware,
            display: DisplayCapabilities {
                display_id: "qa-display".to_string(),
                current_resolution: Some(DisplayResolution {
                    width: 2560,
                    height: 1440,
                }),
                supported_resolutions: Vec::new(),
                current_refresh_rate: Some(60),
                supported_refresh_rates: vec![60],
                hdr_supported: Some(false),
                hdr_enabled: Some(false),
            },
        });
        println!(
            "REAL MARVEL RESOLVER QA: {}",
            serde_json::to_string_pretty(&result).expect("serialize recommendation")
        );
        assert_eq!(
            result
                .upscaling
                .technology
                .as_ref()
                .map(|technology| technology.label.as_str()),
            Some("DLSS 4")
        );
        assert_eq!(
            result.frame_generation.mode,
            crate::graphics_profile::FrameGenerationModeRecommendation::Off
        );
    }
}
