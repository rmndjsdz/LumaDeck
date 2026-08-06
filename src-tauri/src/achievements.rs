use futures_util::{stream, StreamExt};
use image::ImageFormat;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

const VERY_RARE_THRESHOLD: f64 = 5.0;
const RARE_THRESHOLD: f64 = 20.0;
const UNCOMMON_THRESHOLD: f64 = 50.0;
const COMMON_THRESHOLD: f64 = 80.0;
pub const DEFAULT_RECENT_LIMIT: usize = 10;
pub const ACHIEVEMENT_SYNC_FRESHNESS_SECONDS: i64 = 15 * 60;
const ICON_MAX_BYTES: usize = 4 * 1024 * 1024;
const ICON_DOWNLOAD_CONCURRENCY: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AchievementRarity {
    VeryRare,
    Rare,
    Uncommon,
    Common,
    VeryCommon,
}

pub fn rarity_from_str(value: &str) -> AchievementRarity {
    match value {
        "very-rare" => AchievementRarity::VeryRare,
        "rare" => AchievementRarity::Rare,
        "uncommon" => AchievementRarity::Uncommon,
        "very-common" => AchievementRarity::VeryCommon,
        _ => AchievementRarity::Common,
    }
}

impl AchievementRarity {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::VeryRare => "very-rare",
            Self::Rare => "rare",
            Self::Uncommon => "uncommon",
            Self::Common => "common",
            Self::VeryCommon => "very-common",
        }
    }

    pub fn virtual_tier(self) -> VirtualTier {
        match self {
            Self::VeryRare | Self::Rare => VirtualTier::Gold,
            Self::Uncommon => VirtualTier::Silver,
            Self::Common | Self::VeryCommon => VirtualTier::Bronze,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum VirtualTier {
    Bronze,
    Silver,
    Gold,
}

impl VirtualTier {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bronze => "bronze",
            Self::Silver => "silver",
            Self::Gold => "gold",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Achievement {
    pub api_name: String,
    pub display_name: String,
    pub description: String,
    pub hidden: bool,
    pub unlocked: bool,
    pub unlock_time: Option<String>,
    pub unlock_percentage: Option<f64>,
    pub rarity: AchievementRarity,
    pub virtual_tier: VirtualTier,
    pub icon_unlocked: Option<String>,
    pub icon_locked: Option<String>,
    pub local_icon_unlocked: Option<String>,
    pub local_icon_locked: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AchievementSummary {
    pub total: i64,
    pub unlocked: i64,
    pub locked: i64,
    pub completion_percentage: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AchievementDistribution {
    pub bronze: i64,
    pub silver: i64,
    pub gold: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AchievementUnlockedDistribution {
    pub bronze: i64,
    pub silver: i64,
    pub gold: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AchievementDistributions {
    pub total: AchievementDistribution,
    pub unlocked: AchievementUnlockedDistribution,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AchievementRecent {
    pub achievements: Vec<Achievement>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameAchievements {
    pub game_id: String,
    pub steam_app_id: i64,
    pub achievements: Vec<Achievement>,
    pub summary: AchievementSummary,
    pub distribution: AchievementDistribution,
    pub recent: AchievementRecent,
    pub total_distribution: AchievementDistribution,
    pub unlocked_distribution: AchievementUnlockedDistribution,
    pub last_synced_at: Option<String>,
    pub sync_status: String,
    pub schema_version: i64,
}

pub fn rarity_from_percentage(percentage: Option<f64>) -> AchievementRarity {
    let percentage = percentage.unwrap_or(COMMON_THRESHOLD);
    if percentage < VERY_RARE_THRESHOLD {
        AchievementRarity::VeryRare
    } else if percentage < RARE_THRESHOLD {
        AchievementRarity::Rare
    } else if percentage < UNCOMMON_THRESHOLD {
        AchievementRarity::Uncommon
    } else if percentage < COMMON_THRESHOLD {
        AchievementRarity::Common
    } else {
        AchievementRarity::VeryCommon
    }
}

pub fn summarize(achievements: &[Achievement]) -> AchievementSummary {
    let total = achievements.len() as i64;
    let unlocked = achievements
        .iter()
        .filter(|achievement| achievement.unlocked)
        .count() as i64;
    AchievementSummary {
        total,
        unlocked,
        locked: total.saturating_sub(unlocked),
        completion_percentage: if total == 0 {
            0.0
        } else {
            unlocked as f64 * 100.0 / total as f64
        },
    }
}

fn distribution_for<'a>(achievements: impl Iterator<Item = &'a Achievement>) -> (i64, i64, i64) {
    achievements.fold(
        (0, 0, 0),
        |(bronze, silver, gold), achievement| match achievement.rarity.virtual_tier() {
            VirtualTier::Bronze => (bronze + 1, silver, gold),
            VirtualTier::Silver => (bronze, silver + 1, gold),
            VirtualTier::Gold => (bronze, silver, gold + 1),
        },
    )
}

pub fn is_sync_fresh(last_synced_at: Option<&str>, now: i64) -> bool {
    last_synced_at
        .and_then(|value| value.parse::<i64>().ok())
        .map(|synced_at| now.saturating_sub(synced_at) < ACHIEVEMENT_SYNC_FRESHNESS_SECONDS)
        .unwrap_or(false)
}

pub fn needs_icon_source_refresh(achievements: &[Achievement]) -> bool {
    !achievements.is_empty()
        && achievements.iter().any(|achievement| {
            achievement.icon_unlocked.is_none() || achievement.icon_locked.is_none()
        })
}

pub fn distribute_total(achievements: &[Achievement]) -> AchievementDistribution {
    let (bronze, silver, gold) = distribution_for(achievements.iter());
    AchievementDistribution {
        bronze,
        silver,
        gold,
    }
}

pub fn distribute_unlocked(achievements: &[Achievement]) -> AchievementUnlockedDistribution {
    let (bronze, silver, gold) = distribution_for(
        achievements
            .iter()
            .filter(|achievement| achievement.unlocked),
    );
    AchievementUnlockedDistribution {
        bronze,
        silver,
        gold,
    }
}

pub fn recent(achievements: &[Achievement], limit: usize) -> AchievementRecent {
    let mut recent = achievements
        .iter()
        .filter(|achievement| achievement.unlocked && achievement.unlock_time.is_some())
        .cloned()
        .collect::<Vec<_>>();
    recent.sort_by(|left, right| {
        unlock_timestamp(right)
            .cmp(&unlock_timestamp(left))
            .then_with(|| left.api_name.cmp(&right.api_name))
    });
    recent.truncate(limit);
    AchievementRecent {
        achievements: recent,
    }
}

fn unlock_timestamp(achievement: &Achievement) -> i64 {
    achievement
        .unlock_time
        .as_deref()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or_default()
}

pub fn source_hash(achievements: &[Achievement], total: i64) -> String {
    let fingerprint = achievements
        .iter()
        .map(|achievement| {
            (
                &achievement.api_name,
                &achievement.display_name,
                &achievement.description,
                achievement.hidden,
                achievement.unlocked,
                &achievement.unlock_time,
                achievement.unlock_percentage,
                achievement.rarity,
                achievement.virtual_tier,
                &achievement.icon_unlocked,
                &achievement.icon_locked,
            )
        })
        .collect::<Vec<_>>();
    let encoded = serde_json::to_vec(&(total, fingerprint)).unwrap_or_default();
    Sha256::digest(encoded)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct IconCacheReport {
    pub downloaded_count: usize,
    pub reused_count: usize,
    pub skipped_count: usize,
    pub failed_count: usize,
    pub errors: Vec<String>,
}

#[derive(Debug)]
struct IconTask {
    achievement_index: usize,
    api_name: String,
    variant: &'static str,
    source_url: Option<String>,
}

#[derive(Debug)]
struct CachedIcon {
    achievement_index: usize,
    variant: &'static str,
    local_path: Option<String>,
    downloaded: bool,
    reused: bool,
    skipped: bool,
    error: Option<String>,
}

pub async fn cache_icons_with_report(
    root: &Path,
    app_id: i64,
    achievements: &mut [Achievement],
) -> IconCacheReport {
    let Some(client) = build_icon_client().ok().map(Arc::new) else {
        return IconCacheReport {
            failed_count: achievements.len().saturating_mul(2),
            errors: vec!["client-setup-failed".to_string()],
            ..IconCacheReport::default()
        };
    };
    let tasks = achievements
        .iter()
        .enumerate()
        .flat_map(|(achievement_index, achievement)| {
            [
                IconTask {
                    achievement_index,
                    api_name: achievement.api_name.clone(),
                    variant: "unlocked",
                    source_url: achievement.icon_unlocked.clone(),
                },
                IconTask {
                    achievement_index,
                    api_name: achievement.api_name.clone(),
                    variant: "locked",
                    source_url: achievement.icon_locked.clone(),
                },
            ]
        })
        .collect::<Vec<_>>();
    let mut report = IconCacheReport::default();
    let mut results = stream::iter(tasks.into_iter().map(|task| {
        let client = Arc::clone(&client);
        async move { cache_one_icon(client, root, app_id, task).await }
    }))
    .buffer_unordered(ICON_DOWNLOAD_CONCURRENCY);
    while let Some(result) = results.next().await {
        if let Some(path) = result.local_path {
            let achievement = &mut achievements[result.achievement_index];
            if result.variant == "unlocked" {
                achievement.local_icon_unlocked = Some(path);
            } else {
                achievement.local_icon_locked = Some(path);
            }
        }
        report.downloaded_count += usize::from(result.downloaded);
        report.reused_count += usize::from(result.reused);
        report.skipped_count += usize::from(result.skipped);
        if let Some(error) = result.error {
            report.failed_count += 1;
            report.errors.push(error);
        }
    }
    report
}

fn build_icon_client() -> Result<reqwest::Client, reqwest::Error> {
    build_icon_client_with_timeouts(Duration::from_secs(5), Duration::from_secs(15))
}

fn build_icon_client_with_timeouts(
    connect_timeout: Duration,
    request_timeout: Duration,
) -> Result<reqwest::Client, reqwest::Error> {
    reqwest::Client::builder()
        .connect_timeout(connect_timeout)
        .timeout(request_timeout)
        .user_agent("LumaDeck/0.1 Steam Achievements")
        .build()
}

async fn cache_one_icon(
    client: Arc<reqwest::Client>,
    root: &Path,
    app_id: i64,
    task: IconTask,
) -> CachedIcon {
    let (relative, target) = cache_path(root, app_id, &task.api_name, task.variant);
    if target.is_file() && is_valid_image(&target) {
        return CachedIcon {
            achievement_index: task.achievement_index,
            variant: task.variant,
            local_path: Some(relative.to_string_lossy().replace('\\', "/")),
            downloaded: false,
            reused: true,
            skipped: false,
            error: None,
        };
    }
    if target.is_file() {
        let _ = fs::remove_file(&target);
    }
    let (legacy_relative, legacy_target) =
        legacy_cache_path(root, app_id, &task.api_name, task.variant);
    if legacy_target.is_file() && is_valid_image(&legacy_target) {
        if let Some(parent) = target.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if fs::copy(&legacy_target, &target).is_ok() && is_valid_image(&target) {
            return CachedIcon {
                achievement_index: task.achievement_index,
                variant: task.variant,
                local_path: Some(relative.to_string_lossy().replace('\\', "/")),
                downloaded: false,
                reused: true,
                skipped: false,
                error: None,
            };
        }
        return CachedIcon {
            achievement_index: task.achievement_index,
            variant: task.variant,
            local_path: Some(legacy_relative.to_string_lossy().replace('\\', "/")),
            downloaded: false,
            reused: true,
            skipped: false,
            error: None,
        };
    };
    if legacy_target.is_file() {
        let _ = fs::remove_file(&legacy_target);
    }
    let Some(source_url) = task.source_url.clone().filter(|value| !value.is_empty()) else {
        return CachedIcon {
            achievement_index: task.achievement_index,
            variant: task.variant,
            local_path: None,
            downloaded: false,
            reused: false,
            skipped: true,
            error: None,
        };
    };
    let response = match client.get(source_url).send().await {
        Ok(response) if response.status().is_success() => response,
        Ok(response) => {
            return failed_icon(task, format!("http-status-{}", response.status().as_u16()))
        }
        Err(error) => {
            return failed_icon(
                task,
                if error.is_timeout() {
                    "timeout".to_string()
                } else {
                    "request-failed".to_string()
                },
            )
        }
    };
    if response
        .content_length()
        .is_some_and(|size| size > ICON_MAX_BYTES as u64)
    {
        return failed_icon(task, "response-too-large".to_string());
    }
    let mut bytes = Vec::new();
    let mut response = response;
    loop {
        let chunk = match response.chunk().await {
            Ok(Some(chunk)) => chunk,
            Ok(None) => break,
            Err(error) => {
                return failed_icon(
                    task,
                    if error.is_timeout() {
                        "timeout".to_string()
                    } else {
                        "read-failed".to_string()
                    },
                )
            }
        };
        if bytes.len().saturating_add(chunk.len()) > ICON_MAX_BYTES {
            return failed_icon(task, "response-too-large".to_string());
        }
        bytes.extend_from_slice(&chunk);
    }
    let encoded = match encode_icon(&bytes) {
        Ok(encoded) => encoded,
        Err(error) => return failed_icon(task, error.to_string()),
    };
    let Some(parent) = target.parent() else {
        return failed_icon(task, "cache-path-invalid".to_string());
    };
    if fs::create_dir_all(parent).is_err() {
        return failed_icon(task, "cache-directory-failed".to_string());
    }
    let temporary = target.with_extension(format!("webp.tmp-{}", std::process::id()));
    if fs::write(&temporary, encoded).is_err() {
        return failed_icon(task, "cache-write-failed".to_string());
    }
    if fs::rename(&temporary, &target).is_err() {
        let _ = fs::remove_file(&temporary);
        if target.is_file() && is_valid_image(&target) {
            return CachedIcon {
                achievement_index: task.achievement_index,
                variant: task.variant,
                local_path: Some(relative.to_string_lossy().replace('\\', "/")),
                downloaded: false,
                reused: true,
                skipped: false,
                error: None,
            };
        }
        return failed_icon(task, "cache-rename-failed".to_string());
    }
    CachedIcon {
        achievement_index: task.achievement_index,
        variant: task.variant,
        local_path: Some(relative.to_string_lossy().replace('\\', "/")),
        downloaded: true,
        reused: false,
        skipped: false,
        error: None,
    }
}

fn failed_icon(task: IconTask, reason: String) -> CachedIcon {
    CachedIcon {
        achievement_index: task.achievement_index,
        variant: task.variant,
        local_path: None,
        downloaded: false,
        reused: false,
        skipped: false,
        error: Some(format!("{}:{}:{}", task.api_name, task.variant, reason)),
    }
}

fn cache_path(root: &Path, app_id: i64, api_name: &str, variant: &str) -> (PathBuf, PathBuf) {
    let relative = PathBuf::from("cache")
        .join("steam-achievements")
        .join(app_id.to_string())
        .join(api_name_hash(api_name))
        .join(format!("{variant}.webp"));
    let target = root.join(&relative);
    (relative, target)
}

fn legacy_cache_path(
    root: &Path,
    app_id: i64,
    api_name: &str,
    variant: &str,
) -> (PathBuf, PathBuf) {
    let relative = PathBuf::from("cache")
        .join("steam-achievements")
        .join(app_id.to_string())
        .join(sanitize_component(api_name))
        .join(format!("{variant}.webp"));
    let target = root.join(&relative);
    (relative, target)
}

fn api_name_hash(api_name: &str) -> String {
    Sha256::digest(api_name.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn encode_icon(bytes: &[u8]) -> Result<Vec<u8>, &'static str> {
    if bytes.is_empty() || bytes.len() > ICON_MAX_BYTES {
        return Err("invalid-image-size");
    }
    let decoded = image::load_from_memory(bytes).map_err(|_| "invalid-image")?;
    let mut encoded = Cursor::new(Vec::new());
    decoded
        .write_to(&mut encoded, ImageFormat::WebP)
        .map_err(|_| "image-encode-failed")?;
    Ok(encoded.into_inner())
}

fn is_valid_image(path: &Path) -> bool {
    fs::read(path)
        .ok()
        .and_then(|bytes| image::load_from_memory(&bytes).ok())
        .is_some()
}

fn sanitize_component(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "unknown".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::{
        api_name_hash, build_icon_client, build_icon_client_with_timeouts, cache_one_icon,
        cache_path, distribute_total, distribute_unlocked, encode_icon, is_sync_fresh,
        needs_icon_source_refresh, rarity_from_percentage, recent, summarize, Achievement,
        AchievementRarity, IconTask, ICON_MAX_BYTES,
    };
    use std::{
        fs,
        io::{Read, Write},
        net::TcpListener,
        sync::Arc,
        thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    fn achievement(api_name: &str, unlocked: bool, rarity: AchievementRarity) -> Achievement {
        Achievement {
            api_name: api_name.to_string(),
            display_name: api_name.to_string(),
            description: String::new(),
            hidden: false,
            unlocked,
            unlock_time: None,
            unlock_percentage: None,
            rarity,
            virtual_tier: rarity.virtual_tier(),
            icon_unlocked: None,
            icon_locked: None,
            local_icon_unlocked: None,
            local_icon_locked: None,
        }
    }

    #[test]
    fn keeps_the_five_ludex_rarity_bands() {
        assert_eq!(
            rarity_from_percentage(Some(4.99)),
            AchievementRarity::VeryRare
        );
        assert_eq!(rarity_from_percentage(Some(5.0)), AchievementRarity::Rare);
        assert_eq!(rarity_from_percentage(Some(19.99)), AchievementRarity::Rare);
        assert_eq!(
            rarity_from_percentage(Some(20.0)),
            AchievementRarity::Uncommon
        );
        assert_eq!(
            rarity_from_percentage(Some(50.0)),
            AchievementRarity::Common
        );
        assert_eq!(
            rarity_from_percentage(Some(80.0)),
            AchievementRarity::VeryCommon
        );
    }

    #[test]
    fn derives_summary_and_virtual_distribution() {
        let values = vec![
            achievement("one", true, AchievementRarity::VeryRare),
            achievement("two", false, AchievementRarity::Uncommon),
            achievement("three", true, AchievementRarity::Common),
        ];
        assert_eq!(summarize(&values).unlocked, 2);
        assert_eq!(summarize(&values).locked, 1);
        assert_eq!(summarize(&values).completion_percentage, 200.0 / 3.0);
        assert_eq!(distribute_total(&values).gold, 1);
        assert_eq!(distribute_total(&values).silver, 1);
        assert_eq!(distribute_total(&values).bronze, 1);
        assert_eq!(distribute_unlocked(&values).gold, 1);
        assert_eq!(distribute_unlocked(&values).bronze, 1);
    }

    #[test]
    fn returns_recent_achievements_in_numeric_descending_order_with_limit() {
        let mut values = vec![
            achievement("older", true, AchievementRarity::Common),
            achievement("newer", true, AchievementRarity::Rare),
            achievement("middle", true, AchievementRarity::Uncommon),
        ];
        values[0].unlock_time = Some("9".to_string());
        values[1].unlock_time = Some("10".to_string());
        values[2].unlock_time = Some("8".to_string());
        let result = recent(&values, 2);
        assert_eq!(
            result
                .achievements
                .iter()
                .map(|value| value.api_name.as_str())
                .collect::<Vec<_>>(),
            vec!["newer", "older"]
        );
    }

    #[test]
    fn detects_fresh_and_stale_syncs() {
        assert!(is_sync_fresh(Some("1000"), 1000 + 60));
        assert!(!is_sync_fresh(Some("1000"), 1000 + 900));
        assert!(!is_sync_fresh(None, 1000));
    }

    #[test]
    fn detects_achievements_without_steam_icon_sources() {
        let mut values = vec![achievement("one", false, AchievementRarity::Common)];
        assert!(needs_icon_source_refresh(&values));
        values[0].icon_unlocked = Some("https://example.test/unlocked.png".to_string());
        values[0].icon_locked = Some("https://example.test/locked.png".to_string());
        assert!(!needs_icon_source_refresh(&values));
    }

    #[test]
    fn hashes_original_api_names_to_avoid_sanitized_collisions() {
        assert_ne!(api_name_hash("A/B"), api_name_hash("A\\B"));
        assert_eq!(api_name_hash("ACH_WIN"), api_name_hash("ACH_WIN"));
    }

    #[test]
    fn rejects_corrupt_and_oversized_icons_before_persistence() {
        assert_eq!(encode_icon(b"not-an-image"), Err("invalid-image"));
        assert_eq!(
            encode_icon(&vec![0_u8; ICON_MAX_BYTES + 1]),
            Err("invalid-image-size")
        );
    }

    #[test]
    fn downloader_reports_timeout_without_aborting_the_platform() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test listener");
        let address = listener.local_addr().expect("listener address");
        let server = thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let mut request = [0_u8; 512];
            let _ = stream.read(&mut request);
            let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 10\r\n\r\n");
            thread::sleep(Duration::from_millis(100));
        });
        let client = Arc::new(
            build_icon_client_with_timeouts(Duration::from_millis(50), Duration::from_millis(20))
                .expect("test client"),
        );
        let task = IconTask {
            achievement_index: 0,
            api_name: "TIMEOUT_TEST".to_string(),
            variant: "unlocked",
            source_url: Some(format!("http://{address}/icon")),
        };
        let result = tauri::async_runtime::block_on(cache_one_icon(
            client,
            &std::env::temp_dir(),
            3035570,
            task,
        ));
        server.join().expect("test server");
        assert!(result.error.is_some());
        assert!(
            result
                .error
                .as_deref()
                .is_some_and(|value| value.ends_with(":timeout")),
            "{:?}",
            result.error
        );
    }

    #[test]
    fn keeps_offline_icon_without_source_as_a_skipped_item() {
        let root = std::env::temp_dir().join(format!(
            "lumadeck-achievement-cache-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let (_, target) = cache_path(&root, 3035570, "OFFLINE", "locked");
        fs::create_dir_all(target.parent().expect("cache parent")).expect("cache directory");
        fs::write(&target, b"corrupt").expect("corrupt cache");
        let client = Arc::new(build_icon_client().expect("client"));
        let result = tauri::async_runtime::block_on(cache_one_icon(
            client,
            &root,
            3035570,
            IconTask {
                achievement_index: 0,
                api_name: "OFFLINE".to_string(),
                variant: "locked",
                source_url: None,
            },
        ));
        assert!(result.skipped);
        assert!(!target.exists());
        let _ = fs::remove_dir_all(root);
    }
}
