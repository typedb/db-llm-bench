//! Provider for any model behind an OpenAI-compatible chat-completions
//! endpoint — OpenAI itself, Groq, OpenRouter, DeepSeek, or a local
//! Ollama/vLLM server. Deliberately speaks only the narrow, universally
//! cloned core of the API (model + messages + output cap), which is what
//! keeps it portable across providers.

use async_trait::async_trait;
use bench_core::{Message, ModelProvider, ModelResponse, ProviderError, TokenUsage};
use provider_http_util::{
    WireMessage, build_client, decode_json, default_max_tokens, model_label, parse_error_message,
    send_for_body, wire_messages,
};
use serde::{Deserialize, Serialize};

/// Which wire field carries the output cap. `max_tokens` is the widely
/// cloned form (Groq, OpenRouter, Ollama, ...); OpenAI's newest models
/// require `max_completion_tokens` instead.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaxTokensField {
    #[default]
    MaxTokens,
    MaxCompletionTokens,
}

/// Deserialized from this provider's entry in config.yml.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenAiCompatibleConfig {
    /// The provider's model identifier — naming schemes differ per provider
    /// (e.g. "gpt-5", "llama-3.3-70b-versatile", "deepseek/deepseek-chat").
    pub model: String,
    /// Endpoint base, e.g. "https://api.groq.com/openai/v1" or
    /// "http://localhost:11434/v1"; "/chat/completions" is appended.
    pub base_url: String,
    /// Label identifying this entry in output records; defaults to the
    /// model ID. Also useful to disambiguate the same weights served by
    /// different providers.
    #[serde(default)]
    pub label: Option<String>,
    /// Environment variable holding the API key (e.g. "GROQ_API_KEY").
    /// Omit entirely for unauthenticated local servers.
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    /// Wire field for the output cap: "max_tokens" (default) or
    /// "max_completion_tokens".
    #[serde(default)]
    pub max_tokens_field: MaxTokensField,
    /// Extra top-level keys merged into the request body verbatim.
    ///
    /// The rest of this provider speaks only the universally cloned core of
    /// the API, which is what keeps it portable; this is the escape hatch for
    /// the parameters that are not portable. Reasoning effort is the case that
    /// forced it — a model whose thinking cannot be turned down emits tens of
    /// thousands of tokens per call, and with a serial runner that is the
    /// difference between a run finishing in an hour and in a day.
    ///
    /// Keys here override the fields built above if they collide, so this can
    /// also correct a provider that wants a different spelling. Nothing is
    /// validated: an unsupported key reaches the provider and its error comes
    /// back as an ordinary HTTP failure.
    #[serde(default)]
    pub extra_body: serde_json::Map<String, serde_json::Value>,
}

impl OpenAiCompatibleConfig {
    /// Resolve the API key from the environment variable named by
    /// `api_key_env`; None (no Authorization header) when the field is
    /// omitted. Run before constructing the provider — the provider itself
    /// never reads the environment.
    pub fn resolve_api_key(&self) -> Result<Option<String>, String> {
        match &self.api_key_env {
            Some(env_var) => std::env::var(env_var).map(Some).map_err(|_| {
                format!("api_key_env `{env_var}` is set in config but the variable is not set")
            }),
            None => Ok(None),
        }
    }
}

pub struct OpenAiCompatible {
    config: OpenAiCompatibleConfig,
    /// None for unauthenticated local servers.
    api_key: Option<String>,
    url: reqwest::Url,
    http: reqwest::Client,
}

impl OpenAiCompatible {
    pub fn new(config: OpenAiCompatibleConfig, api_key: Option<String>) -> Result<Self, String> {
        let url = endpoint_url(&config.base_url)?;
        Ok(Self {
            config,
            api_key,
            url,
            http: build_client()?,
        })
    }

    fn build_request<'a>(&'a self, conversation: &'a [Message]) -> ChatRequest<'a> {
        let (max_tokens, max_completion_tokens) = match self.config.max_tokens_field {
            MaxTokensField::MaxTokens => (Some(self.config.max_tokens), None),
            MaxTokensField::MaxCompletionTokens => (None, Some(self.config.max_tokens)),
        };
        ChatRequest {
            model: &self.config.model,
            messages: wire_messages(conversation),
            max_tokens,
            max_completion_tokens,
            extra: &self.config.extra_body,
        }
    }
}

/// Join and validate eagerly, so a typo'd base_url fails at startup rather
/// than surfacing mid-run as a retried-with-backoff network error.
fn endpoint_url(base_url: &str) -> Result<reqwest::Url, String> {
    let url = reqwest::Url::parse(&format!(
        "{}/chat/completions",
        base_url.trim_end_matches('/')
    ))
    .map_err(|e| format!("invalid base_url `{base_url}`: {e}"))?;
    // A scheme-less "localhost:11434/v1" parses with "localhost" as the
    // scheme, so a plain parse check isn't enough.
    if !matches!(url.scheme(), "http" | "https") {
        return Err(format!(
            "invalid base_url `{base_url}`: expected an http:// or https:// URL"
        ));
    }
    Ok(url)
}

