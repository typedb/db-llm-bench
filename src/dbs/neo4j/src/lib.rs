//! Neo4j package via the community `neo4rs` crate.
//!
//! Mutation safety: neo4rs 0.8 exposes no read-access-mode, so every query
//! runs in an explicit transaction that is ALWAYS rolled back. A generated
//! write query therefore executes but can never change the dataset — note
//! that unlike the SQL/TypeDB packages it does not produce an error, just
//! results that won't match the expected value.

use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use bench_core::temporal::{canonical_date, canonical_datetime, canonical_datetime_utc};
use bench_core::value::shape_rows;
use bench_core::{Database, QueryError, Value};
use neo4rs::{BoltType, Graph, Txn};
use serde::Deserialize;
use tokio::sync::OnceCell;

/// Client-side ceiling, deliberately longer than the server-side
/// db.transaction.timeout configured in the compose files (180s), so the
/// server cancels first with a clean TransactionTimedOut instead of the
/// client abandoning a transaction that keeps holding locks.
const QUERY_TIMEOUT: Duration = Duration::from_secs(185);
const MAX_ROWS: usize = 10_000;

/// The server-side transaction timeout codes — the model's fault (a
/// pathological query). Other codes merely containing "Timeout" (e.g. lock
/// acquisition) are contention, classified by kind instead.
const TIMEOUT_CODES: [&str; 2] = [
    "Neo.ClientError.Transaction.TransactionTimedOut",
    "Neo.ClientError.Transaction.TransactionTimedOutClientConfiguration",
];

/// Exhausting the transaction memory pool is the query's fault, despite
/// arriving as a TransientError. The runner executes one query at a time, so
/// nothing but the query being run can have consumed the pool — and unlike a
/// genuine transient, retrying is futile: the same query allocates the same
/// way every time, which is exactly how this used to burn the retry budget and
/// then kill the run. Scoring it against the model instead keeps the run alive
/// and records the truth, that the generated query was too expensive to run.
const MEMORY_CODES: [&str; 1] = ["Neo.TransientError.General.MemoryPoolOutOfMemoryError"];

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Neo4jAuth {
    pub username: String,
    pub password: String,
}

impl Default for Neo4jAuth {
    fn default() -> Self {
        Self {
            username: "neo4j".to_string(),
            password: "password".to_string(),
        }
    }
}

pub struct Neo4j {
    config: neo4rs::Config,
    graph: OnceCell<Graph>,
}

impl Neo4j {
    /// Unlike the SQL/TypeDB packages, neo4rs only parses the URI when
    /// connecting, so a malformed URI surfaces as infrastructure at the
    /// first query rather than at startup.
    pub fn new(
        uri: &str,
        database: Option<&str>,
        auth: Option<&Neo4jAuth>,
    ) -> Result<Self, String> {
        let default_auth = Neo4jAuth::default();
        let auth = auth.unwrap_or(&default_auth);
        let mut builder = neo4rs::ConfigBuilder::default()
            .uri(uri)
            .user(&auth.username)
            .password(&auth.password)
            .max_connections(2);
        if let Some(database) = database {
            builder = builder.db(database);
        }
        let config = builder
            .build()
            .map_err(|e| format!("invalid Neo4j config for `{uri}`: {e}"))?;
        Ok(Self {
            config,
            graph: OnceCell::new(),
        })
    }

    /// Connect lazily so constructing the package can't fail; connection
    /// problems surface as infrastructure at the first query.
    async fn graph(&self) -> Result<&Graph, QueryError> {
        self.graph
            .get_or_try_init(|| async { Graph::connect(self.config.clone()).await })
            .await
            .map_err(|e| QueryError::Infrastructure(e.to_string()))
    }
}

