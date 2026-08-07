use reqwest::{Client, StatusCode};
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;

pub const OPENROUTER_PROVIDER_ID: &str = "openrouter";
pub const OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1";
pub const DEFAULT_MODEL: &str = "google/gemini-2.5-flash";

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AIConnectionStatus {
    pub state: String,
    pub message: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StructuredTextRequest {
    pub system_prompt: String,
    pub user_prompt: String,
}

#[derive(Debug, Clone)]
pub struct StructuredTextResponse {
    pub content: String,
    route: OpenRouterRoute,
}

#[derive(Debug, Clone)]
struct OpenRouterRoute {
    requested_model: String,
    used_model: Option<String>,
    provider: Option<String>,
    free_router: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AIRequestError {
    NotConfigured,
    InvalidModel,
    Authentication,
    PaymentRequired,
    RateLimited,
    Timeout,
    ModelUnavailable,
    ProviderUnavailable,
    InvalidResponseMetadata {
        http_status: u16,
        content_type: String,
        body_length: usize,
    },
    EmptyResponse,
}

impl AIRequestError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotConfigured => "AI_NOT_CONFIGURED",
            Self::InvalidModel => "AI_INVALID_MODEL",
            Self::Authentication => "AI_AUTHENTICATION_ERROR",
            Self::PaymentRequired => "AI_PAYMENT_REQUIRED",
            Self::RateLimited => "AI_RATE_LIMITED",
            Self::Timeout => "AI_TIMEOUT",
            Self::ModelUnavailable => "AI_MODEL_UNAVAILABLE",
            Self::ProviderUnavailable => "AI_PROVIDER_UNAVAILABLE",
            Self::InvalidResponseMetadata { .. } => "AI_INVALID_RESPONSE",
            Self::EmptyResponse => "AI_EMPTY_RESPONSE",
        }
    }

    pub fn diagnostic(&self) -> String {
        match self {
            Self::InvalidResponseMetadata {
                http_status,
                content_type,
                body_length,
            } => format!(
                "http_status={http_status} content_type={content_type} body_length={body_length}"
            ),
            _ => "details=unavailable".to_string(),
        }
    }
}

pub trait AIProvider {
    fn test_connection(&self) -> impl std::future::Future<Output = AIConnectionStatus> + Send;
    fn generate_structured_text(
        &self,
        request: StructuredTextRequest,
    ) -> impl std::future::Future<Output = Result<StructuredTextResponse, AIRequestError>> + Send;
}

pub struct OpenRouterProvider {
    base_url: String,
    model: String,
    api_key: String,
    timeout: Duration,
}

pub async fn generate_structured_text_from_settings(
    state: &crate::settings::DatabaseState,
    request: StructuredTextRequest,
) -> Result<StructuredTextResponse, AIRequestError> {
    let configuration =
        crate::settings::get_ai_configuration(state).map_err(|_| AIRequestError::NotConfigured)?;
    if configuration.configuration.provider_id != OPENROUTER_PROVIDER_ID
        || !configuration.api_key_configured
        || !configuration.credential_available
    {
        return Err(AIRequestError::NotConfigured);
    }
    let api_key =
        crate::settings::get_ai_api_key(state).map_err(|_| AIRequestError::NotConfigured)?;
    let requested_model = configuration.configuration.model.clone();
    let result = OpenRouterProvider::new(requested_model.clone(), api_key)
        .generate_structured_text(request)
        .await;
    match &result {
        Ok(response) => state.log(
            "ai-generation",
            "OPENROUTER_ROUTE",
            &format_openrouter_route(&response.route),
        ),
        Err(error) => state.log(
            "ai-generation",
            "OPENROUTER_REQUEST_FAILED",
            &format!(
                "requested_model={requested_model} error_code={} {}",
                error.code(),
                error.diagnostic()
            ),
        ),
    }
    result
}

impl OpenRouterProvider {
    pub fn new(model: String, api_key: String) -> Self {
        Self {
            base_url: OPENROUTER_BASE_URL.to_string(),
            model,
            api_key,
            timeout: Duration::from_secs(30),
        }
    }

    fn status(state: &str, message: impl Into<Option<String>>) -> AIConnectionStatus {
        AIConnectionStatus {
            state: state.to_string(),
            message: message.into(),
        }
    }
}

