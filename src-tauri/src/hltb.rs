use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use std::{
    sync::{Mutex, OnceLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

const HLTB_BASE_URL: &str = "https://howlongtobeat.com";
const HLTB_SEARCH_URL: &str = "https://howlongtobeat.com/api/bleed";
const HLTB_INIT_URL: &str = "https://howlongtobeat.com/api/bleed/init";
const HLTB_USER_AGENT: &str = "Mozilla/5.0";

#[derive(Debug, Error)]
pub enum HltbError {
    #[error("HowLongToBeat is unreachable")]
    Offline,
    #[error("HowLongToBeat returned HTTP status {0}")]
    Api(u16),
    #[error("HowLongToBeat returned an invalid response")]
    InvalidResponse,
    #[error("HowLongToBeat request could not be created")]
    RequestSetup,
}

#[derive(Debug, Clone)]
pub struct HltbResult {
    pub hltb_id: String,
    pub matched_title: String,
    pub main_story_minutes: Option<i64>,
    pub main_extra_minutes: Option<i64>,
    pub completionist_minutes: Option<i64>,
    pub confidence: f64,
    pub match_type: &'static str,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HltbCandidate {
    pub hltb_id: String,
    pub title: String,
    pub main_story_minutes: Option<i64>,
    pub main_extra_minutes: Option<i64>,
    pub completionist_minutes: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    data: Vec<SearchEntry>,
}

#[derive(Debug, Deserialize, Clone)]
struct HltbToken {
    token: String,
    #[serde(rename = "hpKey")]
    hp_key: String,
    #[serde(rename = "hpVal")]
    hp_val: String,
    #[serde(skip)]
    fetched_at: Option<Instant>,
}

static TOKEN_CACHE: OnceLock<Mutex<Option<HltbToken>>> = OnceLock::new();

#[derive(Debug, Deserialize)]
struct SearchEntry {
    game_id: serde_json::Value,
    game_name: String,
    #[serde(default)]
    comp_main: serde_json::Value,
    #[serde(default)]
    comp_plus: serde_json::Value,
    #[serde(default)]
    comp_100: serde_json::Value,
}

pub async fn search_game(title: &str) -> Result<Vec<HltbCandidate>, HltbError> {
    let normalized_title = normalize_title(title);
    if normalized_title.is_empty() {
        return Ok(Vec::new());
    }
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(8))
        .timeout(Duration::from_secs(15))
        .user_agent(HLTB_USER_AGENT)
        .build()
        .map_err(|_| HltbError::RequestSetup)?;
    let token = get_token(&client).await?;
    let response = search_with_token(&client, &normalized_title, &token).await?;
    if response.status().as_u16() == 403 {
        clear_token();
        let refreshed = get_token(&client).await?;
        return parse_search_response(
            search_with_token(&client, &normalized_title, &refreshed).await?,
        )
        .await;
    }
    parse_search_response(response).await
}

async fn search_with_token(
    client: &Client,
    title: &str,
    token: &HltbToken,
) -> Result<reqwest::Response, HltbError> {
    let mut payload = Map::new();
    payload.insert("searchType".to_string(), json!("games"));
    payload.insert(
        "searchTerms".to_string(),
        json!(title.split_whitespace().collect::<Vec<_>>()),
    );
    payload.insert("searchPage".to_string(), json!(1));
    payload.insert("size".to_string(), json!(10));
    payload.insert(
        "searchOptions".to_string(),
        json!({
            "games": {
                "userId": 0, "platform": "", "sortCategory": "popular",
                "rangeCategory": "main", "rangeTime": {"min": null, "max": null},
                "gameplay": {"perspective": "", "flow": "", "genre": "", "difficulty": ""},
                "rangeYear": {"min": "", "max": ""}, "modifier": ""
            },
            "users": {"sortCategory": "postcount"},
            "lists": {"sortCategory": "follows"},
            "filter": "", "sort": 0, "randomizer": 0
        }),
    );
    payload.insert("useCache".to_string(), json!(true));
    payload.insert(token.hp_key.clone(), json!(token.hp_val));
    client
        .post(HLTB_SEARCH_URL)
        .header("Origin", HLTB_BASE_URL)
        .header("Referer", format!("{HLTB_BASE_URL}/"))
        .header("x-auth-token", &token.token)
        .header("x-hp-key", &token.hp_key)
        .header("x-hp-val", &token.hp_val)
        .json(&Value::Object(payload))
        .send()
        .await
        .map_err(|_| HltbError::Offline)
}

async fn parse_search_response(
    response: reqwest::Response,
) -> Result<Vec<HltbCandidate>, HltbError> {
    let status = response.status();
    if !status.is_success() {
        return Err(HltbError::Api(status.as_u16()));
    }
    let parsed = response
        .json::<SearchResponse>()
        .await
        .map_err(|_| HltbError::InvalidResponse)?;
    Ok(parsed
        .data
        .into_iter()
        .filter_map(|entry| {
            Some(HltbCandidate {
                hltb_id: value_as_string(&entry.game_id)?,
                title: entry.game_name,
                main_story_minutes: seconds_to_minutes(&entry.comp_main),
                main_extra_minutes: seconds_to_minutes(&entry.comp_plus),
                completionist_minutes: seconds_to_minutes(&entry.comp_100),
            })
        })
        .collect())
}

async fn get_token(client: &Client) -> Result<HltbToken, HltbError> {
    let cache = TOKEN_CACHE.get_or_init(|| Mutex::new(None));
    if let Some(token) = cache
        .lock()
        .map_err(|_| HltbError::RequestSetup)?
        .as_ref()
        .filter(|token| {
            token
                .fetched_at
                .is_some_and(|time| time.elapsed() < Duration::from_secs(600))
        })
        .cloned()
    {
        return Ok(token);
    }
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| HltbError::RequestSetup)?
        .as_millis();
    let response = client
        .get(format!("{HLTB_INIT_URL}?t={timestamp}"))
        .header("Origin", HLTB_BASE_URL)
        .header("Referer", format!("{HLTB_BASE_URL}/"))
        .send()
        .await
        .map_err(|_| HltbError::Offline)?;
    if !response.status().is_success() {
        return Err(HltbError::Api(response.status().as_u16()));
    }
    let mut token = response
        .json::<HltbToken>()
        .await
        .map_err(|_| HltbError::InvalidResponse)?;
    token.fetched_at = Some(Instant::now());
    *cache.lock().map_err(|_| HltbError::RequestSetup)? = Some(token.clone());
    Ok(token)
}