#[async_trait]
impl Database for Neo4j {
    fn query_language(&self) -> &'static str {
        "cypher"
    }

    async fn send_query(&self, query: &str) -> Result<Value, QueryError> {
        let graph = self.graph().await?;
        tokio::time::timeout(QUERY_TIMEOUT, run_query(graph, query))
            .await
            .map_err(|_| QueryError::Timeout)?
    }

    /// Connecting alone doesn't validate the database name (it's selected
    /// per-transaction), so run a trivial query through the full path.
    async fn health_check(&self) -> Result<(), QueryError> {
        self.send_query("RETURN 1").await.map(|_| ())
    }
}

async fn run_query(graph: &Graph, query: &str) -> Result<Value, QueryError> {
    let mut txn = graph
        .start_txn()
        .await
        .map_err(|e| QueryError::Infrastructure(e.to_string()))?;
    let result = collect_rows(&mut txn, query).await;
    // Always roll back — this is the mutation guard. On a query error the
    // query's own failure is the informative one; the rollback result only
    // matters when the query succeeded.
    let rollback = txn.rollback().await;
    let value = result?;
    rollback.map_err(|e| QueryError::Infrastructure(format!("rollback failed: {e}")))?;
    Ok(value)
}

async fn collect_rows(txn: &mut Txn, query: &str) -> Result<Value, QueryError> {
    let mut stream = txn
        .execute(neo4rs::query(query))
        .await
        .map_err(map_neo4j_error)?;
    let mut raw_names: Vec<String> = Vec::new();
    let mut fields: Vec<String> = Vec::new();
    let mut table: Vec<Vec<Value>> = Vec::new();
    while let Some(row) = stream.next(txn.handle()).await.map_err(map_neo4j_error)? {
        if table.len() >= MAX_ROWS {
            return Err(QueryError::WrongShape(format!(
                "result exceeded {MAX_ROWS} rows"
            )));
        }
        let cells: HashMap<String, BoltType> = row
            .to_strict()
            .map_err(|e| QueryError::WrongShape(format!("undecodable row: {e}")))?;
        if raw_names.is_empty() {
            raw_names = cells.keys().cloned().collect();
            // HashMap order is arbitrary; sorted keys keep row shaping
            // deterministic.
            raw_names.sort();
            fields = raw_names.iter().map(|name| normalize_field(name)).collect();
        }
        let mut row_values = Vec::with_capacity(fields.len());
        for (field, raw_name) in fields.iter().zip(&raw_names) {
            let cell = cells.get(raw_name).ok_or_else(|| {
                QueryError::Infrastructure(format!("row missing column `{raw_name}`"))
            })?;
            row_values.push(coerce_bolt(field, cell)?);
        }
        table.push(row_values);
    }
    Ok(shape_rows(&fields, table))
}

/// Cypher derives column names from expression text unless aliased, so an
/// unaliased projection like `RETURN c.brand` is named "c.brand". Strip
/// plain property-access prefixes so those match the same field names as
/// the other DB packages; aggregates and aliases pass through untouched.
/// (Two projections of the same property from different entities would
/// collide after stripping — such queries need aliases regardless.)
fn normalize_field(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((entity, property)) if is_identifier(entity) && is_identifier(property) => {
            property.to_string()
        }
        _ => name.to_string(),
    }
}

fn is_identifier(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|c| c.is_alphanumeric() || c == '_')
}