impl AIProvider for OpenRouterProvider {
    async fn test_connection(&self) -> AIConnectionStatus {
        if self.api_key.trim().is_empty() {
            return Self::status("not-configured", Some("API Key no configurada".to_string()));
        }
        if !is_valid_model(&self.model) {
            return Self::status(
                "invalid-model",
                Some("El modelo configurado no es válido".to_string()),
            );
        }

        let client = match Client::builder().timeout(self.timeout).build() {
            Ok(client) => client,
            Err(_) => {
                return Self::status("error", Some("No se pudo preparar la conexión".to_string()))
            }
        };
        let response = client
            .get(format!("{}/models", self.base_url.trim_end_matches('/')))
            .bearer_auth(&self.api_key)
            .header("HTTP-Referer", "https://lumadeck.local")
            .header("X-Title", "LumaDeck")
            .send()
            .await;

        match response {
            Ok(response)
                if response.status() == StatusCode::UNAUTHORIZED
                    || response.status() == StatusCode::FORBIDDEN =>
            {
                Self::status(
                    "authentication-error",
                    Some("OpenRouter rechazó la API Key".to_string()),
                )
            }
            Ok(response) if response.status().is_success() => Self::status(
                "connected",
                Some("Conexión validada correctamente".to_string()),
            ),
            Ok(response)
                if response.status() == StatusCode::REQUEST_TIMEOUT
                    || response.status() == StatusCode::GATEWAY_TIMEOUT =>
            {
                Self::status(
                    "timeout",
                    Some("OpenRouter agotó el tiempo de espera".to_string()),
                )
            }
            Ok(_) => Self::status("offline", Some("OpenRouter no está disponible".to_string())),
            Err(error) if error.is_timeout() => Self::status(
                "timeout",
                Some("La conexión agotó el tiempo de espera".to_string()),
            ),
            Err(_) => Self::status(
                "offline",
                Some("No se pudo conectar con OpenRouter".to_string()),
            ),
        }
    }

    async fn generate_structured_text(
        &self,
        request: StructuredTextRequest,
    ) -> Result<StructuredTextResponse, AIRequestError> {
        if self.api_key.trim().is_empty() {
            return Err(AIRequestError::NotConfigured);
        }
        if !is_valid_model(&self.model) {
            return Err(AIRequestError::InvalidModel);
        }
        let client = Client::builder()
            .timeout(self.timeout)
            .build()
            .map_err(|_| AIRequestError::ProviderUnavailable)?;
        let body = json!({
            "model": self.model,
            "messages": [
                { "role": "system", "content": request.system_prompt },
                { "role": "user", "content": request.user_prompt }
            ],
            "stream": false,
            "temperature": 0.1,
            "response_format": {
                "type": "json_schema",
                "json_schema": {
                    "name": "lumadeck_structured_object",
                    "strict": false,
                    "schema": {
                        "type": "object",
                        "additionalProperties": true
                    }
                }
            },
            "provider": { "require_parameters": true }
        });
        let response = client
            .post(format!(
                "{}/chat/completions",
                self.base_url.trim_end_matches('/')
            ))
            .bearer_auth(&self.api_key)
            .header("HTTP-Referer", "https://lumadeck.local")
            .header("X-Title", "LumaDeck")
            .header("X-OpenRouter-Metadata", "enabled")
            .json(&body)
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    AIRequestError::Timeout
                } else {
                    AIRequestError::ProviderUnavailable
                }
            })?;
        let status = response.status();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("unavailable")
            .to_string();
        let response_body =
            response
                .text()
                .await
                .map_err(|_| AIRequestError::InvalidResponseMetadata {
                    http_status: status.as_u16(),
                    content_type: content_type.clone(),
                    body_length: 0,
                })?;
        let body_length = response_body.len();
        if !status.is_success() {
            return Err(match status {
                StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => AIRequestError::Authentication,
                StatusCode::PAYMENT_REQUIRED => AIRequestError::PaymentRequired,
                StatusCode::TOO_MANY_REQUESTS => AIRequestError::RateLimited,
                StatusCode::REQUEST_TIMEOUT | StatusCode::GATEWAY_TIMEOUT => {
                    AIRequestError::Timeout
                }
                StatusCode::BAD_REQUEST | StatusCode::NOT_FOUND => AIRequestError::ModelUnavailable,
                _ if status.is_server_error() => AIRequestError::ProviderUnavailable,
                _ => AIRequestError::InvalidResponseMetadata {
                    http_status: status.as_u16(),
                    content_type,
                    body_length,
                },
            });
        }
        let response_json =
            serde_json::from_str::<serde_json::Value>(&response_body).map_err(|_| {
                AIRequestError::InvalidResponseMetadata {
                    http_status: status.as_u16(),
                    content_type: content_type.clone(),
                    body_length,
                }
            })?;
        let route = openrouter_route_from_response(&self.model, &response_json);
        #[cfg(debug_assertions)]
        log_openrouter_route(&route);
        let payload =
            serde_json::from_value::<OpenRouterChatResponse>(response_json).map_err(|_| {
                AIRequestError::InvalidResponseMetadata {
                    http_status: status.as_u16(),
                    content_type,
                    body_length,
                }
            })?;
        let content = payload
            .choices
            .into_iter()
            .next()
            .and_then(|choice| choice.message.content)
            .and_then(OpenRouterMessageContent::into_text)
            .filter(|value| !value.trim().is_empty())
            .ok_or(AIRequestError::EmptyResponse)?;
        Ok(StructuredTextResponse { content, route })
    }
}

