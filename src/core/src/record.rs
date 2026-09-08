//! Result records produced by the runner and marshalled by bench-output.
//! They live here so the runner doesn't depend on the output stage.

use std::collections::BTreeMap;

use serde::{Serialize, Serializer};

use crate::Value;
use crate::model::TokenUsage;

#[derive(Debug, Serialize)]
pub struct BenchmarkOutput {
    pub questions: Vec<QuestionOutput>,
}

#[derive(Debug, Serialize)]
pub struct QuestionOutput {
    pub question: String,
    pub difficulty: String,
    /// Only present (as `true`) for deliberately-unanswerable questions,
    /// so answerable questions serialize exactly as before.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub unanswerable: bool,
    /// None only for unanswerable questions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected: Option<Value>,
    /// Keyed by DB ID not query language
    pub dbs: BTreeMap<String, DbOutput>,
}

#[derive(Debug, Serialize)]
pub struct DbOutput {
    pub language: String,
    /// The ground-truth query; None only for unanswerable questions.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correct: Option<String>,
    pub results: Vec<ResultRecord>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultRecord {
    pub model: String,
    pub max_retries: u32,
    pub retries_used: u32,
    pub examples: u32,
    pub skills: bool,
    pub repetition: u32,
    /// The final query produced, whether or not it succeeded.
    pub generated: String,
    /// Every attempt in order, including the final one. Lower retry levels
    /// are derived by cutting this trace, so each attempt carries its own
    /// token/latency cost.
    pub attempts: Vec<Attempt>,
    /// Totals across all attempts.
    pub tokens: TokenUsage,
    pub latency_ms: u64,
    /// The DB's share of `latency_ms` — time the generated queries spent
    /// executing. Subtract it from `latency_ms` for the model's share.
    pub db_latency_ms: u64,
    pub result: RecordResult,
    pub accurate: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attempt {
    /// None when no query was extractable (malformed or UNANSWERABLE
    /// response).
    pub query: Option<String>,
    /// The model's raw reply, recorded only when nothing could be extracted
    /// from it. Without this a malformed run says "no query found" and
    /// discards the evidence, so diagnosing a model that ignores the output
    /// format needs a rerun with logging. Omitted whenever `query` is set, so
    /// results for well-behaved models serialize exactly as before.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response: Option<String>,
    pub tokens: TokenUsage,
    pub latency_ms: u64,
    /// How long this attempt's query spent executing against the DB, part of
    /// `latency_ms`. Zero when the attempt produced no query to run (a
    /// malformed response or an UNANSWERABLE declaration), which is why an
    /// average over query performance has to exclude records that never
    /// executed rather than treating them as instant.
    pub db_latency_ms: u64,
    /// None when this attempt succeeded (only ever the last attempt).
    pub error: Option<String>,
}

/// Sum token usage and latency across an attempt trace — the totals rule
/// shared by the runner and retry-level derivation.
pub fn attempt_totals(attempts: &[Attempt]) -> (TokenUsage, u64, u64) {
    let mut tokens = TokenUsage::default();
    let mut latency_ms = 0;
    let mut db_latency_ms = 0;
    for attempt in attempts {
        tokens.add(attempt.tokens);
        latency_ms += attempt.latency_ms;
        db_latency_ms += attempt.db_latency_ms;
    }
    (tokens, latency_ms, db_latency_ms)
}

#[derive(Debug, Clone)]
pub enum RecordResult {
    /// The coerced result of a successfully executed query.
    Value(Value),
    /// The model correctly declared a deliberately-unanswerable question
    /// UNANSWERABLE. A wrong decline on an answerable question is `Error`
    /// (with "declared UNANSWERABLE" in the attempt trace), so this variant
    /// always means an accurate record.
    Unanswerable,
    /// The query never produced a usable result; details are in the attempt
    /// trace.
    Error,
}

impl Serialize for RecordResult {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            RecordResult::Value(v) => v.serialize(serializer),
            RecordResult::Unanswerable => serializer.serialize_str("unanswerable"),
            RecordResult::Error => serializer.serialize_str("error"),
        }
    }
}
