use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, time::Duration};

const STEAM_STORE_BASE_URL: &str = "https://store.steampowered.com";
const OPENCRITIC_API_BASE_URL: &str = "https://opencritic-api.p.rapidapi.com";
const OPENCRITIC_API_HOST: &str = "opencritic-api.p.rapidapi.com";
const METACRITIC_API_BASE_URL: &str = "https://unofficial-metacritic-api.p.rapidapi.com";
const METACRITIC_API_HOST: &str = "unofficial-metacritic-api.p.rapidapi.com";

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewsSourcesDto {
    pub game_id: String,
    pub title: String,
    pub steam_app_id: i64,
    pub metacritic: Option<MetacriticDto>,
    pub opencritic: Option<OpenCriticDto>,
    pub steam: Option<SteamReviewsDto>,
    pub errors: Vec<ReviewProviderErrorDto>,
    pub input_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetacriticDto {
    pub score: Option<i64>,
    pub platform: Option<String>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCriticDto {
    pub id: Option<i64>,
    pub name: Option<String>,
    pub score: Option<f64>,
    pub review_count: Option<i64>,
    pub percent_recommended: Option<f64>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamReviewsDto {
    pub all: Option<SteamReviewSnapshotDto>,
    pub recent: Option<SteamReviewSnapshotDto>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamReviewSnapshotDto {
    pub query_summary: Option<SteamReviewSummaryDto>,
    pub reviews: Vec<SteamReviewDto>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamReviewSummaryDto {
    pub total_reviews: Option<i64>,
    pub total_positive: Option<i64>,
    pub total_negative: Option<i64>,
    pub review_score: Option<i64>,
    pub review_score_desc: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SteamReviewDto {
    pub recommendation_id: Option<String>,
    pub author: Option<String>,
    pub review: Option<String>,
    pub voted_up: Option<bool>,
    pub playtime_forever_minutes: Option<i64>,
    pub timestamp_created: Option<i64>,
    pub language: Option<String>,
    pub votes_up: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewProviderErrorDto {
    pub provider: String,
    pub code: String,
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SteamAppDetailsEnvelope {
    success: bool,
    data: Option<SteamAppDetails>,
}

#[derive(Debug, Deserialize)]
struct SteamAppDetails {
    metacritic: Option<RawMetacritic>,
}

#[derive(Debug, Deserialize)]
struct RawMetacritic {
    score: Option<i64>,
    url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawSteamReviewResponse {
    success: Option<i64>,
    query_summary: Option<RawSteamReviewSummary>,
    reviews: Option<Vec<RawSteamReview>>,
}

#[derive(Debug, Deserialize)]
struct RawSteamReviewSummary {
    total_reviews: Option<i64>,
    total_positive: Option<i64>,
    total_negative: Option<i64>,
    review_score: Option<i64>,
    review_score_desc: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawSteamReview {
    recommendationid: Option<String>,
    author: Option<RawSteamAuthor>,
    review: Option<String>,
    voted_up: Option<bool>,
    timestamp_created: Option<i64>,
    language: Option<String>,
    votes_up: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct RawSteamAuthor {
    playtime_forever: Option<i64>,
}

#[derive(Debug)]
enum ProviderFailure {
    CredentialUnavailable,
    Network,
    Http(u16),
    InvalidResponse,
    NotFound,
}

type ReviewLogger<'a> = &'a (dyn Fn(&str, &str) + Send + Sync);

fn log_review(logger: ReviewLogger<'_>, checkpoint: &str, details: String) {
    logger(checkpoint, &details);
}

fn json_kind(value: &Value) -> &'static str {
    if value.is_object() {
        "object"
    } else if value.is_array() {
        "array"
    } else if value.is_null() {
        "null"
    } else {
        "scalar"
    }
}

fn json_text(value: &Value, max_length: usize) -> String {
    let text = value.to_string();
    if text.len() <= max_length {
        return text;
    }
    let truncated = text.chars().take(max_length).collect::<String>();
    format!("{truncated}...")
}

impl ProviderFailure {
    fn code(&self) -> &'static str {
        match self {
            Self::CredentialUnavailable => "credential-unavailable",
            Self::Network => "network",
            Self::Http(_) => "network",
            Self::InvalidResponse => "invalid-response",
            Self::NotFound => "not-found",
        }
    }

    fn message(&self) -> Option<String> {
        match self {
            Self::CredentialUnavailable => Some("RapidAPI credential is unavailable".to_string()),
            Self::Http(status) => Some(format!("HTTP status {status}")),
            Self::Network => Some("Provider is unreachable".to_string()),
            Self::InvalidResponse => Some("Provider returned an invalid response".to_string()),
            Self::NotFound => Some("No matching game was found".to_string()),
        }
    }
}

async fn request_json<T: for<'de> Deserialize<'de>>(
    request: reqwest::RequestBuilder,
) -> Result<T, ProviderFailure> {
    let response = request.send().await.map_err(|_| ProviderFailure::Network)?;
    if !response.status().is_success() {
        return Err(ProviderFailure::Http(response.status().as_u16()));
    }
    response
        .json::<T>()
        .await
        .map_err(|_| ProviderFailure::InvalidResponse)
}

fn rapid_api_request(
    client: &Client,
    url: String,
    api_key: &str,
    host: &str,
) -> reqwest::RequestBuilder {
    client
        .get(url)
        .header("Content-Type", "application/json")
        .header("X-RapidAPI-Key", api_key)
        .header("X-RapidAPI-Host", host)
}

fn slugify_title(value: &str) -> String {
    normalized_title(value)
        .chars()
        .filter_map(ascii_slug_character)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
}

fn ascii_slug_character(character: char) -> Option<char> {
    let folded = match character {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => 'c',
        'ď' | 'đ' => 'd',
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => 'g',
        'ĥ' | 'ħ' => 'h',
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => 'i',
        'ĵ' => 'j',
        'ķ' => 'k',
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => 'l',
        'ñ' | 'ń' | 'ņ' | 'ň' | 'ŉ' | 'ŋ' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => 'o',
        'ŕ' | 'ŗ' | 'ř' => 'r',
        'ś' | 'ŝ' | 'ş' | 'š' | 'ß' => 's',
        'ť' | 'ţ' | 'ŧ' => 't',
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => 'u',
        'ŵ' => 'w',
        'ý' | 'ÿ' | 'ŷ' => 'y',
        'ź' | 'ż' | 'ž' => 'z',
        character if character.is_ascii_alphanumeric() || character.is_whitespace() => character,
        _ => return None,
    };
    Some(folded)
}

fn metacritic_slug_candidates(title: &str) -> Vec<String> {
    let cleaned = title.trim();
    let mut candidates = vec![slugify_title(cleaned)];
    if normalized_title(cleaned) == "final fantasy xv windows edition" {
        candidates.insert(0, "final-fantasy-xv-royal-edition".to_string());
    }
    let without_edition = [
        " - Complete Edition",
        " Complete Edition",
        " - Definitive Edition",
        " Definitive Edition",
        " - Game of the Year Edition",
        " Game of the Year Edition",
        " Edition",
        " edition",
        " Remastered",
        " remastered",
    ]
    .iter()
    .find_map(|suffix| cleaned.strip_suffix(suffix))
    .unwrap_or(cleaned)
    .trim();
    if !without_edition.is_empty() && without_edition != cleaned {
        candidates.push(slugify_title(without_edition));
    }
    candidates.sort();
    candidates.dedup();
    candidates
}

fn parse_score_value(value: Option<&Value>) -> Option<i64> {
    let numeric = value
        .and_then(Value::as_f64)
        .filter(|score| score.is_finite() && (0.0..=100.0).contains(score))
        .map(|score| score.round() as i64);
    if numeric.is_some() {
        return numeric;
    }
    let text = value.and_then(Value::as_str)?.trim();
    let number = text
        .split(|character: char| !character.is_ascii_digit())
        .find(|part| !part.is_empty())?
        .parse::<i64>()
        .ok()?;
    (0..=100).contains(&number).then_some(number)
}

async fn fetch_metacritic(
    client: &Client,
    app_id: i64,
    title: &str,
    rapid_api_key: Option<&str>,
    logger: ReviewLogger<'_>,
) -> Result<Option<MetacriticDto>, ProviderFailure> {
    if let Some(api_key) = rapid_api_key.filter(|value| !value.trim().is_empty()) {
        for slug in metacritic_slug_candidates(title) {
            log_review(
                logger,
                "REVIEWS_METACRITIC_REQUEST",
                format!(
                    "app_id={app_id} title={title:?} slug={slug:?} route=games host={METACRITIC_API_HOST}"
                ),
            );
            let mut response = rapid_api_request(
                client,
                format!("{METACRITIC_API_BASE_URL}/api/v1/games/{slug}"),
                api_key,
                METACRITIC_API_HOST,
            )
            .send()
            .await
            .map_err(|_| ProviderFailure::Network)?;
            if response.status().as_u16() == 404 {
                log_review(
                    logger,
                    "REVIEWS_METACRITIC_RESPONSE",
                    format!(
                        "app_id={app_id} slug={slug:?} route=games status=404 payload=not-found"
                    ),
                );
                log_review(
                    logger,
                    "REVIEWS_METACRITIC_REQUEST",
                    format!(
                        "app_id={app_id} title={title:?} slug={slug:?} route=game host={METACRITIC_API_HOST}"
                    ),
                );
                response = rapid_api_request(
                    client,
                    format!("{METACRITIC_API_BASE_URL}/api/v1/game/{slug}"),
                    api_key,
                    METACRITIC_API_HOST,
                )
                .send()
                .await
                .map_err(|_| ProviderFailure::Network)?;
            }
            if response.status().as_u16() == 404 {
                log_review(
                    logger,
                    "REVIEWS_METACRITIC_RESPONSE",
                    format!(
                        "app_id={app_id} slug={slug:?} route=game status=404 payload=not-found"
                    ),
                );
                continue;
            }
            if !response.status().is_success() {
                log_review(
                    logger,
                    "REVIEWS_METACRITIC_RESPONSE",
                    format!(
                        "app_id={app_id} slug={slug:?} status={} payload=provider-error",
                        response.status().as_u16()
                    ),
                );
                return Err(ProviderFailure::Http(response.status().as_u16()));
            }
            let status = response.status().as_u16();
            let payload: Value = response
                .json()
                .await
                .map_err(|_| ProviderFailure::InvalidResponse)?;
            let data = payload
                .get("data")
                .filter(|value| value.is_object())
                .unwrap_or(&payload);
            let platforms = data
                .get("platforms")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let pc_platform = platforms.iter().find(|platform| {
                normalized_title(
                    platform
                        .get("platform")
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                ) == "pc"
            });
            let selected = pc_platform.or_else(|| {
                platforms.iter().find(|platform| {
                    platform
                        .get("critic_reviews")
                        .and_then(|reviews| parse_score_value(reviews.get("meta_score")))
                        .is_some()
                })
            });
            let score = selected
                .and_then(|platform| platform.get("critic_reviews"))
                .and_then(|reviews| parse_score_value(reviews.get("meta_score")))
                .or_else(|| parse_score_value(data.get("metaScore")))
                .or_else(|| parse_score_value(data.get("metascore")));
            let selected_platform = selected
                .and_then(|platform| platform.get("platform"))
                .and_then(Value::as_str)
                .map(ToString::to_string);
            let platform_names = platforms
                .iter()
                .filter_map(|platform| platform.get("platform").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join(",");
            log_review(
                logger,
                "REVIEWS_METACRITIC_RESPONSE",
                format!(
                    "app_id={app_id} title={title:?} slug={slug:?} status={status} payload_kind={} platform_count={} platforms={platform_names:?} selected_platform={selected_platform:?} score={score:?} fallback={} payload_preview={}",
                    json_kind(&payload),
                    platforms.len(),
                    pc_platform.is_none() && selected.is_some(),
                    json_text(&payload, 4000),
                ),
            );
            return Ok(Some(MetacriticDto {
                score,
                platform: selected_platform,
                url: data
                    .get("url")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
                    .or_else(|| Some(format!("https://www.metacritic.com/game/{slug}/"))),
            }));
        }
        return Ok(None);
    }

    let app_id_text = app_id.to_string();
    let responses: HashMap<String, SteamAppDetailsEnvelope> = request_json(
        client
            .get(format!("{STEAM_STORE_BASE_URL}/api/appdetails"))
            .query(&[
                ("appids", app_id_text.as_str()),
                ("l", "english"),
                ("cc", "us"),
            ]),
    )
    .await?;
    let payload = responses
        .get(&app_id_text)
        .filter(|response| response.success)
        .and_then(|response| response.data.as_ref())
        .and_then(|data| data.metacritic.as_ref());
    Ok(payload.map(|value| MetacriticDto {
        score: value.score,
        platform: None,
        url: value.url.clone(),
    }))
}

fn normalized_title(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .filter(|character| character.is_alphanumeric() || character.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn value_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter()
        .find_map(|key| value.get(*key)?.as_str().map(ToString::to_string))
}

fn value_i64(value: &Value, keys: &[&str]) -> Option<i64> {
    keys.iter().find_map(|key| {
        let value = value.get(*key)?;
        value
            .as_i64()
            .or_else(|| value.as_f64().map(|number| number.round() as i64))
            .or_else(|| value.as_str()?.trim().parse::<i64>().ok())
    })
}

fn value_f64(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter().find_map(|key| {
        let value = value.get(*key)?;
        value
            .as_f64()
            .or_else(|| value.as_str()?.trim().parse::<f64>().ok())
    })
}

fn value_score(value: &Value, keys: &[&str]) -> Option<f64> {
    keys.iter()
        .find_map(|key| parse_score_value(value.get(*key)).map(|score| score as f64))
}

pub fn is_usable_opencritic_cache(value: &OpenCriticDto) -> bool {
    value.score.is_some() || value.review_count.is_some_and(|count| count > 0)
}

async fn fetch_opencritic(
    client: &Client,
    title: &str,
    rapid_api_key: Option<&str>,
    logger: ReviewLogger<'_>,
) -> Result<Option<OpenCriticDto>, ProviderFailure> {
    if title.trim().is_empty() {
        return Err(ProviderFailure::NotFound);
    }
    let api_key = rapid_api_key
        .filter(|value| !value.trim().is_empty())
        .ok_or(ProviderFailure::CredentialUnavailable)?;
    log_review(
        logger,
        "REVIEWS_OPENCRITIC_REQUEST",
        format!("title={title:?} route=search criteria={title:?} host={OPENCRITIC_API_HOST}"),
    );
    let response = rapid_api_request(
        client,
        format!("{OPENCRITIC_API_BASE_URL}/game/search"),
        api_key,
        OPENCRITIC_API_HOST,
    )
    .query(&[("criteria", title)])
    .send()
    .await
    .map_err(|_| ProviderFailure::Network)?;
    let status = response.status().as_u16();
    if !response.status().is_success() {
        log_review(
            logger,
            "REVIEWS_OPENCRITIC_RESPONSE",
            format!("route=search status={status} payload=provider-error"),
        );
        return Err(ProviderFailure::Http(status));
    }
    let search: Value = response
        .json()
        .await
        .map_err(|_| ProviderFailure::InvalidResponse)?;
    let candidates = search
        .as_array()
        .or_else(|| search.get("data").and_then(Value::as_array))
        .or_else(|| search.get("results").and_then(Value::as_array))
        .or_else(|| search.get("games").and_then(Value::as_array))
        .ok_or(ProviderFailure::InvalidResponse)?;
    let candidate_names = candidates
        .iter()
        .filter_map(|item| value_string(item, &["name", "title", "gameName"]))
        .take(12)
        .collect::<Vec<_>>()
        .join("|");
    log_review(
        logger,
        "REVIEWS_OPENCRITIC_RESPONSE",
        format!(
            "route=search status={status} payload_kind={} candidate_count={} candidate_names={candidate_names:?} payload_preview={}",
            json_kind(&search),
            candidates.len(),
            json_text(&search, 4000),
        ),
    );
    let normalized = normalized_title(title);
    let candidate = candidates
        .iter()
        .find(|item| {
            value_string(item, &["name", "title", "gameName"])
                .map(|name| normalized_title(&name) == normalized)
                .unwrap_or(false)
        })
        .or_else(|| {
            candidates.iter().find(|item| {
                value_string(item, &["name", "title", "gameName"])
                    .map(|name| normalized_title(&name).contains(&normalized))
                    .unwrap_or(false)
            })
        })
        .or_else(|| candidates.first())
        .ok_or(ProviderFailure::NotFound)?;
    let id = value_i64(candidate, &["id", "gameId", "opencriticId"])
        .ok_or(ProviderFailure::InvalidResponse)?;
    let candidate_name = value_string(candidate, &["name", "title", "gameName"]);
    log_review(
        logger,
        "REVIEWS_OPENCRITIC_REQUEST",
        format!(
            "title={title:?} route=detail game_id={id} candidate_name={candidate_name:?} host={OPENCRITIC_API_HOST}"
        ),
    );
    let detail_response = rapid_api_request(
        client,
        format!("{OPENCRITIC_API_BASE_URL}/game/{id}"),
        api_key,
        OPENCRITIC_API_HOST,
    )
    .send()
    .await
    .map_err(|_| ProviderFailure::Network)?;
    let detail_status = detail_response.status().as_u16();
    if !detail_response.status().is_success() {
        log_review(
            logger,
            "REVIEWS_OPENCRITIC_RESPONSE",
            format!("route=detail game_id={id} status={detail_status} payload=provider-error"),
        );
        return Err(ProviderFailure::Http(detail_status));
    }
    let detail: Value = detail_response
        .json()
        .await
        .map_err(|_| ProviderFailure::InvalidResponse)?;
    let detail_data = detail
        .get("data")
        .filter(|value| value.is_object())
        .unwrap_or(&detail);
    let score = value_score(
        detail_data,
        &[
            "topCriticScore",
            "topCriticAverage",
            "averageScore",
            "score",
        ],
    )
    .or_else(|| value_score(candidate, &["topCriticScore", "topCriticAverage"]));
    let review_count = value_i64(detail_data, &["numReviews", "reviewCount", "totalReviews"])
        .or_else(|| value_i64(candidate, &["numReviews", "reviewCount"]));
    let percent_recommended = value_f64(detail_data, &["percentRecommended"])
        .or_else(|| value_f64(candidate, &["percentRecommended"]));
    log_review(
        logger,
        "REVIEWS_OPENCRITIC_RESPONSE",
        format!(
            "route=detail game_id={id} status={detail_status} payload_kind={} score={score:?} review_count={review_count:?} percent_recommended={percent_recommended:?} payload_preview={}",
            json_kind(&detail),
            json_text(&detail, 4000),
        ),
    );
    Ok(Some(OpenCriticDto {
        id: Some(id),
        name: value_string(detail_data, &["name", "title"])
            .or_else(|| value_string(candidate, &["name", "title", "gameName"])),
        score,
        review_count,
        percent_recommended,
        url: Some(format!("https://opencritic.com/game/{id}")),
    }))
}

fn map_steam_snapshot(
    value: RawSteamReviewResponse,
) -> Result<SteamReviewSnapshotDto, ProviderFailure> {
    if value.success != Some(1) {
        return Err(ProviderFailure::InvalidResponse);
    }
    Ok(SteamReviewSnapshotDto {
        query_summary: value.query_summary.map(|summary| SteamReviewSummaryDto {
            total_reviews: summary.total_reviews,
            total_positive: summary.total_positive,
            total_negative: summary.total_negative,
            review_score: summary.review_score,
            review_score_desc: summary.review_score_desc,
        }),
        reviews: value
            .reviews
            .unwrap_or_default()
            .into_iter()
            .map(|review| {
                let playtime_forever_minutes = review
                    .author
                    .as_ref()
                    .and_then(|author| author.playtime_forever);
                SteamReviewDto {
                    recommendation_id: review.recommendationid,
                    author: playtime_forever_minutes.map(|_| "Comunidad Steam".to_string()),
                    review: review.review,
                    voted_up: review.voted_up,
                    playtime_forever_minutes,
                    timestamp_created: review.timestamp_created,
                    language: review.language,
                    votes_up: review.votes_up,
                }
            })
            .collect(),
    })
}

fn is_supported_review_language(language: Option<&str>) -> bool {
    matches!(
        language
            .map(|value| value.trim().to_ascii_lowercase())
            .as_deref(),
        Some("english" | "en" | "spanish" | "es")
    )
}

fn select_featured_reviews(mut snapshot: SteamReviewSnapshotDto) -> SteamReviewSnapshotDto {
    snapshot
        .reviews
        .retain(|review| is_supported_review_language(review.language.as_deref()));
    snapshot.reviews.sort_by(|left, right| {
        right
            .votes_up
            .unwrap_or_default()
            .cmp(&left.votes_up.unwrap_or_default())
            .then_with(|| {
                right
                    .timestamp_created
                    .unwrap_or_default()
                    .cmp(&left.timestamp_created.unwrap_or_default())
            })
    });
    snapshot.reviews.truncate(6);
    snapshot
}

async fn fetch_steam(
    client: &Client,
    app_id: i64,
) -> (Option<SteamReviewsDto>, Vec<ReviewProviderErrorDto>) {
    let all_request = client
        .get(format!("{STEAM_STORE_BASE_URL}/appreviews/{app_id}"))
        .query(&[
            ("json", "1"),
            ("filter", "all"),
            ("language", "all"),
            ("purchase_type", "all"),
            ("num_per_page", "100"),
        ]);
    let recent_request = client
        .get(format!("{STEAM_STORE_BASE_URL}/appreviews/{app_id}"))
        .query(&[
            ("json", "1"),
            ("filter", "recent"),
            ("language", "all"),
            ("purchase_type", "all"),
            ("num_per_page", "10"),
        ]);
    let (all, recent) = futures_util::join!(
        request_json::<RawSteamReviewResponse>(all_request),
        request_json::<RawSteamReviewResponse>(recent_request),
    );
    let mut errors = Vec::new();
    let all = match all.and_then(map_steam_snapshot) {
        Ok(value) => Some(select_featured_reviews(value)),
        Err(error) => {
            errors.push(ReviewProviderErrorDto {
                provider: "steam".to_string(),
                code: error.code().to_string(),
                message: Some(format!(
                    "historical: {}",
                    error.message().unwrap_or_default()
                )),
            });
            None
        }
    };
    let recent = match recent.and_then(map_steam_snapshot) {
        Ok(value) => Some(value),
        Err(error) => {
            errors.push(ReviewProviderErrorDto {
                provider: "steam".to_string(),
                code: error.code().to_string(),
                message: Some(format!("recent: {}", error.message().unwrap_or_default())),
            });
            None
        }
    };
    (
        (all.is_some() || recent.is_some()).then_some(SteamReviewsDto { all, recent }),
        errors,
    )
}

pub async fn fetch_reviews(
    game_id: &str,
    title: &str,
    app_id: i64,
    rapid_api_key: Option<&str>,
    cached_metacritic: Option<MetacriticDto>,
    cached_opencritic: Option<OpenCriticDto>,
    cached_steam: Option<SteamReviewsDto>,
    steam_cache_fresh: bool,
    logger: ReviewLogger<'_>,
) -> Result<ReviewsSourcesDto, String> {
    if app_id <= 0 {
        return Err("GAME_IDENTIFIER_MISSING".to_string());
    }
    let client = Client::builder()
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(20))
        .user_agent("LumaDeck/ReviewsDomainV1")
        .build()
        .map_err(|_| "REVIEWS_CLIENT_SETUP_ERROR".to_string())?;
    let metacritic_request = async {
        if let Some(value) = cached_metacritic {
            log_review(
                logger,
                "REVIEWS_METACRITIC_CACHE_HIT",
                format!("app_id={app_id} policy=non-expiring"),
            );
            Ok(Some(value))
        } else {
            fetch_metacritic(&client, app_id, title, rapid_api_key, logger).await
        }
    };
    let opencritic_request = async {
        if let Some(value) = cached_opencritic {
            log_review(
                logger,
                "REVIEWS_OPENCRITIC_CACHE_HIT",
                format!("title={title:?} policy=non-expiring"),
            );
            Ok(Some(value))
        } else {
            fetch_opencritic(&client, title, rapid_api_key, logger).await
        }
    };
    let steam_request = async {
        if steam_cache_fresh {
            log_review(
                logger,
                "REVIEWS_STEAM_CACHE_HIT",
                format!("app_id={app_id} policy=daily"),
            );
            return (cached_steam.clone(), Vec::new());
        }
        let (fresh, errors) = fetch_steam(&client, app_id).await;
        if fresh.is_some() || cached_steam.is_none() {
            (fresh, errors)
        } else {
            log_review(
                logger,
                "REVIEWS_STEAM_CACHE_FALLBACK",
                format!("app_id={app_id} reason=refresh-failed"),
            );
            (cached_steam.clone(), errors)
        }
    };
    let (metacritic, opencritic, steam) =
        futures_util::join!(metacritic_request, opencritic_request, steam_request,);
    let mut errors = Vec::new();
    let metacritic = match metacritic {
        Ok(value) => value,
        Err(error) => {
            errors.push(ReviewProviderErrorDto {
                provider: "metacritic".to_string(),
                code: error.code().to_string(),
                message: error.message(),
            });
            None
        }
    };
    let opencritic = match opencritic {
        Ok(value) => value,
        Err(error) => {
            errors.push(ReviewProviderErrorDto {
                provider: "opencritic".to_string(),
                code: error.code().to_string(),
                message: error.message(),
            });
            None
        }
    };
    let (steam, steam_errors) = steam;
    errors.extend(steam_errors);
    Ok(ReviewsSourcesDto {
        game_id: game_id.to_string(),
        title: title.to_string(),
        steam_app_id: app_id,
        metacritic,
        opencritic,
        steam,
        errors,
        input_fingerprint: None,
    })
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        is_supported_review_language, is_usable_opencritic_cache, metacritic_slug_candidates,
        normalized_title, parse_score_value, select_featured_reviews, OpenCriticDto,
        SteamReviewDto, SteamReviewSnapshotDto,
    };

    #[test]
    fn normalizes_titles_for_opencritic_matching() {
        assert_eq!(
            normalized_title("The Witcher 3: Wild Hunt"),
            "the witcher 3 wild hunt"
        );
    }

    #[test]
    fn preserves_metacritic_alias_candidates_and_edition_fallbacks() {
        let candidates = metacritic_slug_candidates("Final Fantasy XV Windows Edition");
        assert!(candidates
            .iter()
            .any(|candidate| candidate == "final-fantasy-xv-royal-edition"));

        let candidates = metacritic_slug_candidates("The Witcher 3: Wild Hunt - Complete Edition");
        assert!(candidates
            .iter()
            .any(|candidate| candidate == "the-witcher-3-wild-hunt"));

        let candidates = metacritic_slug_candidates("MARVEL Tōkon: Fighting Souls");
        assert!(candidates
            .iter()
            .any(|candidate| candidate == "marvel-tokon-fighting-souls"));
    }

    #[test]
    fn rejects_empty_opencritic_cache_entries() {
        assert!(!is_usable_opencritic_cache(&OpenCriticDto {
            id: Some(20101),
            name: Some("Marvel Tokon: Fighting Souls".to_string()),
            score: None,
            review_count: Some(0),
            percent_recommended: Some(-1.0),
            url: Some("https://opencritic.com/game/20101".to_string()),
        }));
        assert!(is_usable_opencritic_cache(&OpenCriticDto {
            id: Some(20101),
            name: Some("Marvel Tokon: Fighting Souls".to_string()),
            score: Some(87.0),
            review_count: Some(20),
            percent_recommended: Some(90.0),
            url: Some("https://opencritic.com/game/20101".to_string()),
        }));
    }

    #[test]
    fn parses_metacritic_numeric_and_text_scores() {
        assert_eq!(parse_score_value(Some(&json!(84))), Some(84));
        assert_eq!(parse_score_value(Some(&json!("73/100"))), Some(73));
        assert_eq!(parse_score_value(Some(&json!("unknown"))), None);
        assert_eq!(parse_score_value(Some(&json!(101))), None);
    }

    #[test]
    fn selects_six_most_helpful_reviews_in_supported_languages() {
        let reviews = (0..8)
            .map(|index| SteamReviewDto {
                recommendation_id: Some(index.to_string()),
                author: None,
                review: Some("review".to_string()),
                voted_up: Some(true),
                playtime_forever_minutes: None,
                timestamp_created: Some(index),
                language: Some(if index == 7 {
                    "french".to_string()
                } else if index % 2 == 0 {
                    "english".to_string()
                } else {
                    "spanish".to_string()
                }),
                votes_up: Some(index),
            })
            .collect();
        let selected = select_featured_reviews(SteamReviewSnapshotDto {
            query_summary: None,
            reviews,
        });
        assert_eq!(selected.reviews.len(), 6);
        assert!(selected
            .reviews
            .iter()
            .all(|review| is_supported_review_language(review.language.as_deref())));
        assert_eq!(selected.reviews[0].votes_up, Some(6));
    }
}
