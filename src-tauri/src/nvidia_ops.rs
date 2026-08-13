use crate::settings::DatabaseState;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::State;

const RECOMMENDATIONS_DIR: &str = "Recommendations";
const APPLICATION_STORAGE: &str = "ApplicationStorage.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NvidiaOpsStatus {
    Available,
    BelowMinSpec,
    Unsupported,
    CacheMissing,
    NvidiaAppNotFound,
    GameNotFound,
    Ambiguous,
    ParseError,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NvidiaOpsRequest {
    pub game_id: String,
    pub steam_app_id: Option<i64>,
    pub executable_path: Option<String>,
    pub title: Option<String>,
    pub display_resolution: Option<DisplayResolution>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayResolution {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NvidiaOpsGame {
    pub steam_app_id: Option<i64>,
    pub short_name: String,
    pub cms_id: Option<i64>,
    pub executable: Option<String>,
    pub is_ops_supported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendedSetting {
    pub canonical_key: String,
    pub display_name: String,
    pub value: String,
    pub raw_key: String,
    pub raw_value: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NvidiaOpsConfidence {
    High,
    Medium,
    Low,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NvidiaRecommendedProfile {
    pub source: String,
    pub source_version: Option<String>,
    pub source_fingerprint: String,
    pub resolution: Option<DisplayResolution>,
    pub pop_index: u32,
    pub below_min_spec: bool,
    pub settings: Vec<RecommendedSetting>,
    pub confidence: NvidiaOpsConfidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NvidiaOpsResponse {
    pub status: NvidiaOpsStatus,
    pub game: Option<NvidiaOpsGame>,
    pub profile: Option<NvidiaRecommendedProfile>,
    pub diagnostic: Option<String>,
}

#[derive(Debug, Clone)]
struct CatalogApplication {
    game: NvidiaOpsGame,
    title: String,
    executable_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OpsTarget {
    resolution: Option<DisplayResolution>,
    pop_index: u32,
    below_min_spec: bool,
}

#[derive(Debug, Clone)]
struct RecommendationPackage {
    pops_path: PathBuf,
    pops: Value,
    targets: Vec<OpsTarget>,
    version: Option<String>,
}

#[tauri::command]
pub fn get_nvidia_ops_profile(
    state: State<'_, DatabaseState>,
    request: NvidiaOpsRequest,
) -> NvidiaOpsResponse {
    let response = resolve(&request);
    let identity = response
        .game
        .as_ref()
        .map(|game| {
            format!(
                "steam_app_id={:?} short_name={} pop_index={}",
                game.steam_app_id,
                game.short_name,
                response
                    .profile
                    .as_ref()
                    .map(|profile| profile.pop_index.to_string())
                    .unwrap_or_else(|| "none".to_string())
            )
        })
        .unwrap_or_else(|| "game=none".to_string());
    state.log(
        "nvidia-ops",
        status_event(response.status),
        &format!("game_id={} {identity}", request.game_id),
    );
    response
}

pub fn resolve(request: &NvidiaOpsRequest) -> NvidiaOpsResponse {
    if request.game_id.trim().is_empty() {
        return response(
            NvidiaOpsStatus::GameNotFound,
            None,
            None,
            Some("GAME_ID_EMPTY"),
        );
    }

    let Some(root) = nvidia_backend_root() else {
        return response(
            NvidiaOpsStatus::NvidiaAppNotFound,
            None,
            None,
            Some("NVIDIA_APP_BACKEND_NOT_FOUND"),
        );
    };
    resolve_at_root(request, &root)
}

fn resolve_at_root(request: &NvidiaOpsRequest, root: &Path) -> NvidiaOpsResponse {
    let storage_path = root.join(APPLICATION_STORAGE);
    let storage = match read_json(&storage_path) {
        Ok(value) => value,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return response(
                NvidiaOpsStatus::NvidiaAppNotFound,
                None,
                None,
                Some("APPLICATION_STORAGE_NOT_FOUND"),
            )
        }
        Err(_) => {
            return response(
                NvidiaOpsStatus::ParseError,
                None,
                None,
                Some("APPLICATION_STORAGE_READ_FAILED"),
            )
        }
    };
    let applications = catalog_applications(&storage);
    let matches = match_catalog(&applications, request);
    if matches.is_empty() {
        return response(
            NvidiaOpsStatus::GameNotFound,
            None,
            None,
            Some("GAME_NOT_FOUND"),
        );
    }
    if matches.len() > 1 {
        return response(
            NvidiaOpsStatus::Ambiguous,
            None,
            None,
            Some("GAME_MATCH_AMBIGUOUS"),
        );
    }

    let application = &matches[0];
    let game = application.game.clone();
    if !game.is_ops_supported {
        return response(
            NvidiaOpsStatus::Unsupported,
            Some(game),
            None,
            Some("OPS_UNSUPPORTED"),
        );
    }

    let recommendations_root = root.join(RECOMMENDATIONS_DIR).join(&game.short_name);
    let package = match find_recommendation_package(&recommendations_root) {
        Ok(Some(package)) => package,
        Ok(None) => {
            return response(
                NvidiaOpsStatus::CacheMissing,
                Some(game),
                None,
                Some("RECOMMENDATIONS_MISSING"),
            )
        }
        Err(_) => {
            return response(
                NvidiaOpsStatus::ParseError,
                Some(game),
                None,
                Some("RECOMMENDATIONS_PARSE_FAILED"),
            )
        }
    };
    let target = select_target(&package.targets, request.display_resolution.as_ref());
    let Some(target) = target else {
        return response(
            NvidiaOpsStatus::ParseError,
            Some(game),
            None,
            Some("RECOMMENDATION_TARGET_MISSING"),
        );
    };
    let settings = match normalize_settings(&package.pops, target.pop_index) {
        Some(settings) => settings,
        None => {
            return response(
                NvidiaOpsStatus::ParseError,
                Some(game),
                None,
                Some("POP_SETTINGS_MISSING"),
            )
        }
    };
    let fingerprint = match sha256_file(&package.pops_path) {
        Ok(value) => value,
        Err(_) => "unavailable".to_string(),
    };
    let profile = NvidiaRecommendedProfile {
        source: "NVIDIA_OPTIMAL_PLAYABLE_SETTINGS".to_string(),
        source_version: package.version,
        source_fingerprint: fingerprint,
        resolution: target.resolution,
        pop_index: target.pop_index,
        below_min_spec: target.below_min_spec,
        settings,
        confidence: NvidiaOpsConfidence::High,
    };
    let status = if profile.below_min_spec {
        NvidiaOpsStatus::BelowMinSpec
    } else {
        NvidiaOpsStatus::Available
    };
    response(status, Some(game), Some(profile), None)
}

fn response(
    status: NvidiaOpsStatus,
    game: Option<NvidiaOpsGame>,
    profile: Option<NvidiaRecommendedProfile>,
    diagnostic: Option<&str>,
) -> NvidiaOpsResponse {
    NvidiaOpsResponse {
        status,
        game,
        profile,
        diagnostic: diagnostic.map(str::to_string),
    }
}

fn status_event(status: NvidiaOpsStatus) -> &'static str {
    match status {
        NvidiaOpsStatus::Available => "NVIDIA_OPS_PROFILE_SELECTED",
        NvidiaOpsStatus::BelowMinSpec => "NVIDIA_OPS_BELOW_MIN_SPEC",
        NvidiaOpsStatus::Unsupported => "NVIDIA_OPS_UNSUPPORTED",
        NvidiaOpsStatus::CacheMissing => "NVIDIA_OPS_PACKAGE_MISSING",
        NvidiaOpsStatus::NvidiaAppNotFound => "NVIDIA_OPS_APP_NOT_FOUND",
        NvidiaOpsStatus::GameNotFound => "NVIDIA_OPS_GAME_NOT_FOUND",
        NvidiaOpsStatus::Ambiguous => "NVIDIA_OPS_GAME_AMBIGUOUS",
        NvidiaOpsStatus::ParseError => "NVIDIA_OPS_PARSE_ERROR",
    }
}

fn nvidia_backend_root() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|local_app_data| {
        PathBuf::from(local_app_data)
            .join("NVIDIA Corporation")
            .join("NVIDIA App")
            .join("NvBackend")
    })
}

fn read_json(path: &Path) -> std::io::Result<Value> {
    let contents = fs::read_to_string(path)?;
    serde_json::from_str(&contents)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string()))
}

fn catalog_applications(value: &Value) -> Vec<CatalogApplication> {
    let mut applications = Vec::new();
    collect_catalog_applications(value, &mut applications);
    applications
}

fn collect_catalog_applications(value: &Value, output: &mut Vec<CatalogApplication>) {
    match value {
        Value::Object(object) => {
            if let Some(application) = object.get("Application").and_then(Value::as_object) {
                if let Some(record) = parse_catalog_application(application) {
                    output.push(record);
                }
            }
            for child in object.values() {
                collect_catalog_applications(child, output);
            }
        }
        Value::Array(values) => {
            for child in values {
                collect_catalog_applications(child, output);
            }
        }
        _ => {}
    }
}

fn parse_catalog_application(application: &Map<String, Value>) -> Option<CatalogApplication> {
    let short_name = application.get("ShortName")?.as_str()?.trim();
    if short_name.is_empty() {
        return None;
    }
    let title = application
        .get("DisplayName")
        .and_then(Value::as_str)
        .unwrap_or(short_name)
        .to_string();
    let mut executable_paths = string_array(application, "DetectedFiles");
    executable_paths.extend(string_array(application, "ImageFiles"));
    if let Some(driver_profile) = application.get("DriverProfile").and_then(Value::as_str) {
        executable_paths.push(driver_profile.to_string());
    }
    let executable = executable_paths.first().cloned();
    Some(CatalogApplication {
        game: NvidiaOpsGame {
            steam_app_id: application
                .get("LaunchCmd")
                .and_then(Value::as_str)
                .and_then(extract_steam_app_id),
            short_name: short_name.to_string(),
            cms_id: application.get("CmsId").and_then(as_i64),
            executable,
            is_ops_supported: application
                .get("IsOpsSupported")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        },
        title,
        executable_paths,
    })
}

fn string_array(application: &Map<String, Value>, key: &str) -> Vec<String> {
    application
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

fn as_i64(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_str().and_then(|text| text.parse::<i64>().ok()))
}

fn extract_steam_app_id(launch_command: &str) -> Option<i64> {
    let marker = "steam://rungameid/";
    let start = launch_command.to_ascii_lowercase().find(marker)? + marker.len();
    let digits = launch_command[start..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect::<String>();
    (!digits.is_empty())
        .then(|| digits.parse::<i64>().ok())
        .flatten()
}

fn match_catalog<'a>(
    applications: &'a [CatalogApplication],
    request: &NvidiaOpsRequest,
) -> Vec<&'a CatalogApplication> {
    if let Some(steam_app_id) = request.steam_app_id {
        return applications
            .iter()
            .filter(|application| application.game.steam_app_id == Some(steam_app_id))
            .collect();
    }
    if let Some(executable_path) = request.executable_path.as_deref() {
        let normalized = normalize_path(executable_path);
        let matches = applications
            .iter()
            .filter(|application| {
                application
                    .executable_paths
                    .iter()
                    .any(|candidate| normalize_path(candidate) == normalized)
            })
            .collect::<Vec<_>>();
        if !matches.is_empty() {
            return matches;
        }
    }
    request.title.as_deref().map_or_else(Vec::new, |title| {
        let normalized = title.trim().to_ascii_lowercase();
        applications
            .iter()
            .filter(|application| application.title.trim().to_ascii_lowercase() == normalized)
            .collect()
    })
}