fn openrouter_route_from_response(
    requested_model: &str,
    response: &serde_json::Value,
) -> OpenRouterRoute {
    let used_model = response
        .get("model")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);
    let provider = response
        .get("provider")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            response
                .pointer("/openrouter_metadata/endpoints/available")
                .and_then(serde_json::Value::as_array)
                .and_then(|endpoints| {
                    endpoints.iter().find_map(|endpoint| {
                        endpoint
                            .get("selected")
                            .and_then(serde_json::Value::as_bool)
                            .filter(|selected| *selected)
                            .and(endpoint.get("provider"))
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_string)
                    })
                })
        });
    let free_router = requested_model == "openrouter/free"
        || response
            .pointer("/openrouter_metadata/strategy")
            .and_then(serde_json::Value::as_str)
            == Some("free");

    OpenRouterRoute {
        requested_model: requested_model.to_string(),
        used_model,
        provider,
        free_router,
    }
}

fn format_openrouter_route(route: &OpenRouterRoute) -> String {
    format!(
        "requested_model={} used_model={} provider={} free_router={}",
        route.requested_model,
        route.used_model.as_deref().unwrap_or("unavailable"),
        route.provider.as_deref().unwrap_or("unavailable"),
        route.free_router
    )
}

#[cfg(debug_assertions)]
fn log_openrouter_route(route: &OpenRouterRoute) {
    println!("[OpenRouter][dev] {}", format_openrouter_route(route));
}

#[derive(Debug, Deserialize)]
struct OpenRouterChatResponse {
    choices: Vec<OpenRouterChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenRouterChoice {
    message: OpenRouterMessage,
}

#[derive(Debug, Deserialize)]
struct OpenRouterMessage {
    content: Option<OpenRouterMessageContent>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum OpenRouterMessageContent {
    Text(String),
    Parts(Vec<OpenRouterContentPart>),
    Part(OpenRouterContentPart),
}

impl OpenRouterMessageContent {
    fn into_text(self) -> Option<String> {
        match self {
            Self::Text(value) => Some(value),
            Self::Parts(parts) => Some(
                parts
                    .into_iter()
                    .filter_map(|part| part.text)
                    .collect::<Vec<_>>()
                    .join(""),
            ),
            Self::Part(part) => part.text,
        }
    }
}

#[derive(Debug, Deserialize)]
struct OpenRouterContentPart {
    text: Option<String>,
}

pub fn is_valid_provider(provider_id: &str) -> bool {
    provider_id == OPENROUTER_PROVIDER_ID
}

pub fn is_valid_model(model: &str) -> bool {
    let normalized = model.trim();
    !normalized.is_empty()
        && normalized.len() <= 256
        && !normalized.chars().any(|character| character.is_control())
}

pub fn is_valid_api_key(api_key: &str) -> bool {
    let normalized = api_key.trim();
    normalized.len() >= 10
        && normalized.len() <= 256
        && !normalized.chars().any(|character| character.is_control())
}

#[cfg(test)]
mod tests {
    use super::{is_valid_api_key, is_valid_model, is_valid_provider, OpenRouterChatResponse};

    #[test]
    fn accepts_openrouter_and_manual_models_without_supporting_other_providers() {
        assert!(is_valid_provider("openrouter"));
        assert!(!is_valid_provider("ollama"));
        assert!(is_valid_model("google/gemini-2.5-flash"));
        assert!(is_valid_model("custom/provider-model"));
        assert!(!is_valid_model("   "));
        assert!(!is_valid_model("model\nwith-break"));
    }

    #[test]
    fn rejects_short_or_multiline_api_keys() {
        assert!(is_valid_api_key("sk-or-v1-valid-key"));
        assert!(!is_valid_api_key("short"));
        assert!(!is_valid_api_key("sk-or-v1-valid\nkey"));
    }

    #[test]
    fn accepts_openrouter_text_and_content_parts() {
        let text = serde_json::from_str::<OpenRouterChatResponse>(
            r#"{"choices":[{"message":{"content":"{\"ok\":true}"}}]}"#,
        )
        .expect("text response");
        assert_eq!(text.choices.len(), 1);

        let parts = serde_json::from_str::<OpenRouterChatResponse>(
            r#"{"choices":[{"message":{"content":[{"type":"text","text":"{\"ok\":"},{"type":"text","text":"true}"}]}}]}"#,
        )
        .expect("parts response");
        assert_eq!(parts.choices.len(), 1);
    }
}
