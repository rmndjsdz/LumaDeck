use image::ImageFormat;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};

const VERY_RARE_THRESHOLD: f64 = 5.0;
const RARE_THRESHOLD: f64 = 15.0;
const UNCOMMON_THRESHOLD: f64 = 30.0;
const COMMON_THRESHOLD: f64 = 60.0;

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

pub fn virtual_tier_from_str(value: &str) -> VirtualTier {
    match value {
        "silver" => VirtualTier::Silver,
        "gold" => VirtualTier::Gold,
        _ => VirtualTier::Bronze,
    }
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

pub fn distribute(achievements: &[Achievement]) -> AchievementDistribution {
    achievements.iter().fold(
        AchievementDistribution {
            bronze: 0,
            silver: 0,
            gold: 0,
        },
        |mut distribution, achievement| {
            match achievement.virtual_tier {
                VirtualTier::Bronze => distribution.bronze += 1,
                VirtualTier::Silver => distribution.silver += 1,
                VirtualTier::Gold => distribution.gold += 1,
            }
            distribution
        },
    )
}

pub fn recent(achievements: &[Achievement]) -> AchievementRecent {
    let mut recent = achievements
        .iter()
        .filter(|achievement| achievement.unlocked && achievement.unlock_time.is_some())
        .cloned()
        .collect::<Vec<_>>();
    recent.sort_by(|left, right| {
        right
            .unlock_time
            .cmp(&left.unlock_time)
            .then_with(|| left.api_name.cmp(&right.api_name))
    });
    AchievementRecent {
        achievements: recent,
    }
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

pub async fn cache_icons(root: &Path, app_id: i64, achievements: &mut [Achievement]) -> usize {
    let client = reqwest::Client::builder()
        .user_agent("LumaDeck/0.1 Steam Achievements")
        .build()
        .ok();
    let mut downloaded = 0;
    for achievement in achievements {
        downloaded += cache_one_icon(
            client.as_ref(),
            root,
            app_id,
            &achievement.api_name,
            "unlocked",
            achievement.icon_unlocked.as_deref(),
            &mut achievement.local_icon_unlocked,
        )
        .await;
        downloaded += cache_one_icon(
            client.as_ref(),
            root,
            app_id,
            &achievement.api_name,
            "locked",
            achievement.icon_locked.as_deref(),
            &mut achievement.local_icon_locked,
        )
        .await;
    }
    downloaded
}

async fn cache_one_icon(
    client: Option<&reqwest::Client>,
    root: &Path,
    app_id: i64,
    api_name: &str,
    variant: &str,
    source_url: Option<&str>,
    local_path: &mut Option<String>,
) -> usize {
    let relative = PathBuf::from("cache")
        .join("steam-achievements")
        .join(app_id.to_string())
        .join(sanitize_component(api_name))
        .join(format!("{variant}.webp"));
    let target = root.join(&relative);
    if target.is_file() && is_valid_image(&target) {
        *local_path = Some(relative.to_string_lossy().replace('\\', "/"));
        return 0;
    }
    if target.is_file() {
        let _ = fs::remove_file(&target);
    }
    let Some(client) = client else {
        return 0;
    };
    let Some(source_url) = source_url.filter(|value| !value.is_empty()) else {
        return 0;
    };
    let response = match client.get(source_url).send().await {
        Ok(response) if response.status().is_success() => response,
        _ => return 0,
    };
    let bytes = match response.bytes().await {
        Ok(bytes) => bytes,
        Err(_) => return 0,
    };
    let decoded = match image::load_from_memory(&bytes) {
        Ok(image) => image,
        Err(_) => return 0,
    };
    let mut encoded = Cursor::new(Vec::new());
    if decoded.write_to(&mut encoded, ImageFormat::WebP).is_err() {
        return 0;
    }
    let Some(parent) = target.parent() else {
        return 0;
    };
    if fs::create_dir_all(parent).is_err() {
        return 0;
    }
    let temporary = target.with_extension(format!("webp.tmp-{}", std::process::id()));
    if fs::write(&temporary, encoded.into_inner()).is_err() {
        return 0;
    }
    if fs::rename(&temporary, &target).is_err() {
        let _ = fs::remove_file(&temporary);
        if target.is_file() && is_valid_image(&target) {
            *local_path = Some(relative.to_string_lossy().replace('\\', "/"));
        }
        return 0;
    }
    *local_path = Some(relative.to_string_lossy().replace('\\', "/"));
    1
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
        distribute, rarity_from_percentage, summarize, Achievement, AchievementRarity, VirtualTier,
    };

    fn achievement(api_name: &str, unlocked: bool, tier: VirtualTier) -> Achievement {
        Achievement {
            api_name: api_name.to_string(),
            display_name: api_name.to_string(),
            description: String::new(),
            hidden: false,
            unlocked,
            unlock_time: None,
            unlock_percentage: None,
            rarity: AchievementRarity::Common,
            virtual_tier: tier,
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
        assert_eq!(rarity_from_percentage(Some(14.99)), AchievementRarity::Rare);
        assert_eq!(
            rarity_from_percentage(Some(15.0)),
            AchievementRarity::Uncommon
        );
        assert_eq!(
            rarity_from_percentage(Some(30.0)),
            AchievementRarity::Common
        );
        assert_eq!(
            rarity_from_percentage(Some(60.0)),
            AchievementRarity::VeryCommon
        );
    }

    #[test]
    fn derives_summary_and_virtual_distribution() {
        let values = vec![
            achievement("one", true, VirtualTier::Gold),
            achievement("two", false, VirtualTier::Silver),
            achievement("three", true, VirtualTier::Bronze),
        ];
        assert_eq!(summarize(&values).unlocked, 2);
        assert_eq!(summarize(&values).locked, 1);
        assert_eq!(summarize(&values).completion_percentage, 200.0 / 3.0);
        assert_eq!(distribute(&values).gold, 1);
        assert_eq!(distribute(&values).silver, 1);
        assert_eq!(distribute(&values).bronze, 1);
    }
}