fn normalize_path(path: &str) -> String {
    path.replace('/', "\\")
        .trim_end_matches('\\')
        .to_ascii_lowercase()
}

fn find_recommendation_package(root: &Path) -> std::io::Result<Option<RecommendationPackage>> {
    if !root.is_dir() {
        return Ok(None);
    }
    let mut pops_paths = Vec::new();
    collect_pops_paths(root, &mut pops_paths)?;
    if pops_paths.is_empty() {
        return Ok(None);
    }
    for pops_path in pops_paths {
        let Ok(pops) = read_json(&pops_path) else {
            continue;
        };
        if !valid_pops_shape(&pops) {
            continue;
        }
        let profile_metadata_path = pops_path.parent().map(|path| path.join("metadata.json"));
        let metadata = profile_metadata_path
            .as_deref()
            .filter(|path| path.is_file())
            .and_then(|path| read_json(path).ok());
        let Some(targets) = metadata
            .as_ref()
            .map(find_ops_targets)
            .filter(|v| !v.is_empty())
        else {
            continue;
        };
        let version = metadata.as_ref().and_then(find_version);
        return Ok(Some(RecommendationPackage {
            pops_path,
            pops,
            targets,
            version,
        }));
    }
    Ok(None)
}

fn collect_pops_paths(root: &Path, output: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_pops_paths(&path, output)?;
        } else if file_type.is_file()
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case("pops.pub.tsv"))
        {
            output.push(path);
        }
    }
    Ok(())
}