#[async_trait]
impl ModelProvider for OpenAiCompatible {
    fn model_id(&self) -> String {
        model_label(self.config.label.as_deref(), &self.config.model)
    }

    async fn send_prompt(&self, conversation: &[Message]) -> Result<ModelResponse, ProviderError> {
        let mut request = self
            .http
            .post(self.url.clone())
            .json(&self.build_request(conversation));
        if let Some(api_key) = &self.api_key {
            request = request.bearer_auth(api_key);
        }
        decode_response(&send_for_body(request).await?)
    }
}

/// Some gateways (notably OpenRouter) deliver upstream-provider failures as
/// an error object inside an HTTP 200 body. Surface those as Transient with
/// the API's own message rather than aborting on "undecodable response".
fn decode_response(body: &str) -> Result<ModelResponse, ProviderError> {
    match decode_json::<ChatResponse>(body) {
        Ok(parsed) => into_model_response(parsed),
        Err(undecodable) => Err(match parse_error_message(body) {
            Some(message) => {
                ProviderError::Transient(format!("API error in HTTP 200 response: {message}"))
            }
            None => undecodable,
        }),
    }
}

/// An abnormal finish_reason ("length", "content_filter", ...) travels
/// along as the stop reason so the attempt trace can say why there was no
/// usable query; null content (e.g. a filtered response) becomes empty
/// text and flows through extraction as a malformed response.
fn into_model_response(parsed: ChatResponse) -> Result<ModelResponse, ProviderError> {
    let choice = parsed
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| ProviderError::Fatal("API response contained no choices".to_string()))?;
    let stop = choice.finish_reason.filter(|reason| reason != "stop");
    let usage = parsed.usage.unwrap_or_default();
    Ok(ModelResponse {
        text: choice.message.content.unwrap_or_default(),
        tokens: TokenUsage {
            input: usage.prompt_tokens,
            output: usage.completion_tokens,
        },
        stop,
    })
}

#[derive(Serialize)]
struct ChatRequest<'a> {
    model: &'a str,
    messages: Vec<WireMessage<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_completion_tokens: Option<u32>,
    /// Flattened last so a colliding key from config wins over the field
    /// built above it.
    #[serde(flatten)]
    extra: &'a serde_json::Map<String, serde_json::Value>,
}

#[derive(Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
    /// The Option (rather than a defaulted Usage) tolerates an explicit
    /// `"usage": null` as well as a missing field; zeros beat a decode
    /// error either way.
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Choice {
    message: ChoiceMessage,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct ChoiceMessage {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Deserialize, Default)]
struct Usage {
    #[serde(default)]
    prompt_tokens: u64,
    #[serde(default)]
    completion_tokens: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> OpenAiCompatibleConfig {
        OpenAiCompatibleConfig {
            model: "llama-3.3-70b-versatile".to_string(),
            base_url: "https://api.groq.com/openai/v1/".to_string(),
            label: None,
            api_key_env: None,
            max_tokens: default_max_tokens(),
            max_tokens_field: MaxTokensField::default(),
            extra_body: serde_json::Map::new(),
        }
    }

    #[test]
    fn builds_the_minimal_portable_request() {
        let provider = OpenAiCompatible::new(config(), None).unwrap();
        assert_eq!(
            provider.url.as_str(),
            "https://api.groq.com/openai/v1/chat/completions"
        );
        let conversation = [
            Message::user("generate a query"),
            Message::assistant("```\nbad\n```"),
            Message::user("that failed; fix it"),
        ];
        let request = serde_json::to_value(provider.build_request(&conversation)).unwrap();
        assert_eq!(
            request,
            serde_json::json!({
                "model": "llama-3.3-70b-versatile",
                "messages": [
                    {"role": "user", "content": "generate a query"},
                    {"role": "assistant", "content": "```\nbad\n```"},
                    {"role": "user", "content": "that failed; fix it"},
                ],
                "max_tokens": 4096,
            })
        );
    }

    #[test]
    fn max_completion_tokens_knob_switches_the_wire_field() {
        let mut cfg = config();
        cfg.max_tokens_field = MaxTokensField::MaxCompletionTokens;
        let provider = OpenAiCompatible::new(cfg, None).unwrap();
        let request = serde_json::to_value(provider.build_request(&[Message::user("q")])).unwrap();
        assert!(request.get("max_tokens").is_none());
        assert_eq!(request["max_completion_tokens"], 4096);
    }

    #[test]
    fn invalid_base_urls_fail_at_startup() {
        let mut missing_scheme = config();
        missing_scheme.base_url = "localhost:11434/v1".to_string();
        assert!(OpenAiCompatible::new(missing_scheme, None).is_err());

        let mut garbage = config();
        garbage.base_url = "not a url".to_string();
        assert!(OpenAiCompatible::new(garbage, None).is_err());
    }