/// Scalars, lists, maps, and temporals coerce; graph entities (nodes,
/// relationships, paths) are a wrong shape, and the message tells the model
/// how to fix it.
fn coerce_bolt(name: &str, value: &BoltType) -> Result<Value, QueryError> {
    Ok(match value {
        BoltType::Null(_) => Value::Null,
        BoltType::Boolean(b) => Value::Bool(b.value),
        BoltType::Integer(i) => Value::Int(i.value),
        BoltType::Float(f) => Value::Float(f.value),
        BoltType::String(s) => Value::String(s.value.clone()),
        BoltType::List(list) => Value::List(
            list.value
                .iter()
                .map(|item| coerce_bolt(name, item))
                .collect::<Result<_, _>>()?,
        ),
        BoltType::Map(map) => Value::Object(
            map.value
                .iter()
                .map(|(key, item)| Ok((key.value.clone(), coerce_bolt(name, item)?)))
                .collect::<Result<_, _>>()?,
        ),
        BoltType::Date(date) => {
            let date: chrono::NaiveDate =
                date.try_into().map_err(|_| temporal_error(name, "date"))?;
            Value::String(canonical_date(date))
        }
        BoltType::LocalDateTime(datetime) => {
            let datetime: chrono::NaiveDateTime = datetime
                .try_into()
                .map_err(|_| temporal_error(name, "datetime"))?;
            Value::String(canonical_datetime(datetime))
        }
        BoltType::DateTime(datetime) => {
            let datetime: chrono::DateTime<chrono::FixedOffset> = datetime
                .try_into()
                .map_err(|_| temporal_error(name, "zoned datetime"))?;
            Value::String(canonical_datetime_utc(datetime.with_timezone(&chrono::Utc)))
        }
        other => {
            return Err(QueryError::WrongShape(format!(
                "variable `{name}` is bound to {}, which is not a supported value; \
                 return properties or scalar values instead",
                bolt_kind(other)
            )));
        }
    })
}

fn temporal_error(name: &str, kind: &str) -> QueryError {
    QueryError::WrongShape(format!("variable `{name}`: out-of-range {kind}"))
}

fn bolt_kind(value: &BoltType) -> &'static str {
    match value {
        BoltType::Node(_) => "a node",
        BoltType::Relation(_) | BoltType::UnboundedRelation(_) => "a relationship",
        BoltType::Path(_) => "a path",
        BoltType::Point2D(_) | BoltType::Point3D(_) => "a point",
        BoltType::Bytes(_) => "bytes",
        BoltType::Duration(_) => "a duration",
        BoltType::Time(_) | BoltType::LocalTime(_) => "a time value",
        BoltType::DateTimeZoneId(_) => "a zone-id datetime",
        _ => "an unsupported value",
    }
}

fn map_neo4j_error(error: neo4rs::Error) -> QueryError {
    let display = error.to_string();
    match error {
        neo4rs::Error::Neo4j(e) => classify_neo4j(e.code(), &display),
        // A failure that arrives mid-stream — during PULL, once rows are
        // already flowing — is not surfaced as `Error::Neo4j`: neo4rs wraps it
        // in `Error::UnexpectedMessage` with the Bolt map Debug-formatted into
        // the string, so the code is only reachable as text. Without this, a
        // query that dies partway through streaming (a transaction timeout is
        // the common one, since the server kills it precisely when it has been
        // producing rows for too long) looks like an unclassifiable transport
        // fault and aborts the entire run instead of scoring as one bad query.
        _ => match code_from_message(&display) {
            Some(code) => classify_neo4j(code, &display),
            None => QueryError::Infrastructure(display),
        },
    }
}

/// Recover a `Neo.X.Y.Z` status code from an error rendered as text.
fn code_from_message(display: &str) -> Option<&str> {
    let rest = &display[display.find("Neo.")?..];
    let end = rest
        .find(|c: char| !c.is_ascii_alphanumeric() && c != '.')
        .unwrap_or(rest.len());
    let code = rest[..end].trim_end_matches('.');
    // Four segments or it isn't a status code — guards against a stray "Neo."
    // elsewhere in a message.
    (code.split('.').count() == 4).then_some(code)
}