fn valid_pops_shape(value: &Value) -> bool {
    value
        .get("settings")
        .and_then(Value::as_array)
        .is_some_and(|settings| !settings.is_empty())
        && value
            .get("pops")
            .and_then(Value::as_array)
            .is_some_and(|pops| !pops.is_empty())
}

fn find_ops_targets(value: &Value) -> Vec<OpsTarget> {
    let mut targets = Vec::new();
    collect_ops_targets(value, &mut targets);
    targets
}

fn collect_ops_targets(value: &Value, output: &mut Vec<OpsTarget>) {
    match value {
        Value::Object(object) => {
            if let Some(ops) = object.get("ops").and_then(Value::as_array) {
                for entry in ops {
                    let Some(entry) = entry.as_object() else {
                        continue;
                    };
                    let Some(pop_index) = entry.get("pops").and_then(as_i64) else {
                        continue;
                    };
                    if pop_index <= 0 {
                        continue;
                    }
                    output.push(OpsTarget {
                        resolution: entry.get("resolution").and_then(parse_resolution),
                        pop_index: pop_index as u32,
                        below_min_spec: entry
                            .get("belowMinSpec")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                    });
                }
            }
            for child in object.values() {
                collect_ops_targets(child, output);
            }
        }
        Value::Array(values) => {
            for child in values {
                collect_ops_targets(child, output);
            }
        }
        _ => {}
    }
}