    #[test]
    fn parses_a_normal_response() {
        let response = decode_response(
            r#"{
                "choices": [{
                    "message": {"role": "assistant", "content": "```\nselect 1\n```"},
                    "finish_reason": "stop"
                }],
                "usage": {"prompt_tokens": 640, "completion_tokens": 22, "total_tokens": 662}
            }"#,
        )
        .unwrap();
        assert_eq!(response.text, "```\nselect 1\n```");
        assert_eq!(response.tokens.input, 640);
        assert_eq!(response.tokens.output, 22);
        assert_eq!(response.stop, None);
    }

    #[test]
    fn truncation_and_null_content_surface_as_abnormal() {
        let response = decode_response(
            r#"{
                "choices": [{"message": {"content": null}, "finish_reason": "content_filter"}],
                "usage": null
            }"#,
        )
        .unwrap();
        assert_eq!(response.text, "");
        assert_eq!(response.stop.as_deref(), Some("content_filter"));
        // Null usage decodes to zeros rather than failing.
        assert_eq!(response.tokens.input, 0);

        assert!(matches!(
            decode_response(r#"{"choices": []}"#),
            Err(ProviderError::Fatal(_))
        ));
    }

    #[test]
    fn an_error_body_behind_http_200_is_transient() {
        let result = decode_response(
            r#"{"error": {"message": "upstream provider unavailable", "code": 502}}"#,
        );
        assert!(matches!(
            result,
            Err(ProviderError::Transient(m)) if m.contains("upstream provider unavailable")
        ));

        // Truly undecodable bodies stay fatal.
        assert!(matches!(
            decode_response("<html>bad gateway</html>"),
            Err(ProviderError::Fatal(_))
        ));
    }

    #[test]
    fn resolving_a_named_env_var_requires_it_to_be_set() {
        let mut cfg = config();
        cfg.api_key_env = Some("BENCH_TEST_NO_SUCH_KEY".to_string());
        assert!(cfg.resolve_api_key().is_err());
        // No api_key_env at all means an unauthenticated local server.
        assert_eq!(config().resolve_api_key().unwrap(), None);
    }

    #[test]
    fn label_overrides_the_record_model_id() {
        let provider = OpenAiCompatible::new(config(), None).unwrap();
        assert_eq!(provider.model_id(), "llama-3.3-70b-versatile");

        let mut labelled = config();
        labelled.label = Some("llama-70b-groq".to_string());
        let provider = OpenAiCompatible::new(labelled, None).unwrap();
        assert_eq!(provider.model_id(), "llama-70b-groq");
    }

    #[test]
    fn extra_body_keys_reach_the_wire_and_win_collisions() {
        let mut cfg = config();
        cfg.extra_body = serde_json::json!({
            "reasoning_effort": "high",
            "max_tokens": 999,
        })
        .as_object()
        .unwrap()
        .clone();
        let provider = OpenAiCompatible::new(cfg, None).unwrap();
        let request = serde_json::to_value(provider.build_request(&[Message::user("hi")])).unwrap();
        assert_eq!(request["reasoning_effort"], "high");
        // The config's own max_tokens loses to the override — flattening last
        // is what lets extra_body correct a field this provider builds.
        assert_eq!(request["max_tokens"], 999);
    }

    #[test]
    fn an_absent_extra_body_changes_nothing() {
        let provider = OpenAiCompatible::new(config(), None).unwrap();
        let request = serde_json::to_value(provider.build_request(&[Message::user("hi")])).unwrap();
        // Sorted, because serde_json's map is a BTreeMap.
        assert_eq!(
            request.as_object().unwrap().keys().collect::<Vec<_>>(),
            vec!["max_tokens", "messages", "model"]
        );
    }

    /// The config reaches this struct as JSON transcoded from the run's YAML,
    /// and `deny_unknown_fields` means a field this provider does not know
    /// aborts the run — but only once models are built, long after `verify`
    /// has pronounced the config fine. Pin the full shape a real entry uses.
    #[test]
    fn deserializes_a_full_entry_including_extra_body() {
        let entry = serde_json::json!({
            "model": "glm-5.2",
            "base_url": "https://api.z.ai/api/paas/v4",
            "label": "glm-5.2@z.ai",
            "api_key_env": "ZAI_API_KEY",
            "max_tokens": 32768,
            "extra_body": {"reasoning_effort": "high"},
        });
        let cfg: OpenAiCompatibleConfig = serde_json::from_value(entry).unwrap();
        assert_eq!(cfg.extra_body["reasoning_effort"], "high");
        let provider = OpenAiCompatible::new(cfg, None).unwrap();
        let body = serde_json::to_value(provider.build_request(&[Message::user("q")])).unwrap();
        assert_eq!(body["reasoning_effort"], "high");
        assert_eq!(body["max_tokens"], 32768);
    }
}
