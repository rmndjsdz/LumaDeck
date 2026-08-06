use crate::{
    ai::{self, AIRequestError, StructuredTextRequest},
    reviews::{ReviewsSourcesDto, SteamReviewDto},
    settings::DatabaseState,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

pub const PROMPT_VERSION: i64 = 1;
const MAX_STEAM_SAMPLES: usize = 8;
const MIN_REVIEW_TEXT_LENGTH: usize = 24;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConsensusSources {
    pub metacritic_included: bool,
    pub opencritic_included: bool,
    pub steam_included: bool,
    pub critic_review_count: Option<i64>,
    pub player_review_count: Option<i64>,
    pub sampled_steam_reviews: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GameReviewConsensus {
    pub game_id: String,
    pub overall_rating: Option<f64>,
    pub agreement: String,
    pub agreement_label: String,
    pub strengths: Vec<String>,
    pub weaknesses: Vec<String>,
    pub conclusion: String,
    pub sources: ConsensusSources,
    pub generated_at: String,
    pub prompt_version: i64,
    pub provider_id: String,
    pub model_id: Option<String>,
    pub input_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConsensusInput {
    pub game_id: String,
    pub title: String,
    pub metacritic: Option<ConsensusMetacritic>,
    pub opencritic: Option<ConsensusOpenCritic>,
    pub steam: Option<ConsensusSteam>,
    pub sampled_reviews: Vec<ConsensusReview>,
    #[serde(skip_serializing)]
    pub fingerprint: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConsensusMetacritic {
    pub score: i64,
    pub max_score: i64,
    pub platform: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConsensusOpenCritic {
    pub score: Option<f64>,
    pub review_count: Option<i64>,
    pub percent_recommended: Option<f64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConsensusSteam {
    pub historical_positive_percent: Option<f64>,
    pub historical_total: Option<i64>,
    pub recent_positive_percent: Option<f64>,
    pub recent_total: Option<i64>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConsensusReview {
    pub id: String,
    pub recommended: bool,
    pub text: String,
    pub playtime_hours: Option<f64>,
    pub helpful_votes: Option<i64>,
    pub language: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelConsensusContent {
    overall_rating: Option<f64>,
    agreement: String,
    agreement_label: String,
    strengths: Vec<String>,
    weaknesses: Vec<String>,
    conclusion: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsensusError {
    InsufficientData,
    InvalidModelResponse,
    AI(AIRequestError),
}

impl ConsensusError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InsufficientData => "CONSENSUS_INSUFFICIENT_DATA",
            Self::InvalidModelResponse => "CONSENSUS_INVALID_RESPONSE",
            Self::AI(error) => error.code(),
        }
    }
}

pub fn build_input(game_id: &str, sources: &ReviewsSourcesDto) -> ConsensusInput {
    let metacritic = sources.metacritic.as_ref().and_then(|source| {
        source.score.map(|score| ConsensusMetacritic {
            score,
            max_score: 100,
            platform: source.platform.clone(),
        })
    });
    let opencritic = sources
        .opencritic
        .as_ref()
        .map(|source| ConsensusOpenCritic {
            score: source.score,
            review_count: source.review_count,
            percent_recommended: source.percent_recommended,
        });
    let steam = sources.steam.as_ref().map(|source| ConsensusSteam {
        historical_positive_percent: source
            .all
            .as_ref()
            .and_then(|value| positive_percent(value.query_summary.as_ref())),
        historical_total: source
            .all
            .as_ref()
            .and_then(|value| value.query_summary.as_ref()?.total_reviews),
        recent_positive_percent: source
            .recent
            .as_ref()
            .and_then(|value| positive_percent(value.query_summary.as_ref())),
        recent_total: source
            .recent
            .as_ref()
            .and_then(|value| value.query_summary.as_ref()?.total_reviews),
    });
    let sampled_reviews = sample_reviews(sources);
    let mut input = ConsensusInput {
        game_id: game_id.to_string(),
        title: sources.title.clone(),
        metacritic,
        opencritic,
        steam,
        sampled_reviews,
        fingerprint: String::new(),
    };
    input.fingerprint = fingerprint(&input);
    input
}

pub fn has_minimum_data(input: &ConsensusInput) -> bool {
    let critic_sources = usize::from(input.metacritic.is_some())
        + usize::from(
            input
                .opencritic
                .as_ref()
                .is_some_and(|value| value.score.is_some() || value.review_count.is_some()),
        );
    let has_positive = input
        .sampled_reviews
        .iter()
        .any(|review| review.recommended);
    let has_negative = input
        .sampled_reviews
        .iter()
        .any(|review| !review.recommended);
    let diverse_steam = input.sampled_reviews.len() >= 3 && has_positive && has_negative;
    let enough_steam = input
        .steam
        .as_ref()
        .and_then(|value| value.historical_total)
        .is_some_and(|total| total >= 20)
        && input.sampled_reviews.len() >= 4
        && has_positive
        && has_negative;
    critic_sources >= 2 || (critic_sources >= 1 && diverse_steam) || enough_steam
}

pub fn build_prompt(input: &ConsensusInput) -> StructuredTextRequest {
    StructuredTextRequest {
        system_prompt: "Analiza exclusivamente la información proporcionada. No utilices conocimiento externo ni inventes características del juego. No atribuyas temas a críticos o jugadores si no aparecen en las fuentes. Identifica fortalezas y debilidades solo cuando estén apoyadas por evidencia repetida. Distingue entre alto acuerdo, acuerdo moderado, opiniones divididas, recepción polarizada y datos insuficientes. Devuelve JSON válido y nada más, sin Markdown ni emojis.".to_string(),
        user_prompt: format!(
            "Genera un consenso breve en español con este esquema JSON: overallRating (número 0..5 o null), agreement (high|moderate|divided|polarized|insufficient_data), agreementLabel, strengths (máximo 4 frases breves), weaknesses (máximo 3 frases breves), conclusion (45-80 palabras). No presentes como mayoritaria una opinión aislada y no copies fragmentos extensos. Datos disponibles:\n{}",
            serde_json::to_string_pretty(input).unwrap_or_else(|_| "{}".to_string())
        ),
    }
}

pub fn parse_model_response(
    raw: &str,
) -> Result<ModelConsensusContentForValidation, ConsensusError> {
    let json_text = extract_json(raw).ok_or(ConsensusError::InvalidModelResponse)?;
    let parsed = serde_json::from_str::<ModelConsensusContent>(&json_text)
        .map_err(|_| ConsensusError::InvalidModelResponse)?;
    let overall_rating = parsed.overall_rating.map(|value| {
        if value.is_finite() && (0.0..=5.0).contains(&value) {
            Some(value)
        } else {
            None
        }
    });
    if parsed.overall_rating.is_some() && overall_rating.flatten().is_none() {
        return Err(ConsensusError::InvalidModelResponse);
    }
    let agreement = parsed.agreement.trim().to_string();
    if !matches!(
        agreement.as_str(),
        "high" | "moderate" | "divided" | "polarized" | "insufficient_data"
    ) {
        return Err(ConsensusError::InvalidModelResponse);
    }
    let agreement_label =
        normalize_text(&parsed.agreement_label, 120).ok_or(ConsensusError::InvalidModelResponse)?;
    let strengths = normalize_list(parsed.strengths, 4)?;
    let weaknesses = normalize_list(parsed.weaknesses, 3)?;
    let conclusion =
        normalize_text(&parsed.conclusion, 700).ok_or(ConsensusError::InvalidModelResponse)?;
    Ok(ModelConsensusContentForValidation {
        overall_rating: overall_rating.flatten(),
        agreement,
        agreement_label,
        strengths,
        weaknesses,
        conclusion,
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelConsensusContentForValidation {
    pub overall_rating: Option<f64>,
    pub agreement: String,
    pub agreement_label: String,
    pub strengths: Vec<String>,
    pub weaknesses: Vec<String>,
    pub conclusion: String,
}

pub async fn generate(
    state: &DatabaseState,
    input: &ConsensusInput,
) -> Result<GameReviewConsensus, ConsensusError> {
    if !has_minimum_data(input) {
        return Err(ConsensusError::InsufficientData);
    }
    let response = ai::generate_structured_text_from_settings(state, build_prompt(input))
        .await
        .map_err(ConsensusError::AI)?;
    let content = match parse_model_response(&response.content) {
        Ok(content) => content,
        Err(error) => {
            state.log(
                "ai-generation",
                "CONSENSUS_RESPONSE_INVALID",
                &describe_model_response(&response.content),
            );
            return Err(error);
        }
    };
    let configuration = crate::settings::get_ai_configuration(state)
        .map_err(|_| ConsensusError::AI(AIRequestError::NotConfigured))?;
    Ok(GameReviewConsensus {
        game_id: input.game_id.clone(),
        overall_rating: content.overall_rating,
        agreement: content.agreement,
        agreement_label: content.agreement_label,
        strengths: content.strengths,
        weaknesses: content.weaknesses,
        conclusion: content.conclusion,
        sources: source_counts(input),
        generated_at: chrono::Utc::now().to_rfc3339(),
        prompt_version: PROMPT_VERSION,
        provider_id: configuration.configuration.provider_id,
        model_id: Some(configuration.configuration.model),
        input_fingerprint: input.fingerprint.clone(),
    })
}

pub fn source_counts(input: &ConsensusInput) -> ConsensusSources {
    ConsensusSources {
        metacritic_included: input.metacritic.is_some(),
        opencritic_included: input.opencritic.is_some(),
        steam_included: input.steam.is_some(),
        critic_review_count: input
            .opencritic
            .as_ref()
            .and_then(|value| value.review_count),
        player_review_count: input
            .steam
            .as_ref()
            .and_then(|value| value.historical_total),
        sampled_steam_reviews: input.sampled_reviews.len(),
    }
}

fn positive_percent(summary: Option<&crate::reviews::SteamReviewSummaryDto>) -> Option<f64> {
    let summary = summary?;
    let total = summary.total_reviews?;
    let positive = summary.total_positive?;
    if total <= 0 {
        return None;
    }
    Some((positive as f64 / total as f64) * 100.0)
}

fn sample_reviews(sources: &ReviewsSourcesDto) -> Vec<ConsensusReview> {
    let mut candidates = Vec::new();
    if let Some(steam) = &sources.steam {
        if let Some(all) = &steam.all {
            add_candidates(&mut candidates, &all.reviews, false);
        }
        if let Some(recent) = &steam.recent {
            add_candidates(&mut candidates, &recent.reviews, true);
        }
    }
    candidates.sort_by(|left, right| candidate_order(left, right));
    let mut selected = Vec::new();
    let mut selected_ids = HashSet::new();
    for recommended in [true, false] {
        for candidate in candidates
            .iter()
            .filter(|candidate| candidate.recommended == recommended)
        {
            if selected.len() >= MAX_STEAM_SAMPLES || selected_ids.contains(&candidate.id) {
                continue;
            }
            selected_ids.insert(candidate.id.clone());
            selected.push(candidate.review.clone());
            if selected
                .iter()
                .filter(|value: &&ConsensusReview| value.recommended == recommended)
                .count()
                >= 3
            {
                break;
            }
        }
    }
    for candidate in candidates.iter().filter(|candidate| candidate.recent) {
        if selected.len() >= MAX_STEAM_SAMPLES || selected_ids.contains(&candidate.id) {
            continue;
        }
        selected_ids.insert(candidate.id.clone());
        selected.push(candidate.review.clone());
        if selected.len() >= 8 {
            break;
        }
    }
    selected
}

#[derive(Clone)]
struct SampleCandidate {
    id: String,
    recommended: bool,
    recent: bool,
    votes: i64,
    timestamp: i64,
    review: ConsensusReview,
}

fn add_candidates(target: &mut Vec<SampleCandidate>, reviews: &[SteamReviewDto], recent: bool) {
    for review in reviews {
        let Some(text) = review.review.as_deref().and_then(clean_review_text) else {
            continue;
        };
        let id = review
            .recommendation_id
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| stable_text_id(&text));
        if target.iter().any(|candidate| candidate.id == id) {
            continue;
        }
        target.push(SampleCandidate {
            id: id.clone(),
            recommended: review.voted_up.unwrap_or(false),
            recent,
            votes: review.votes_up.unwrap_or_default(),
            timestamp: review.timestamp_created.unwrap_or_default(),
            review: ConsensusReview {
                id,
                recommended: review.voted_up.unwrap_or(false),
                text,
                playtime_hours: review
                    .playtime_forever_minutes
                    .map(|value| value as f64 / 60.0),
                helpful_votes: review.votes_up,
                language: review.language.clone(),
            },
        });
    }
}

fn candidate_order(left: &SampleCandidate, right: &SampleCandidate) -> std::cmp::Ordering {
    right
        .votes
        .cmp(&left.votes)
        .then_with(|| right.timestamp.cmp(&left.timestamp))
        .then_with(|| left.id.cmp(&right.id))
}

fn clean_review_text(value: &str) -> Option<String> {
    let without_html = strip_html(value);
    let normalized = without_html
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let normalized = normalized.trim();
    if normalized.chars().count() < MIN_REVIEW_TEXT_LENGTH {
        return None;
    }
    Some(normalized.chars().take(800).collect())
}

fn strip_html(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut in_tag = false;
    for character in value.chars() {
        match character {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(character),
            _ => {}
        }
    }
    result
}

fn stable_text_id(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn fingerprint(input: &ConsensusInput) -> String {
    let payload = serde_json::json!({
        "promptVersion": PROMPT_VERSION,
        "gameId": input.game_id,
        "title": input.title,
        "metacritic": input.metacritic,
        "opencritic": input.opencritic,
        "steam": input.steam,
        "sampledReviews": input.sampled_reviews,
    });
    let digest = Sha256::digest(serde_json::to_vec(&payload).unwrap_or_default());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn extract_json(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.starts_with("```") {
        let body = trimmed
            .trim_start_matches('`')
            .trim_start_matches("json")
            .trim();
        return body
            .strip_suffix("```")
            .map(|content| content.trim().to_string());
    }
    Some(trimmed.to_string())
}

fn describe_model_response(value: &str) -> String {
    let trimmed = value.trim();
    let json = serde_json::from_str::<serde_json::Value>(trimmed).ok();
    let json_type = json
        .as_ref()
        .map(|value| {
            if value.is_object() {
                "object"
            } else if value.is_array() {
                "array"
            } else if value.is_string() {
                "string"
            } else if value.is_number() {
                "number"
            } else if value.is_boolean() {
                "boolean"
            } else {
                "null"
            }
        })
        .unwrap_or("invalid-json");
    let keys = json
        .as_ref()
        .and_then(serde_json::Value::as_object)
        .map(|object| {
            let mut keys = object.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            keys.join(",")
        })
        .unwrap_or_default();
    format!(
        "content_length={} json_type={} json_keys={} starts_with_markdown_fence={} line_count={}",
        value.len(),
        json_type,
        if keys.is_empty() { "<none>" } else { &keys },
        trimmed.starts_with("```"),
        value.lines().count()
    )
}

fn normalize_text(value: &str, max_length: usize) -> Option<String> {
    let normalized = value
        .replace(['\n', '\r', '\t'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .trim_end_matches('.')
        .trim()
        .to_string();
    if normalized.is_empty() || normalized.chars().count() > max_length {
        return None;
    }
    Some(normalized)
}

fn normalize_list(values: Vec<String>, max_items: usize) -> Result<Vec<String>, ConsensusError> {
    if values.len() > max_items {
        return Err(ConsensusError::InvalidModelResponse);
    }
    let mut normalized: Vec<String> = Vec::new();
    for value in values {
        let value = normalize_text(&value, 120).ok_or(ConsensusError::InvalidModelResponse)?;
        if !normalized
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(&value))
        {
            normalized.push(value);
        }
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::{build_input, build_prompt, has_minimum_data, parse_model_response};
    use crate::reviews::{
        MetacriticDto, OpenCriticDto, ReviewsSourcesDto, SteamReviewDto, SteamReviewSnapshotDto,
        SteamReviewSummaryDto, SteamReviewsDto,
    };

    fn fixture() -> ReviewsSourcesDto {
        let review = |id: &str, recommended: bool| {
            SteamReviewDto {
            recommendation_id: Some(id.to_string()),
            author: None,
            review: Some("La exploración y el combate ofrecen una experiencia muy satisfactoria y consistente.".to_string()),
            voted_up: Some(recommended),
            playtime_forever_minutes: Some(1200),
            timestamp_created: Some(1),
            language: Some("english".to_string()),
            votes_up: Some(5),
        }
        };
        ReviewsSourcesDto {
            game_id: "game-1".to_string(),
            title: "Fixture Game".to_string(),
            steam_app_id: 1,
            metacritic: Some(MetacriticDto {
                score: Some(80),
                platform: Some("PC".to_string()),
                url: None,
            }),
            opencritic: Some(OpenCriticDto {
                id: Some(1),
                name: Some("Fixture Game".to_string()),
                score: Some(82.0),
                review_count: Some(100),
                percent_recommended: Some(85.0),
                url: None,
            }),
            steam: Some(SteamReviewsDto {
                all: Some(SteamReviewSnapshotDto {
                    query_summary: Some(SteamReviewSummaryDto {
                        total_reviews: Some(100),
                        total_positive: Some(70),
                        total_negative: Some(30),
                        review_score: None,
                        review_score_desc: None,
                    }),
                    reviews: vec![review("positive", true), review("negative", false)],
                }),
                recent: None,
            }),
            errors: Vec::new(),
            input_fingerprint: None,
        }
    }

    #[test]
    fn builds_stable_input_with_balanced_samples_and_fingerprint() {
        let sources = fixture();
        let left = build_input("game-1", &sources);
        let right = build_input("game-1", &sources);
        assert_eq!(left.fingerprint, right.fingerprint);
        assert_eq!(left.sampled_reviews.len(), 2);
        assert!(left.sampled_reviews.iter().any(|review| review.recommended));
        assert!(left
            .sampled_reviews
            .iter()
            .any(|review| !review.recommended));
        assert!(has_minimum_data(&left));
        assert!(build_prompt(&left).user_prompt.contains("Fixture Game"));
    }

    #[test]
    fn rejects_invalid_model_json_and_accepts_valid_structured_json() {
        assert!(parse_model_response("not-json").is_err());
        let valid = parse_model_response(r#"{"overallRating":4.5,"agreement":"high","agreementLabel":"Coinciden","strengths":["Combate"],"weaknesses":["Ritmo"],"conclusion":"La recepción es positiva y consistente entre las fuentes disponibles."}"#).expect("valid consensus");
        assert_eq!(valid.overall_rating, Some(4.5));
        assert!(parse_model_response(r#"{"overallRating":8,"agreement":"high","agreementLabel":"Coinciden","strengths":[],"weaknesses":[],"conclusion":"Conclusión"}"#).is_err());
    }
}