fn find_version(value: &Value) -> Option<String> {
    match value {
        Value::Object(object) => {
            if let Some(version) = object.get("version").and_then(Value::as_str) {
                return Some(version.to_string());
            }
            object.values().find_map(find_version)
        }
        Value::Array(values) => values.iter().find_map(find_version),
        _ => None,
    }
}

fn parse_resolution(value: &Value) -> Option<DisplayResolution> {
    let text = value.as_str()?;
    let (width, height) = text.split_once('x')?;
    Some(DisplayResolution {
        width: width.parse().ok()?,
        height: height.parse().ok()?,
    })
}

fn select_target(targets: &[OpsTarget], display: Option<&DisplayResolution>) -> Option<OpsTarget> {
    if targets.is_empty() {
        return None;
    }
    let Some(display) = display else {
        return targets.first().cloned();
    };
    let exact = targets
        .iter()
        .find(|target| target.resolution.as_ref() == Some(display) && !target.below_min_spec);
    if let Some(exact) = exact {
        return Some(exact.clone());
    }
    let same_aspect = targets
        .iter()
        .filter(|target| {
            target
                .resolution
                .as_ref()
                .is_some_and(|resolution| same_aspect(resolution, display))
                && !target.below_min_spec
                && target
                    .resolution
                    .as_ref()
                    .is_some_and(|resolution| resolution.width <= display.width)
        })
        .max_by_key(|target| target.resolution.as_ref().map_or(0, |r| r.width));
    if let Some(target) = same_aspect {
        return Some(target.clone());
    }
    targets
        .iter()
        .find(|target| target.resolution.as_ref() == Some(display))
        .cloned()
        .or_else(|| targets.first().cloned())
}

fn same_aspect(left: &DisplayResolution, right: &DisplayResolution) -> bool {
    u64::from(left.width) * u64::from(right.height)
        == u64::from(right.width) * u64::from(left.height)
}

