//! Dummy provider for testing: replays scripted responses in order and
//! errors once the script runs out. Every conversation received is recorded
//! for assertions. Token counts are text lengths (input: conversation,
//! output: response), so they're deterministic and nonzero.

use std::collections::VecDeque;
use std::sync::Mutex;

use async_trait::async_trait;
use bench_core::{Message, ModelProvider, ModelResponse, ProviderError, TokenUsage};
use serde::Deserialize;

/// Deserialized from this provider's entry in config.yml, for smoke runs
/// through the CLI.
#[derive(Debug, Clone, Deserialize)]
pub struct DummyConfig {
    pub responses: Vec<String>,
    /// Cycle through the responses forever instead of consuming them.
    #[serde(default)]
    pub repeat: bool,
    /// Label identifying this entry in output records; defaults to "dummy".
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Clone)]
enum Scripted {
    Text(String),
    TransientError(String),
    TimeoutError(String),
}

pub struct DummyProvider {
    responses: Mutex<VecDeque<Scripted>>,
    repeat: bool,
    label: Option<String>,
    conversations: Mutex<Vec<Vec<Message>>>,
}

impl DummyProvider {
    pub fn new(responses: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            responses: Mutex::new(
                responses
                    .into_iter()
                    .map(|text| Scripted::Text(text.into()))
                    .collect(),
            ),
            repeat: false,
            label: None,
            conversations: Mutex::new(Vec::new()),
        }
    }

    /// Cycle through the scripted responses instead of consuming them.
    pub fn repeating(mut self) -> Self {
        self.repeat = true;
        self
    }

    pub fn push_response(&self, text: impl Into<String>) {
        self.responses
            .lock()
            .unwrap()
            .push_back(Scripted::Text(text.into()));
    }

    /// Script a transient provider error (e.g. a rate limit), for testing
    /// harness-level backoff.
    pub fn push_transient_error(&self, message: impl Into<String>) {
        self.responses
            .lock()
            .unwrap()
            .push_back(Scripted::TransientError(message.into()));
    }

    /// Script a provider timeout, for testing that persistent timeouts end
    /// the repetition rather than the run.
    pub fn push_timeout_error(&self, message: impl Into<String>) {
        self.responses
            .lock()
            .unwrap()
            .push_back(Scripted::TimeoutError(message.into()));
    }

    /// Every conversation received so far, in order.
    pub fn conversations(&self) -> Vec<Vec<Message>> {
        self.conversations.lock().unwrap().clone()
    }

    /// Pop the next scripted entry, cycling it to the back when repeating.
    fn next_scripted(&self) -> Option<Scripted> {
        let mut responses = self.responses.lock().unwrap();
        let next = responses.pop_front();
        if self.repeat
            && let Some(scripted) = &next
        {
            responses.push_back(scripted.clone());
        }
        next
    }
}

impl From<DummyConfig> for DummyProvider {
    fn from(config: DummyConfig) -> Self {
        let mut provider = Self::new(config.responses);
        provider.label = config.label;
        if config.repeat {
            provider.repeating()
        } else {
            provider
        }
    }
}

#[async_trait]
impl ModelProvider for DummyProvider {
    fn model_id(&self) -> String {
        self.label.clone().unwrap_or_else(|| "dummy".to_string())
    }

    async fn send_prompt(&self, conversation: &[Message]) -> Result<ModelResponse, ProviderError> {
        let latest = conversation
            .last()
            .map(|m| m.content.as_str())
            .unwrap_or("");
        eprintln!(
            "[dummy model] prompt received ({} message(s)); latest:\n{latest}",
            conversation.len()
        );
        self.conversations
            .lock()
            .unwrap()
            .push(conversation.to_vec());
        match self.next_scripted() {
            Some(Scripted::Text(text)) => {
                eprintln!("[dummy model] returning scripted response:\n{text}");
                Ok(ModelResponse {
                    tokens: TokenUsage {
                        input: conversation.iter().map(|m| m.content.len() as u64).sum(),
                        output: text.len() as u64,
                    },
                    text,
                    stop: None,
                })
            }
            Some(Scripted::TransientError(message)) => {
                eprintln!("[dummy model] returning scripted transient error: {message}");
                Err(ProviderError::Transient(message))
            }
            Some(Scripted::TimeoutError(message)) => {
                eprintln!("[dummy model] returning scripted timeout error: {message}");
                Err(ProviderError::Timeout(message))
            }
            None => {
                eprintln!("[dummy model] script exhausted; returning fatal error");
                Err(ProviderError::Fatal(
                    "dummy provider ran out of scripted responses".to_string(),
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bench_core::Role;

    #[tokio::test]
    async fn replays_responses_in_order_then_errors() {
        let provider = DummyProvider::new(["first"]);
        provider.push_response("second");
        let conversation = [Message {
            role: Role::User,
            content: "hi".to_string(),
        }];
        assert_eq!(
            provider.send_prompt(&conversation).await.unwrap().text,
            "first"
        );
        assert_eq!(
            provider.send_prompt(&conversation).await.unwrap().text,
            "second"
        );
        assert!(provider.send_prompt(&conversation).await.is_err());
        assert_eq!(provider.conversations().len(), 3);
    }

    #[tokio::test]
    async fn repeating_provider_cycles() {
        let provider = DummyProvider::new(["a", "b"]).repeating();
        let conversation = [Message {
            role: Role::User,
            content: "hi".to_string(),
        }];
        for expected in ["a", "b", "a", "b", "a"] {
            assert_eq!(
                provider.send_prompt(&conversation).await.unwrap().text,
                expected
            );
        }
    }
}