fn clear_token() {
    if let Some(cache) = TOKEN_CACHE.get() {
        if let Ok(mut value) = cache.lock() {
            *value = None;
        }
    }
}

pub fn choose_match(title: &str, candidates: &[HltbCandidate]) -> Option<HltbResult> {
    let query = normalize_title(title);
    if query.is_empty() || candidates.is_empty() {
        return None;
    }
    let mut scored = candidates
        .iter()
        .map(|candidate| {
            (
                score_title(&query, &normalize_title(&candidate.title)),
                candidate,
            )
        })
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| right.0.total_cmp(&left.0));
    let (confidence, candidate) = scored.first().copied()?;
    let second_confidence = scored.get(1).map(|entry| entry.0).unwrap_or(0.0);
    let exact = normalize_title(&candidate.title) == query;
    let safe = (exact && confidence >= 0.99)
        || (confidence >= 0.86 && confidence - second_confidence >= 0.08);
    if !safe {
        return None;
    }
    Some(HltbResult {
        hltb_id: candidate.hltb_id.clone(),
        matched_title: candidate.title.clone(),
        main_story_minutes: candidate.main_story_minutes,
        main_extra_minutes: candidate.main_extra_minutes,
        completionist_minutes: candidate.completionist_minutes,
        confidence,
        match_type: if exact { "exact" } else { "approximate" },
    })
}

pub fn normalize_title(value: &str) -> String {
    let folded = value
        .chars()
        .flat_map(|character| character.to_lowercase())
        .collect::<String>();
    let without_suffixes = [
        " deluxe edition",
        " ultimate edition",
        " complete edition",
        " definitive edition",
        " game of the year edition",
        " goty edition",
        " windows edition",
        " steam edition",
        " remastered",
        " remaster",
        " remake",
    ]
    .iter()
    .fold(folded, |current, suffix| current.replace(suffix, ""));
    without_suffixes
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || character.is_whitespace() {
                character
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn score_title(query: &str, candidate: &str) -> f64 {
    if query == candidate {
        return 1.0;
    }
    if candidate.starts_with(query) || query.starts_with(candidate) {
        return 0.92;
    }
    let query_tokens = query.split_whitespace().collect::<Vec<_>>();
    let candidate_tokens = candidate.split_whitespace().collect::<Vec<_>>();
    let overlap = query_tokens
        .iter()
        .filter(|token| candidate_tokens.contains(token))
        .count() as f64;
    let token_score = overlap / query_tokens.len().max(candidate_tokens.len()) as f64;
    let distance = levenshtein(query, candidate) as f64;
    let length = query.len().max(candidate.len()) as f64;
    (token_score * 0.65 + (1.0 - distance / length) * 0.35).clamp(0.0, 1.0)
}

fn levenshtein(left: &str, right: &str) -> usize {
    let mut previous = (0..=right.chars().count()).collect::<Vec<_>>();
    for (row, left_char) in left.chars().enumerate() {
        let mut current = vec![row + 1];
        for (column, right_char) in right.chars().enumerate() {
            current.push(if left_char == right_char {
                previous[column]
            } else {
                1 + previous[column]
                    .min(current[column])
                    .min(previous[column + 1])
            });
        }
        previous = current;
    }
    previous[right.chars().count()]
}

fn value_as_string(value: &serde_json::Value) -> Option<String> {
    value
        .as_i64()
        .map(|number| number.to_string())
        .or_else(|| value.as_str().map(str::to_string))
}

fn seconds_to_minutes(value: &serde_json::Value) -> Option<i64> {
    let seconds = value
        .as_i64()
        .or_else(|| value.as_str().and_then(|text| text.parse::<i64>().ok()))?;
    (seconds > 0).then_some((seconds as f64 / 60.0).round() as i64)
}

#[cfg(test)]
mod tests {
    use super::{choose_match, normalize_title, HltbCandidate};

    fn candidate(title: &str) -> HltbCandidate {
        HltbCandidate {
            hltb_id: "1".to_string(),
            title: title.to_string(),
            main_story_minutes: Some(120),
            main_extra_minutes: Some(180),
            completionist_minutes: Some(240),
        }
    }

    #[test]
    fn normalizes_special_editions_and_symbols() {
        assert_eq!(
            normalize_title("The Witcher 3™: Wild Hunt® - GOTY Edition"),
            "the witcher 3 wild hunt"
        );
    }

    #[test]
    fn accepts_exact_match() {
        let result = choose_match("Celeste", &[candidate("Celeste")]);
        assert_eq!(result.expect("exact result").match_type, "exact");
    }

    #[test]
    fn rejects_ambiguous_results() {
        let result = choose_match("Doom", &[candidate("Doom Eternal"), candidate("Doom II")]);
        assert!(result.is_none());
    }
}
