//! Claude provider: a hand-rolled `reqwest` client against the Anthropic
//! Messages API (there is no official Rust SDK). Uses adaptive thinking by
//! default, per current API guidance for Claude 4.6+ models.

use async_trait::async_trait;
use bench_core::{Message, ModelProvider, ModelResponse, ProviderError, TokenUsage};
use provider_http_util::{
    WireMessage, build_client, decode_json, default_max_tokens, model_label, send_for_body,
    wire_messages,
};
use serde::{Deserialize, Serialize};

const MESSAGES_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Model families that accept `thinking: {type: "adaptive"}`. Older models
/// (Haiku 4.5, Sonnet 4.5 and earlier) reject the parameter with a 400.
fn supports_adaptive_thinking(model: &str) -> bool {
    [
        "fable-5",
        "mythos-5",
        "opus-4-6",
        "opus-4-7",
        "opus-4-8",
        "sonnet-4-6",
    ]
    .iter()
    .any(|family| model.contains(family))
}

/// Deserialized from this provider's entry in config.yml.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClaudeConfig {
    /// Model ID, e.g. "claude-opus-4-8".
    pub model: String,
    /// Label identifying this entry in output records; defaults to the
    /// model ID. Set it when benchmarking the same model under different
    /// settings (e.g. "haiku-high-effort").
    #[serde(default)]
    pub label: Option<String>,
    /// Falls back to the ANTHROPIC_API_KEY environment variable, which is
    /// the recommended place for it — avoid committing keys in config.yml.
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    /// Adaptive thinking on/off. Defaults per model: on for families that
    /// support it, off otherwise (e.g. Haiku 4.5 rejects the parameter).
    /// Set explicitly to override the family detection.
    #[serde(default)]
    pub thinking: Option<bool>,
    /// Optional output_config effort level: low | medium | high | xhigh |
    /// max. Only Opus-tier models, Sonnet 4.6, and Fable 5 accept it —
    /// setting it for Haiku 4.5 is a fatal 400.
    #[serde(default)]
    pub effort: Option<String>,
}

impl ClaudeConfig {
    /// Resolve the API key: the inline `api_key` wins, else fall back to
    /// ANTHROPIC_API_KEY. Run before constructing the provider — the
    /// provider itself never reads the environment.
    pub fn resolve_api_key(&self) -> Result<String, String> {
        self.api_key
            .clone()
            .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok())
            .ok_or_else(|| {
                "no Anthropic API key: set ANTHROPIC_API_KEY or `api_key` in the model config"
                    .to_string()
            })
    }

    fn thinking_enabled(&self) -> bool {
        self.thinking
            .unwrap_or_else(|| supports_adaptive_thinking(&self.model))
    }
}

pub struct Claude {
    config: ClaudeConfig,
    api_key: String,
    http: reqwest::Client,
}

impl Claude {
    pub fn new(config: ClaudeConfig, api_key: String) -> Result<Self, String> {
        Ok(Self {
            config,
            api_key,
            http: build_client()?,
        })
    }

