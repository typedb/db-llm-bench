//! Shared types for the benchmark framework: the canonical result value,
//! the `Database` and `ModelProvider` traits that DB/provider packages
//! implement, and the question definitions.

pub mod db;
pub mod model;
pub mod question;
pub mod record;
pub mod temporal;
pub mod value;

pub use db::{Database, QueryError};
pub use model::{Message, ModelProvider, ModelResponse, ProviderError, Role, TokenUsage};
pub use question::{Question, QuestionFile};
pub use record::{
    Attempt, BenchmarkOutput, DbOutput, QuestionOutput, RecordResult, ResultRecord, attempt_totals,
};
pub use value::Value;
