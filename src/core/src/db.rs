use async_trait::async_trait;
use thiserror::Error;

use crate::Value;

/// Why a query produced no usable result. Empty results are NOT errors —
/// they come back as a normal [`Value`].
///
/// Model-fault variants (anything that can't possibly be a correct answer:
/// wrong result shape, syntax error, timeout) are fed back to the LLM and
/// count against its retry budget. `Infrastructure` is the harness's problem
/// — it gets retried with backoff without involving or penalising the model.
#[derive(Debug, Error)]
pub enum QueryError {
    #[error("syntax error: {0}")]
    Syntax(String),
    #[error("query timed out")]
    Timeout,
    #[error("result had the wrong shape: {0}")]
    WrongShape(String),
    /// Connection failures, DB restarts, and anything else that is not the
    /// model's fault.
    #[error("infrastructure error: {0}")]
    Infrastructure(String),
}

impl QueryError {
    /// Whether this error counts against the model's retry budget (and gets
    /// fed back to it), as opposed to being retried at the harness level.
    pub fn is_model_fault(&self) -> bool {
        !matches!(self, QueryError::Infrastructure(_))
    }
}

/// Unified interface implemented by each DB package.
///
/// Implementations are responsible for query safety (read-only transactions
/// and query timeouts where viable) and for coercing driver-native results
/// into the canonical [`Value`].
#[async_trait]
pub trait Database: Send + Sync {
    /// The query language this DB is benchmarked under, matching the
    /// per-language keys in the questions file (e.g. "typeql", "cypher", "sql").
    fn query_language(&self) -> &'static str;

    async fn send_query(&self, query: &str) -> Result<Value, QueryError>;

    /// Establish connectivity, so a down server, bad credentials, or missing
    /// database fails the run up-front instead of at the first query. The
    /// default does nothing; packages that connect lazily should override it
    /// to drive their connection once.
    async fn health_check(&self) -> Result<(), QueryError> {
        Ok(())
    }
}