fn normalize_settings(pops: &Value, pop_index: u32) -> Option<Vec<RecommendedSetting>> {
    let settings = pops.get("settings")?.as_array()?;
    let pops = pops.get("pops")?.as_array()?;
    let pop = pops
        .get(usize::try_from(pop_index.checked_sub(1)?).ok()?)?
        .get("values")?
        .as_object()?;
    let mut normalized = Vec::new();
    for (index, setting) in settings.iter().enumerate() {
        let Some(setting_object) = setting.as_object() else {
            continue;
        };
        let Some(raw_key) = setting_object.keys().next() else {
            continue;
        };
        let Some(raw_value) = pop.get(&(index + 1).to_string()).and_then(Value::as_str) else {
            continue;
        };
        normalized.push(RecommendedSetting {
            canonical_key: canonical_key(raw_key),
            display_name: raw_key.clone(),
            value: raw_value.to_string(),
            raw_key: raw_key.clone(),
            raw_value: raw_value.to_string(),
        });
    }
    Some(normalized)
}

fn canonical_key(raw_key: &str) -> String {
    let normalized = raw_key.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "resolution" => "resolution",
        "display mode" => "displayMode",
        "nvidia rtx: dlss super resolution" => "dlssSuperResolution",
        "upscaling (super resolution tech)" => "upscalingTechnology",
        "fidelityfx super resolution 1" => "fsr1",
        "fidelityfx super resolution 3" => "fsr3",
        "xess" => "xess",
        "nvidia rtx: frame generation" => "dlssFrameGeneration",
        "fsr frame generation" => "fsrFrameGeneration",
        "nvidia rtx: dlss ray reconstruction" => "dlssRayReconstruction",
        "nvidia rtx: ray tracing" => "rayTracing",
        "nvidia rtx: path tracing" => "pathTracing",
        "nvidia reflex low latency" => "nvidiaReflexLowLatency",
        "texture quality" => "textureQuality",
        "texture filtering" | "texture filtering quality" => "textureFiltering",
        "shadow quality" => "shadowQuality",
        "shadow cache" => "shadowCache",
        "contact shadows" => "contactShadows",
        "global illumination quality" => "globalIlluminationQuality",
        "effects quality" => "effectsQuality",
        "mesh quality" => "meshQuality",
        "volumetric lighting" | "volumetric quality" => "volumetricLighting",
        "anti-aliasing" | "anti-aliasing quality" => "antiAliasing",
        "ambient occlusion" | "ssao" => "ambientOcclusion",
        "screen space reflections" | "reflection quality" => "screenSpaceReflections",
        "vsync" => "vsync",
        "dynamic resolution" => "dynamicResolution",
        _ => "unknown",
    }
    .to_string()
}