    fn build_request<'a>(&'a self, conversation: &'a [Message]) -> MessagesRequest<'a> {
        MessagesRequest {
            model: &self.config.model,
            max_tokens: self.config.max_tokens,
            messages: wire_messages(conversation),
            thinking: self
                .config
                .thinking_enabled()
                .then_some(Thinking { r#type: "adaptive" }),
            output_config: self
                .config
                .effort
                .as_deref()
                .map(|effort| OutputConfig { effort }),
            cache_control: CacheControl {
                r#type: "ephemeral",
            },
        }
    }
}

#[async_trait]
impl ModelProvider for Claude {
    fn model_id(&self) -> String {
        model_label(self.config.label.as_deref(), &self.config.model)
    }

    async fn send_prompt(&self, conversation: &[Message]) -> Result<ModelResponse, ProviderError> {
        let request = self
            .http
            .post(MESSAGES_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .json(&self.build_request(conversation));
        let body = send_for_body(request).await?;
        Ok(into_model_response(decode_json(&body)?))
    }
}

/// A `refusal` stop reason arrives as a successful response with no text
/// content; the empty text flows through extraction as a malformed response
/// and is scored against the model, not the run. The abnormal stop reason
/// travels along for honest trace records.
fn into_model_response(parsed: MessagesResponse) -> ModelResponse {
    let text = parsed
        .content
        .iter()
        .filter(|block| block.kind == "text")
        .map(|block| block.text.as_str())
        .collect::<Vec<_>>()
        .join("");
    let stop = parsed
        .stop_reason
        .filter(|reason| reason != "end_turn" && reason != "stop_sequence");
    ModelResponse {
        text,
        tokens: TokenUsage {
            input: parsed.usage.input_tokens,
            output: parsed.usage.output_tokens,
        },
        stop,
    }
}

#[derive(Serialize)]
struct MessagesRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    messages: Vec<WireMessage<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<Thinking>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_config: Option<OutputConfig<'a>>,
    /// Top-level automatic caching: the API places the breakpoint on the last
    /// cacheable block itself and moves it as the conversation grows, so
    /// nothing here has to know where the prompt's stable part ends.
    ///
    /// What this does and does not buy, given the runner sends the whole
    /// prompt as one block ending in the question: the repetitions of a
    /// question are byte-identical, so every repetition after the first reads
    /// from cache, and a retry conversation reads its earlier turns. Two
    /// DIFFERENT questions never share a hit, because the cache hash covers
    /// whole blocks and the question sits in the same block as the schema.
    /// Splitting the stable prefix into its own block would capture those too.
    ///
    /// Always on. A cache write costs 1.25x the input price, so this only pays
    /// off when a prompt is reused — which REPETITIONS guarantees at 3. Drop
    /// it if repetitions ever go to 1. Prompts below the model's minimum
    /// cacheable size (4,096 tokens on Haiku 4.5, the highest of any model)
    /// are silently not cached and cost nothing extra.
    cache_control: CacheControl,
}

#[derive(Serialize)]
struct CacheControl {
    r#type: &'static str,
}

#[derive(Serialize)]
struct Thinking {
    r#type: &'static str,
}

#[derive(Serialize)]
struct OutputConfig<'a> {
    effort: &'a str,
}

#[derive(Deserialize)]
struct MessagesResponse {
    content: Vec<ContentBlock>,
    #[serde(default)]
    stop_reason: Option<String>,
    usage: Usage,
}

#[derive(Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    text: String,
}

#[derive(Deserialize)]
struct Usage {
    input_tokens: u64,
    output_tokens: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config() -> ClaudeConfig {
        ClaudeConfig {
            model: "claude-opus-4-8".to_string(),
            label: None,
            api_key: Some("test-key".to_string()),
            max_tokens: default_max_tokens(),
            thinking: None,
            effort: Some("high".to_string()),
        }
    }

    #[test]
    fn label_overrides_the_record_model_id() {
        let claude = Claude::new(config(), "test-key".to_string()).unwrap();
        assert_eq!(claude.model_id(), "claude-opus-4-8");

        let mut labelled = config();
        labelled.label = Some("opus-high-effort".to_string());
        let claude = Claude::new(labelled, "test-key".to_string()).unwrap();
        assert_eq!(claude.model_id(), "opus-high-effort");
    }

    #[test]
    fn builds_the_expected_request_shape() {
        let claude = Claude::new(config(), "test-key".to_string()).unwrap();
        let conversation = [
            Message::user("generate a query"),
            Message::assistant("```\nbad\n```"),
            Message::user("that failed; fix it"),
        ];
        let request = serde_json::to_value(claude.build_request(&conversation)).unwrap();
        assert_eq!(
            request,
            serde_json::json!({
                "model": "claude-opus-4-8",
                "max_tokens": 4096,
                // Automatic prompt caching, sent on every request: the
                // repetitions of a question are identical prompts, so all but
                // the first read from cache.
                "cache_control": {"type": "ephemeral"},
                "messages": [
                    {"role": "user", "content": "generate a query"},
                    {"role": "assistant", "content": "```\nbad\n```"},
                    {"role": "user", "content": "that failed; fix it"},
                ],
                "thinking": {"type": "adaptive"},
                "output_config": {"effort": "high"},
            })
        );
    }