/// Client-error codes describe the query — the model's fault — except the
/// security/session/protocol kinds, which are the harness's problem.
/// Transient, database, and unknown errors are infrastructure. Server-side
/// transaction timeouts are recognised by exact code.
///
/// Classified from the code string rather than neo4rs's `Neo4jErrorKind`,
/// which is `pub(crate)`-constructed and so unavailable on the mid-stream path
/// above. One implementation for both paths is the point: the same failure has
/// to score the same way whether it arrives before or during streaming.
fn classify_neo4j(code: &str, display: &str) -> QueryError {
    if TIMEOUT_CODES.contains(&code) {
        return QueryError::Timeout;
    }
    if MEMORY_CODES.contains(&code) {
        return QueryError::Syntax(display.to_string());
    }
    // Mirrors neo4rs's own `adjust_code`: two transient transaction codes are
    // rewritten to their client equivalents before classification.
    let code = match code {
        "Neo.TransientError.Transaction.LockClientStopped" => {
            "Neo.ClientError.Transaction.LockClientStopped"
        }
        "Neo.TransientError.Transaction.Terminated" => "Neo.ClientError.Transaction.Terminated",
        other => other,
    };
    let mut parts = code.split('.').skip(1);
    let (class, subclass, kind) = (parts.next(), parts.next(), parts.next());
    match (class, subclass, kind) {
        // Security, ProtocolViolation, FatalDiscovery, TransactionTerminated
        // and SessionExpired — the harness's problem, not the query's.
        (Some("ClientError"), Some("Security"), _)
        | (Some("ClientError"), Some("Request"), _)
        | (Some("ClientError"), Some("Database"), Some("DatabaseNotFound"))
        | (Some("ClientError"), Some("Transaction"), Some("Terminated"))
        | (Some("ClientError"), Some("Cluster"), Some("NotALeader"))
        | (Some("ClientError"), Some("General"), Some("ForbiddenOnReadOnlyDatabase")) => {
            QueryError::Infrastructure(display.to_string())
        }
        // Every other client error describes the query itself.
        (Some("ClientError"), Some(_), _) => QueryError::Syntax(display.to_string()),
        _ => QueryError::Infrastructure(display.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coerces_scalars_lists_and_temporals() {
        assert_eq!(
            coerce_bolt("x", &BoltType::from(42i64)).unwrap(),
            Value::Int(42)
        );
        assert_eq!(
            coerce_bolt("x", &BoltType::from(2.5f64)).unwrap(),
            Value::Float(2.5)
        );
        assert_eq!(
            coerce_bolt("x", &BoltType::from("ka")).unwrap(),
            Value::String("ka".to_string())
        );
        assert_eq!(
            coerce_bolt("x", &BoltType::from(vec![1i64, 2i64])).unwrap(),
            Value::List(vec![Value::Int(1), Value::Int(2)])
        );

        let date = chrono::NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        assert_eq!(
            coerce_bolt("x", &BoltType::Date(date.into())).unwrap(),
            Value::String("2024-01-15".to_string())
        );
        let datetime =
            chrono::NaiveDateTime::new(date, chrono::NaiveTime::from_hms_opt(10, 30, 0).unwrap());
        assert_eq!(
            coerce_bolt("x", &BoltType::LocalDateTime(datetime.into())).unwrap(),
            Value::String("2024-01-15T10:30:00.000000000".to_string())
        );
    }

    #[test]
    fn error_kinds_decide_fault_ownership() {
        // Statement errors (syntax, unknown labels) are the model's fault.
        assert!(matches!(
            classify_neo4j("Neo.ClientError.Statement.SyntaxError", "Invalid input"),
            QueryError::Syntax(_)
        ));
        // Auth failures are infrastructure, not the model's.
        assert!(matches!(
            classify_neo4j("Neo.ClientError.Security.Unauthorized", "unauthorized"),
            QueryError::Infrastructure(_)
        ));
        // Transient server conditions are infrastructure (harness backoff).
        assert!(matches!(
            classify_neo4j("Neo.TransientError.Database.DatabaseUnavailable", "down"),
            QueryError::Infrastructure(_)
        ));
        // ...but exhausting the transaction memory pool is the query's fault,
        // even though Neo4j files it under TransientError. Retrying it is
        // futile and used to burn the retry budget and then kill the run.
        assert!(matches!(
            classify_neo4j(
                "Neo.TransientError.General.MemoryPoolOutOfMemoryError",
                "oom"
            ),
            QueryError::Syntax(_)
        ));
        // Both server-side transaction timeout codes map to the model-fault
        // timeout. The ClientConfiguration variant is what a query that
        // outruns the transaction timeout actually returns.
        for code in TIMEOUT_CODES {
            assert!(matches!(
                classify_neo4j(code, "timed out"),
                QueryError::Timeout
            ));
        }
        // Other timeout-ish codes are contention, not a pathological query.
        assert!(matches!(
            classify_neo4j(
                "Neo.TransientError.Transaction.LockAcquisitionTimeout",
                "lock timeout"
            ),
            QueryError::Infrastructure(_)
        ));
        // A terminated transaction is the harness's problem even though the
        // code lives under ClientError.
        assert!(matches!(
            classify_neo4j("Neo.TransientError.Transaction.Terminated", "terminated"),
            QueryError::Infrastructure(_)
        ));
    }

    /// The regression behind `aborted on DB neo4j: ... unexpected response for
    /// PULL`: a timeout that lands mid-stream reaches us as text, and used to
    /// abort the run rather than scoring as one timed-out query.
    #[test]
    fn mid_stream_failures_are_classified_from_the_rendered_code() {
        let pull = "unexpected response for PULL: Ok(Failure(Failure { metadata: BoltMap { \
             value: {BoltString { value: \"code\" }: String(BoltString { value: \
             \"Neo.ClientError.Transaction.TransactionTimedOutClientConfiguration\" }), \
             BoltString { value: \"message\" }: String(BoltString { value: \"The transaction \
             has been terminated.\" })} } }))";
        assert_eq!(
            code_from_message(pull),
            Some("Neo.ClientError.Transaction.TransactionTimedOutClientConfiguration")
        );
        assert!(matches!(
            classify_neo4j(code_from_message(pull).unwrap(), pull),
            QueryError::Timeout
        ));
        // A syntax error surfacing the same way is still the model's fault.
        assert!(matches!(
            classify_neo4j(
                code_from_message("Failure { code: \"Neo.ClientError.Statement.SyntaxError\" }")
                    .unwrap(),
                "boom"
            ),
            QueryError::Syntax(_)
        ));
        // Nothing code-shaped in the text: genuinely a transport fault.
        assert_eq!(code_from_message("connection reset by peer"), None);
        assert_eq!(code_from_message("Neo.Client"), None);
    }

    #[test]
    fn property_access_names_normalize_to_bare_fields() {
        assert_eq!(normalize_field("c.brand"), "brand");
        assert_eq!(normalize_field("car_1.model"), "model");
        // Aliases, bare names, and aggregates pass through untouched.
        assert_eq!(normalize_field("brand"), "brand");
        assert_eq!(normalize_field("avg(c.price)"), "avg(c.price)");
        assert_eq!(normalize_field("count(*)"), "count(*)");
    }

    #[test]
    fn config_builds_with_and_without_database() {
        assert!(Neo4j::new("bolt://localhost:7687", None, None).is_ok());
        assert!(Neo4j::new("bolt://localhost:7687", Some("bench"), None).is_ok());
    }

    #[tokio::test]
    #[ignore = "requires a running Neo4j server (NEO4J_URI, NEO4J_PASSWORD)"]
    async fn queries_a_live_server() {
        let uri =
            std::env::var("NEO4J_URI").unwrap_or_else(|_| "bolt://localhost:7687".to_string());
        let auth = Neo4jAuth {
            username: "neo4j".to_string(),
            password: std::env::var("NEO4J_PASSWORD").unwrap_or_else(|_| "password".to_string()),
        };
        let db = Neo4j::new(&uri, None, Some(&auth)).unwrap();
        let value = db.send_query("RETURN 1 + 1 AS result").await.unwrap();
        assert_eq!(value, Value::Int(2));
        // Writes execute but are rolled back: the node must not survive.
        db.send_query("CREATE (n:BenchRollbackProbe)")
            .await
            .unwrap();
        let count = db
            .send_query("MATCH (n:BenchRollbackProbe) RETURN count(n)")
            .await
            .unwrap();
        assert_eq!(count, Value::Int(0));
    }
}