fn sha256_file(path: &Path) -> std::io::Result<String> {
    let bytes = fs::read(path)?;
    let digest = Sha256::digest(bytes);
    Ok(format!("{digest:x}"))
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_key, extract_steam_app_id, parse_resolution, resolve_at_root, same_aspect,
        NvidiaOpsRequest, NvidiaOpsStatus,
    };
    use std::fs;
    use std::path::PathBuf;

    fn fixture_root(name: &str) -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("lumadeck-nvidia-ops-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let recommendation = if name == "pragmata" {
            root.join("Recommendations/pragmata/fixture/regular_rtx")
        } else {
            root.join("Recommendations/elden_ring/fixture/regular")
        };
        fs::create_dir_all(&recommendation).expect("fixture directory");
        fs::write(
            root.join("ApplicationStorage.json"),
            include_str!("../test-fixtures/nvidia_ops/application-storage.json"),
        )
        .expect("catalog fixture");
        if name == "pragmata" {
            fs::write(
                recommendation.join("metadata.json"),
                include_str!("../test-fixtures/nvidia_ops/pragmata-metadata.json"),
            )
            .expect("pragmata metadata");
            fs::write(
                recommendation.join("pops.pub.tsv"),
                include_str!("../test-fixtures/nvidia_ops/pragmata-pops.pub.tsv"),
            )
            .expect("pragmata pops");
        } else {
            fs::write(
                recommendation.join("metadata.json"),
                include_str!("../test-fixtures/nvidia_ops/elden-ring-metadata.json"),
            )
            .expect("elden metadata");
            fs::write(
                recommendation.join("pops.pub.tsv"),
                include_str!("../test-fixtures/nvidia_ops/elden-ring-pops.pub.tsv"),
            )
            .expect("elden pops");
        }
        root
    }

    #[test]
    fn extracts_steam_app_id_from_launch_command() {
        assert_eq!(
            extract_steam_app_id("start steam://rungameid/1245620"),
            Some(1245620)
        );
    }

    #[test]
    fn parses_resolution_without_assuming_display_resolution_is_game_resolution() {
        assert_eq!(
            parse_resolution(&serde_json::json!("2560x1440")),
            Some(super::DisplayResolution {
                width: 2560,
                height: 1440
            })
        );
        assert!(same_aspect(
            &super::DisplayResolution {
                width: 2560,
                height: 1440
            },
            &super::DisplayResolution {
                width: 3840,
                height: 2160
            }
        ));
    }

    #[test]
    fn canonicalizes_known_settings_and_preserves_unknown() {
        assert_eq!(
            canonical_key("NVIDIA RTX: DLSS Super Resolution"),
            "dlssSuperResolution"
        );
        assert_eq!(canonical_key("Game-specific mystery"), "unknown");
    }

    #[test]
    fn selects_pragmata_pop_17_from_metadata_and_normalizes_frame_generation() {
        let root = fixture_root("pragmata");
        let result = resolve_at_root(
            &NvidiaOpsRequest {
                game_id: "steam-pragmata".to_string(),
                steam_app_id: Some(3357650),
                executable_path: None,
                title: None,
                display_resolution: Some(super::DisplayResolution {
                    width: 3840,
                    height: 2160,
                }),
            },
            &root,
        );
        assert_eq!(result.status, NvidiaOpsStatus::Available);
        let profile = result.profile.expect("profile");
        assert_eq!(profile.pop_index, 17);
        assert_eq!(
            profile.resolution,
            Some(super::DisplayResolution {
                width: 2560,
                height: 1440
            })
        );
        assert_eq!(profile.settings.len(), 5);
        assert!(profile.settings.iter().any(|setting| {
            setting.canonical_key == "dlssFrameGeneration" && setting.raw_value == "Auto"
        }));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn resolves_elden_ring_pop_1_even_when_below_min_spec() {
        let root = fixture_root("elden");
        let result = resolve_at_root(
            &NvidiaOpsRequest {
                game_id: "steam-elden".to_string(),
                steam_app_id: Some(1245620),
                executable_path: None,
                title: None,
                display_resolution: Some(super::DisplayResolution {
                    width: 3840,
                    height: 2160,
                }),
            },
            &root,
        );
        assert_eq!(result.status, NvidiaOpsStatus::BelowMinSpec);
        let profile = result.profile.expect("profile");
        assert_eq!(profile.pop_index, 1);
        assert!(profile.below_min_spec);
        assert_eq!(profile.settings[1].value, "Low");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn missing_recommendations_is_non_fatal_and_reports_cache_missing() {
        let root = std::env::temp_dir().join(format!(
            "lumadeck-nvidia-ops-missing-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("missing fixture directory");
        fs::write(
            root.join("ApplicationStorage.json"),
            include_str!("../test-fixtures/nvidia_ops/application-storage.json"),
        )
        .expect("catalog fixture");
        let result = resolve_at_root(
            &NvidiaOpsRequest {
                game_id: "missing-recommendations".to_string(),
                steam_app_id: Some(3357650),
                executable_path: None,
                title: None,
                display_resolution: None,
            },
            &root,
        );
        assert_eq!(result.status, NvidiaOpsStatus::CacheMissing);
        assert!(result.profile.is_none());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unsupported_catalog_entry_does_not_claim_ops() {
        let root = std::env::temp_dir().join(format!(
            "lumadeck-nvidia-ops-unsupported-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("unsupported fixture directory");
        fs::write(
            root.join("ApplicationStorage.json"),
            r#"{"applications":[{"Application":{"CmsId":42,"DisplayName":"Unsupported","ShortName":"unsupported","LaunchCmd":"start steam://rungameid/42","IsOpsSupported":false}}]}"#,
        )
        .expect("unsupported catalog fixture");
        let result = resolve_at_root(
            &NvidiaOpsRequest {
                game_id: "unsupported".to_string(),
                steam_app_id: Some(42),
                executable_path: None,
                title: None,
                display_resolution: None,
            },
            &root,
        );
        assert_eq!(result.status, NvidiaOpsStatus::Unsupported);
        assert!(result.profile.is_none());
        let _ = fs::remove_dir_all(root);
    }
}