    #[test]
    fn thinking_and_effort_are_omitted_when_disabled() {
        let mut cfg = config();
        cfg.thinking = Some(false);
        cfg.effort = None;
        let claude = Claude::new(cfg, "test-key".to_string()).unwrap();
        let request = serde_json::to_value(claude.build_request(&[Message::user("q")])).unwrap();
        assert!(request.get("thinking").is_none());
        assert!(request.get("output_config").is_none());
    }

    /// The default follows model capability: models that reject the
    /// adaptive-thinking parameter must not receive it.
    #[test]
    fn thinking_defaults_follow_the_model_family() {
        let mut cfg = config();
        cfg.model = "claude-haiku-4-5-20251001".to_string();
        cfg.effort = None;
        let claude = Claude::new(cfg, "test-key".to_string()).unwrap();
        let request = serde_json::to_value(claude.build_request(&[Message::user("q")])).unwrap();
        assert!(request.get("thinking").is_none());

        let claude = Claude::new(config(), "test-key".to_string()).unwrap();
        let request = serde_json::to_value(claude.build_request(&[Message::user("q")])).unwrap();
        assert_eq!(request["thinking"], serde_json::json!({"type": "adaptive"}));

        // An explicit setting overrides the family detection both ways.
        let mut cfg = config();
        cfg.model = "claude-haiku-4-5-20251001".to_string();
        cfg.thinking = Some(true);
        cfg.effort = None;
        let claude = Claude::new(cfg, "test-key".to_string()).unwrap();
        let request = serde_json::to_value(claude.build_request(&[Message::user("q")])).unwrap();
        assert_eq!(request["thinking"], serde_json::json!({"type": "adaptive"}));
    }

    #[test]
    fn extracts_text_blocks_and_usage_from_a_response() {
        // Thinking blocks (empty text under the default display) are skipped
        // and a normal end of turn records no abnormal stop.
        let parsed: MessagesResponse = serde_json::from_str(
            r#"{
                "content": [
                    {"type": "thinking", "thinking": "", "signature": "x"},
                    {"type": "text", "text": "Here you go:\n"},
                    {"type": "text", "text": "```\nselect 1\n```"}
                ],
                "stop_reason": "end_turn",
                "usage": {"input_tokens": 812, "output_tokens": 40}
            }"#,
        )
        .unwrap();
        let response = into_model_response(parsed);
        assert_eq!(response.text, "Here you go:\n```\nselect 1\n```");
        assert_eq!(response.tokens.input, 812);
        assert_eq!(response.tokens.output, 40);
        assert_eq!(response.stop, None);
    }

    #[test]
    fn refusals_surface_as_an_abnormal_stop() {
        let parsed: MessagesResponse = serde_json::from_str(
            r#"{
                "content": [],
                "stop_reason": "refusal",
                "usage": {"input_tokens": 100, "output_tokens": 0}
            }"#,
        )
        .unwrap();
        let response = into_model_response(parsed);
        assert_eq!(response.text, "");
        assert_eq!(response.stop.as_deref(), Some("refusal"));
    }

    #[test]
    fn resolving_without_any_key_source_is_an_error() {
        let mut cfg = config();
        cfg.api_key = None;
        // Only meaningful when the env var is absent; skip otherwise.
        if std::env::var("ANTHROPIC_API_KEY").is_err() {
            assert!(cfg.resolve_api_key().is_err());
        }
        // The inline key wins without touching the environment.
        assert_eq!(config().resolve_api_key().unwrap(), "test-key");
    }
}
