use std::collections::BTreeMap;

use serde::Deserialize;

use crate::Value;

#[derive(Debug, Deserialize)]
pub struct QuestionFile {
    pub questions: Vec<Question>,
}

/// `deny_unknown_fields` so a typo'd or unexpected key is a parse error
/// rather than silently ignored.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Question {
    pub question: String,
    pub difficulty: String,
    /// Deliberately unanswerable against the schema: the only correct
    /// response is the UNANSWERABLE token, so there is no expected value
    /// and there are no ground-truth queries.
    #[serde(default)]
    pub unanswerable: bool,
    /// None only for unanswerable questions (enforced when the question
    /// file is loaded).
    #[serde(default)]
    pub expected: Option<Value>,
    /// Whether the order of a top-level list result is part of correctness
    /// (i.e. the question demands an ordering). Defaults to unordered: rows
    /// compare as a bag. Nested lists always compare ordered, as tuples.
    #[serde(default)]
    pub ordered: bool,
    /// Correct query per language key ("typeql", "sql", "cypher", ...).
    /// Empty only for unanswerable questions.
    #[serde(default)]
    pub queries: BTreeMap<String, String>,
    /// Expected values for stores whose data genuinely differs from the
    /// baseline `expected`, keyed by the config's DB id (not the query
    /// language: the divergence is a property of the store's contents, and
    /// two stores can share a language).
    ///
    /// Reactome publishes its relational dump and its graph dump as separately
    /// built artifacts of the same release, and they disagree — 126,230
    /// `InstanceEdit` rows against 160,392 nodes — so a question that counts
    /// them has no single true answer.
    ///
    /// Use this only for a divergence traced to the data and confirmed by an
    /// independent count, and say which in the question's `divergence` note.
    /// A reference query that disagrees with the others is overwhelmingly
    /// likely to be wrong: the shared `expected` is what catches that, and an
    /// override silences it. Keep the default path a single shared value.
    #[serde(default)]
    pub expected_by_db: BTreeMap<String, Value>,
    /// Why this question's stores disagree. Required alongside
    /// `expected_by_db`, and unused otherwise.
    #[serde(default)]
    pub divergence: Option<String>,
}

impl Question {
    /// The value `db_id` is scored against: its override when the stores
    /// disagree, otherwise the shared `expected`.
    pub fn expected_for(&self, db_id: &str) -> Option<&Value> {
        self.expected_by_db.get(db_id).or(self.expected.as_ref())
    }
}
