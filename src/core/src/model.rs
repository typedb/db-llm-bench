use async_trait::async_trait;
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    User,
    Assistant,
}

#[derive(Debug, Clone)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct TokenUsage {
    pub input: u64,
    pub output: u64,
}

impl TokenUsage {
    pub fn add(&mut self, other: TokenUsage) {
        self.input += other.input;
        self.output += other.output;
    }
}

#[derive(Debug, Clone)]
pub struct ModelResponse {
    pub text: String,
    pub tokens: TokenUsage,
    /// An abnormal stop the provider observed (e.g. "refusal",
    /// "max_tokens"); None for a normal end of turn. Recorded in the
    /// attempt trace so a refusal or truncation isn't indistinguishable
    /// from a merely malformed response.
    pub stop: Option<String>,
}

/// Provider errors are never the model's fault, so no variant counts
/// against the retry budget; the split decides what the harness does next.
#[derive(Debug, Error)]
pub enum ProviderError {
    /// Rate limits, network blips — the harness retries with backoff, and
    /// aborts the run if they persist past its patience.
    #[error("transient provider error: {0}")]
    Transient(String),
    /// The request ran out of time client-side. Retried like Transient, but
    /// persistence ends only the repetition in flight, not the run: a
    /// provider that stalls for 10 minutes at a time costs one record, not
    /// the rest of the benchmark.
    #[error("provider timeout: {0}")]
    Timeout(String),
    /// Auth failures, malformed requests — abort the run.
    #[error("fatal provider error: {0}")]
    Fatal(String),
}

impl ProviderError {
    /// Classify an HTTP failure by status code: 408/409/429 and all 5xx
    /// (including 529 overloaded) are retryable — the same set the official
    /// SDKs retry; the remaining 4xx (bad request, auth, not found) won't
    /// get better on retry.
    pub fn from_http_status(status: u16, message: String) -> ProviderError {
        let detail = format!("HTTP {status}: {message}");
        if status == 408 || status == 409 || status == 429 || status >= 500 {
            ProviderError::Transient(detail)
        } else {
            ProviderError::Fatal(detail)
        }
    }
}

/// Unified interface implemented by each model provider package.
#[async_trait]
pub trait ModelProvider: Send + Sync {
    /// Identifier recorded in the output's `model` field.
    fn model_id(&self) -> String;

    /// Send the conversation so far — the initial prompt plus any retry
    /// feedback — and return the model's response.
    async fn send_prompt(&self, conversation: &[Message]) -> Result<ModelResponse, ProviderError>;
}
