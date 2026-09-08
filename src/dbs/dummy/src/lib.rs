//! Dummy DB for testing. Scripted results are returned first, in order;
//! after that, the query itself is parsed as JSON and echoed back as the
//! result — so a "query" in the dummy language is just the JSON of the
//! result it produces, and invalid JSON exercises the syntax-error/retry
//! path. Every received query is recorded for assertions.

use std::collections::VecDeque;
use std::sync::Mutex;

use async_trait::async_trait;
use bench_core::{Database, QueryError, Value};

pub struct DummyDb {
    language: &'static str,
    scripted: Mutex<VecDeque<Result<Value, QueryError>>>,
    queries: Mutex<Vec<String>>,
}

impl DummyDb {
    pub fn new() -> Self {
        Self {
            language: "dummy",
            scripted: Mutex::new(VecDeque::new()),
            queries: Mutex::new(Vec::new()),
        }
    }

    /// Masquerade as another query language (e.g. "sql") in tests.
    pub fn with_language(mut self, language: &'static str) -> Self {
        self.language = language;
        self
    }

    /// Queue a result to be returned ahead of the JSON-echo fallback.
    pub fn script(&self, result: Result<Value, QueryError>) {
        self.scripted.lock().unwrap().push_back(result);
    }

    /// Every query received so far, in order.
    pub fn queries(&self) -> Vec<String> {
        self.queries.lock().unwrap().clone()
    }
}

impl Default for DummyDb {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Database for DummyDb {
    fn query_language(&self) -> &'static str {
        self.language
    }

    async fn send_query(&self, query: &str) -> Result<Value, QueryError> {
        eprintln!("[dummy db] query received: {query}");
        self.queries.lock().unwrap().push(query.to_string());
        if let Some(scripted) = self.scripted.lock().unwrap().pop_front() {
            match &scripted {
                Ok(value) => eprintln!("[dummy db] returning scripted result: {value:?}"),
                Err(error) => eprintln!("[dummy db] returning scripted error: {error}"),
            }
            return scripted;
        }
        match serde_json::from_str::<serde_json::Value>(query) {
            Ok(json) => {
                let value = Value::from(json);
                eprintln!("[dummy db] echoing query as JSON result: {value:?}");
                Ok(value)
            }
            Err(e) => {
                eprintln!("[dummy db] query is not valid JSON; returning syntax error: {e}");
                Err(QueryError::Syntax(e.to_string()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn echoes_query_as_json_result() {
        let db = DummyDb::new();
        assert_eq!(db.send_query("3").await.unwrap(), Value::Int(3));
        assert_eq!(
            db.send_query(r#"[1, "a"]"#).await.unwrap(),
            Value::List(vec![Value::Int(1), Value::String("a".into())])
        );
    }

    #[tokio::test]
    async fn invalid_json_is_a_syntax_error() {
        let db = DummyDb::new();
        assert!(matches!(
            db.send_query("SELECT nope").await,
            Err(QueryError::Syntax(_))
        ));
    }

    #[tokio::test]
    async fn scripted_results_take_priority_and_queries_are_recorded() {
        let db = DummyDb::new();
        db.script(Err(QueryError::Timeout));
        db.script(Ok(Value::Int(7)));
        assert!(matches!(db.send_query("3").await, Err(QueryError::Timeout)));
        assert_eq!(db.send_query("3").await.unwrap(), Value::Int(7));
        assert_eq!(db.send_query("3").await.unwrap(), Value::Int(3));
        assert_eq!(db.queries().len(), 3);
    }
}
