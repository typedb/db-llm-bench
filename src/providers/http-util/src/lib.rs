//! Shared plumbing for the hand-rolled HTTP model providers: the request
//! skeleton with error classification, the wire message shape, and the
//! `{"error": {"message": ...}}` error-body format — all of which the
//! Anthropic and OpenAI-style APIs have in common.

use std::time::Duration;

use bench_core::{Message, ProviderError, Role};
use serde::{Deserialize, Serialize};

/// Generous ceiling; the runner's own request guard stays the last resort.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(600);

/// Deliberately modest serde default for the providers' `max_tokens`: the
/// expected output is a single query.
pub fn default_max_tokens() -> u32 {
    4096
}

/// The identifier recorded in output records for a provider entry: the
/// configured label, else the model ID.
pub fn model_label(label: Option<&str>, model: &str) -> String {
    label.unwrap_or(model).to_string()
}

pub fn build_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|e| format!("building HTTP client: {e}"))
}

/// A `messages` array entry; both APIs use the same role/content shape.
#[derive(Serialize)]
pub struct WireMessage<'a> {
    pub role: &'static str,
    pub content: &'a str,
}

pub fn wire_messages(conversation: &[Message]) -> Vec<WireMessage<'_>> {
    conversation
        .iter()
        .map(|message| WireMessage {
            role: match message.role {
                Role::User => "user",
                Role::Assistant => "assistant",
            },
            content: &message.content,
        })
        .collect()
}

/// Send the request and return the successful response body. Network
/// failures are Transient (worth a retry) and client-side timeouts are
/// Timeout (retried too, but persistence doesn't abort the run); HTTP
/// failures are classified by status, with the API's own error message
/// extracted from the body where possible.
pub async fn send_for_body(request: reqwest::RequestBuilder) -> Result<String, ProviderError> {
    let response = request.send().await.map_err(|e| {
        if e.is_timeout() {
            ProviderError::Timeout(format!("request timed out: {e}"))
        } else {
            ProviderError::Transient(format!("request failed: {e}"))
        }
    })?;
    let status = response.status();
    if status.is_success() {
        // A read failure on a good status is a network blip mid-body, not a
        // bad response.
        response.text().await.map_err(|e| {
            if e.is_timeout() {
                ProviderError::Timeout(format!("response body timed out: {e}"))
            } else {
                ProviderError::Transient(format!("reading response body: {e}"))
            }
        })
    } else {
        let body = response
            .text()
            .await
            .unwrap_or_else(|_| "<unreadable body>".to_string());
        Err(classify_error(status.as_u16(), &body))
    }
}

/// Decode a successful response body; a failure is Fatal — by this point
/// the status was 2xx, so an undecodable body is a wrong-shaped API.
pub fn decode_json<T: serde::de::DeserializeOwned>(body: &str) -> Result<T, ProviderError> {
    serde_json::from_str(body)
        .map_err(|e| ProviderError::Fatal(format!("undecodable API response: {e}")))
}

fn classify_error(status: u16, body: &str) -> ProviderError {
    let message = parse_error_message(body).unwrap_or_else(|| body.to_string());
    ProviderError::from_http_status(status, message)
}

/// Extract the API's own message from an `{"error": {"message": ...}}`
/// body, whether it arrived with an error status or (as some gateways do)
/// inside an HTTP 200 response.
pub fn parse_error_message(body: &str) -> Option<String> {
    let parsed: ErrorResponse = serde_json::from_str(body).ok()?;
    Some(parsed.error.message)
}

#[derive(Deserialize)]
struct ErrorResponse {
    error: ErrorDetail,
}

#[derive(Deserialize)]
struct ErrorDetail {
    message: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_statuses_by_retryability() {
        let body = r#"{"type":"error","error":{"type":"rate_limit_error","message":"slow down"}}"#;
        assert!(matches!(
            classify_error(429, body),
            ProviderError::Transient(m) if m.contains("slow down")
        ));
        assert!(matches!(
            classify_error(529, "overloaded"),
            ProviderError::Transient(_)
        ));
        assert!(matches!(
            classify_error(500, ""),
            ProviderError::Transient(_)
        ));
        assert!(matches!(
            classify_error(408, ""),
            ProviderError::Transient(_)
        ));
        assert!(matches!(
            classify_error(409, ""),
            ProviderError::Transient(_)
        ));
        assert!(matches!(
            classify_error(400, "bad"),
            ProviderError::Fatal(_)
        ));
        assert!(matches!(classify_error(401, ""), ProviderError::Fatal(_)));
        assert!(matches!(classify_error(404, ""), ProviderError::Fatal(_)));
    }

    #[test]
    fn maps_conversation_roles_onto_the_wire() {
        let conversation = [Message::user("q"), Message::assistant("a")];
        let wire = wire_messages(&conversation);
        assert_eq!(
            serde_json::to_value(&wire).unwrap(),
            serde_json::json!([
                {"role": "user", "content": "q"},
                {"role": "assistant", "content": "a"},
            ])
        );
    }

    #[test]
    fn label_falls_back_to_the_model_id() {
        assert_eq!(model_label(None, "gpt-5"), "gpt-5");
        assert_eq!(
            model_label(Some("gpt-5-via-proxy"), "gpt-5"),
            "gpt-5-via-proxy"
        );
    }
}
