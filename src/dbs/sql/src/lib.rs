//! SQL package: Postgres or MySQL via sqlx ("sql" is the generic language ID;
//! the engine follows the URL scheme, so a dataset selects one by its `url`
//! alone). Mutation safety is layered the same way on both: connect as a
//! SELECT-only role (the hard guarantee — see databases/mysql/roles.sql; a
//! Postgres deployment needs the equivalent GRANT SELECT-only role) with a
//! read-only session default and server-side
//! statement timeout as defense-in-depth, since a generated SET can disable
//! session defaults but cannot escape grants.

use std::time::Duration;

use async_trait::async_trait;
use bench_core::temporal::{canonical_date, canonical_datetime, canonical_datetime_utc};
use bench_core::value::shape_rows;
use bench_core::{Database, QueryError, Value};
use futures::StreamExt;
use rust_decimal::prelude::ToPrimitive;
use serde::Deserialize;
use sqlx::mysql::{MySqlColumn, MySqlConnectOptions, MySqlPool, MySqlPoolOptions, MySqlRow};
use sqlx::postgres::{PgColumn, PgConnectOptions, PgPool, PgPoolOptions, PgRow};
use sqlx::{Column, Executor, Row, TypeInfo};
use tokio::sync::OnceCell;

/// Client-side ceiling, deliberately longer than the server-side
/// statement timeout so the server cancels first — that path yields a clean
/// cancellation error on a still-healthy connection, instead of the client
/// dropping the stream mid-query.
const QUERY_TIMEOUT: Duration = Duration::from_secs(185);
const STATEMENT_TIMEOUT: &str = "180s";
/// The same 180s cap as Postgres's `statement_timeout`, in the milliseconds
/// MySQL wants. Note MySQL applies `max_execution_time` to SELECTs only —
/// harmless here, since the read-only role admits nothing else.
const MYSQL_MAX_EXECUTION_TIME_MS: u64 = 180_000;
const MAX_ROWS: usize = 10_000;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SqlAuth {
    pub username: String,
    pub password: String,
}

/// Which engine a `Sql` talks to, chosen by URL scheme and fixed at
/// construction. Each variant owns its own pool: sqlx's row and type-info
/// types are per-driver, and the coercion tables genuinely differ, so there is
/// nothing useful to share below this point.
enum Backend {
    Postgres {
        options: PgConnectOptions,
        pool: OnceCell<PgPool>,
    },
    MySql {
        options: MySqlConnectOptions,
        pool: OnceCell<MySqlPool>,
    },
}

pub struct Sql {
    backend: Backend,
}

impl Sql {
    /// Validates the URL eagerly so a config typo fails at startup. The scheme
    /// picks the engine (`postgres`/`postgresql` vs `mysql`/`mariadb`); the URL
    /// may embed credentials and a database (mysql://user:pass@host/db) and
    /// explicit `database`/`auth` config overrides them.
    pub fn new(url: &str, database: Option<&str>, auth: Option<&SqlAuth>) -> Result<Self, String> {
        let scheme = url.split("://").next().unwrap_or_default();
        let backend = match scheme {
            "postgres" | "postgresql" => {
                let mut options: PgConnectOptions = url
                    .parse()
                    .map_err(|e| format!("invalid Postgres URL `{url}`: {e}"))?;
                if let Some(database) = database {
                    options = options.database(database);
                }
                if let Some(auth) = auth {
                    options = options.username(&auth.username).password(&auth.password);
                }
                options = options.options([
                    ("default_transaction_read_only", "on"),
                    ("statement_timeout", STATEMENT_TIMEOUT),
                ]);
                Backend::Postgres {
                    options,
                    pool: OnceCell::new(),
                }
            }
            "mysql" | "mariadb" => {
                let mut options: MySqlConnectOptions = url
                    .parse()
                    .map_err(|e| format!("invalid MySQL URL `{url}`: {e}"))?;
                if let Some(database) = database {
                    options = options.database(database);
                }
                if let Some(auth) = auth {
                    options = options.username(&auth.username).password(&auth.password);
                }
                Backend::MySql {
                    options,
                    pool: OnceCell::new(),
                }
            }
            _ => {
                return Err(format!(
                    "unsupported SQL URL `{url}`: expected a postgres:// or mysql:// scheme"
                ));
            }
        };
        Ok(Self { backend })
    }
}

#[async_trait]
impl Database for Sql {
    fn query_language(&self) -> &'static str {
        "sql"
    }

    async fn send_query(&self, query: &str) -> Result<Value, QueryError> {
        // Connect lazily; the first connection validates host, auth, and
        // database existence, surfacing config problems as infrastructure.
        let run = async {
            match &self.backend {
                Backend::Postgres { options, pool } => {
                    let pool = pool
                        .get_or_try_init(|| async {
                            PgPoolOptions::new()
                                .max_connections(2)
                                .connect_with(options.clone())
                                .await
                        })
                        .await
                        .map_err(|e| QueryError::Infrastructure(e.to_string()))?;
                    run_query_postgres(pool, query).await
                }
                Backend::MySql { options, pool } => {
                    let pool = pool
                        .get_or_try_init(|| async { connect_mysql(options.clone()).await })
                        .await
                        .map_err(|e| QueryError::Infrastructure(e.to_string()))?;
                    run_query_mysql(pool, query).await
                }
            }
        };
        tokio::time::timeout(QUERY_TIMEOUT, run)
            .await
            .map_err(|_| QueryError::Timeout)?
    }

    /// Driving the lazy pool validates the host, credentials, and database
    /// (all part of the connect options).
    async fn health_check(&self) -> Result<(), QueryError> {
        match &self.backend {
            Backend::Postgres { options, pool } => pool
                .get_or_try_init(|| async {
                    PgPoolOptions::new()
                        .max_connections(2)
                        .connect_with(options.clone())
                        .await
                })
                .await
                .map(|_| ()),
            Backend::MySql { options, pool } => pool
                .get_or_try_init(|| async { connect_mysql(options.clone()).await })
                .await
                .map(|_| ()),
        }
        .map_err(|e| QueryError::Infrastructure(e.to_string()))
    }
}

/// MySQL has no connect-time equivalent of Postgres's `options` parameter, so
/// the session guards are applied per connection as the pool opens them.
/// `transaction_read_only` is the counterpart of Postgres's
/// `default_transaction_read_only`: defense-in-depth over the SELECT-only
/// grant, not a substitute for it.
async fn connect_mysql(options: MySqlConnectOptions) -> Result<MySqlPool, sqlx::Error> {
    MySqlPoolOptions::new()
        .max_connections(2)
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                conn.execute(
                    format!(
                        "SET SESSION max_execution_time = {MYSQL_MAX_EXECUTION_TIME_MS}, \
                         transaction_read_only = ON"
                    )
                    .as_str(),
                )
                .await?;
                Ok(())
            })
        })
        .connect_with(options)
        .await
}

/// The row-accumulation half of a query is identical across engines; only the
/// per-cell coercion and error classification differ, so those are passed in.
macro_rules! run_query_fn {
    ($name:ident, $pool:ty, $coerce:ident, $classify:ident) => {
        async fn $name(pool: &$pool, query: &str) -> Result<Value, QueryError> {
            let mut stream = sqlx::query(query).fetch(pool);
            let mut columns: Vec<String> = Vec::new();
            let mut table: Vec<Vec<Value>> = Vec::new();
            while let Some(row) = stream.next().await {
                let row = row.map_err(|e| map_sqlx_error(e, $classify))?;
                if table.len() >= MAX_ROWS {
                    return Err(QueryError::WrongShape(format!(
                        "result exceeded {MAX_ROWS} rows"
                    )));
                }
                if columns.is_empty() {
                    columns = row
                        .columns()
                        .iter()
                        .map(|column| column.name().to_string())
                        .collect();
                }
                table.push(
                    row.columns()
                        .iter()
                        .map(|column| $coerce(&row, column))
                        .collect::<Result<Vec<Value>, QueryError>>()?,
                );
            }
            Ok(shape_rows(&columns, table))
        }
    };
}

run_query_fn!(
    run_query_postgres,
    PgPool,
    coerce_column_postgres,
    classify_postgres_error
);
run_query_fn!(
    run_query_mysql,
    MySqlPool,
    coerce_column_mysql,
    classify_mysql_error
);

/// Decodes one cell as `Option<$t>` and maps the payload, so SQL NULL becomes
/// `Value::Null` uniformly without each arm restating it.
macro_rules! cell {
    ($row:expr, $index:expr, $on_error:expr, $t:ty, $map:expr) => {
        $row.try_get::<Option<$t>, _>($index)
            .map_err($on_error)?
            .map($map)
    };
}

fn coerce_column_postgres(row: &PgRow, column: &PgColumn) -> Result<Value, QueryError> {
    let name = column.name();
    let type_name = column.type_info().name();
    let index = column.ordinal();
    let decode_error =
        |e: sqlx::Error| QueryError::WrongShape(format!("column `{name}` ({type_name}): {e}"));

    let value = match type_name {
        "BOOL" => cell!(row, index, decode_error, bool, Value::Bool),
        "INT2" => cell!(row, index, decode_error, i16, |v| Value::Int(v as i64)),
        "INT4" => cell!(row, index, decode_error, i32, |v| Value::Int(v as i64)),
        "INT8" => cell!(row, index, decode_error, i64, Value::Int),
        "FLOAT4" => cell!(row, index, decode_error, f32, |v| Value::Float(v as f64)),
        "FLOAT8" => cell!(row, index, decode_error, f64, Value::Float),
        // Lossy into f64 by design; the canonical Value has no decimal type.
        "NUMERIC" => cell!(row, index, decode_error, rust_decimal::Decimal, |v| v
            .to_f64()
            .map(Value::Float)
            .unwrap_or_else(|| Value::String(v.to_string()))),
        "TEXT" | "VARCHAR" | "BPCHAR" | "CHAR" | "NAME" => {
            cell!(row, index, decode_error, String, Value::String)
        }
        "DATE" => cell!(row, index, decode_error, chrono::NaiveDate, |v| {
            Value::String(canonical_date(v))
        }),
        "TIMESTAMP" => cell!(row, index, decode_error, chrono::NaiveDateTime, |v| {
            Value::String(canonical_datetime(v))
        }),
        "TIMESTAMPTZ" => cell!(
            row,
            index,
            decode_error,
            chrono::DateTime<chrono::Utc>,
            |v| Value::String(canonical_datetime_utc(v))
        ),
        "JSON" | "JSONB" => cell!(row, index, decode_error, serde_json::Value, Value::from),
        other => return Err(unsupported_type(name, other)),
    };
    Ok(value.unwrap_or(Value::Null))
}

fn coerce_column_mysql(row: &MySqlRow, column: &MySqlColumn) -> Result<Value, QueryError> {
    let name = column.name();
    let type_name = column.type_info().name();
    let index = column.ordinal();
    let decode_error =
        |e: sqlx::Error| QueryError::WrongShape(format!("column `{name}` ({type_name}): {e}"));

    let value = match type_name {
        // MySQL has no distinct boolean; sqlx reports TINYINT(1) as BOOLEAN and
        // any wider TINYINT as an integer, which is the closest honest split.
        "BOOLEAN" => cell!(row, index, decode_error, bool, Value::Bool),
        "TINYINT" => cell!(row, index, decode_error, i8, |v| Value::Int(v as i64)),
        "SMALLINT" => cell!(row, index, decode_error, i16, |v| Value::Int(v as i64)),
        "MEDIUMINT" | "INT" => cell!(row, index, decode_error, i32, |v| Value::Int(v as i64)),
        "BIGINT" => cell!(row, index, decode_error, i64, Value::Int),
        // Unsigned columns are pervasive in real MySQL schemas (Reactome keys
        // every table on `int unsigned`), and sqlx refuses to decode them into
        // a signed type, so each width needs its own arm.
        "TINYINT UNSIGNED" => cell!(row, index, decode_error, u8, |v| Value::Int(v as i64)),
        "SMALLINT UNSIGNED" => cell!(row, index, decode_error, u16, |v| Value::Int(v as i64)),
        "MEDIUMINT UNSIGNED" | "INT UNSIGNED" => {
            cell!(row, index, decode_error, u32, |v| Value::Int(v as i64))
        }
        // The one width that can genuinely overflow i64; fall back to the exact
        // decimal string rather than wrapping into a negative.
        "BIGINT UNSIGNED" => cell!(row, index, decode_error, u64, |v| i64::try_from(v)
            .map(Value::Int)
            .unwrap_or_else(|_| Value::String(v.to_string()))),
        "FLOAT" => cell!(row, index, decode_error, f32, |v| Value::Float(v as f64)),
        "DOUBLE" => cell!(row, index, decode_error, f64, Value::Float),
        // Lossy into f64 by design, as with Postgres NUMERIC. Worth knowing
        // that MySQL returns DECIMAL from SUM()/AVG() over integer columns, so
        // this arm carries ordinary aggregate results, not just decimal columns.
        "DECIMAL" => cell!(row, index, decode_error, rust_decimal::Decimal, |v| v
            .to_f64()
            .map(Value::Float)
            .unwrap_or_else(|| Value::String(v.to_string()))),
        "CHAR" | "VARCHAR" | "TINYTEXT" | "TEXT" | "MEDIUMTEXT" | "LONGTEXT" | "ENUM" | "SET" => {
            cell!(row, index, decode_error, String, Value::String)
        }
        "DATE" => cell!(row, index, decode_error, chrono::NaiveDate, |v| {
            Value::String(canonical_date(v))
        }),
        // Both are naive on the wire: MySQL stores TIMESTAMP as UTC but returns
        // it in the session time zone, so there is no offset to carry and
        // canonical_datetime_utc would be asserting a zone we weren't given.
        "DATETIME" | "TIMESTAMP" => {
            cell!(row, index, decode_error, chrono::NaiveDateTime, |v| {
                Value::String(canonical_datetime(v))
            })
        }
        "JSON" => cell!(row, index, decode_error, serde_json::Value, Value::from),
        // `SELECT NULL` and friends come back with no type at all. Postgres
        // infers one, MySQL does not, so it needs an explicit arm.
        "NULL" => None,
        other => return Err(unsupported_type(name, other)),
    };
    Ok(value.unwrap_or(Value::Null))
}

fn unsupported_type(column: &str, type_name: &str) -> QueryError {
    QueryError::WrongShape(format!(
        "column `{column}` has unsupported type {type_name}; \
         select numbers, text, booleans, dates, or json instead"
    ))
}

fn map_sqlx_error(
    error: sqlx::Error,
    classify: fn(&dyn sqlx::error::DatabaseError) -> QueryError,
) -> QueryError {
    match error {
        sqlx::Error::Database(db) => classify(db.as_ref()),
        // Decode failures mean the query selected something outside the
        // coercible types — the model can fix that.
        error @ (sqlx::Error::ColumnDecode { .. } | sqlx::Error::Decode(_)) => {
            QueryError::WrongShape(error.to_string())
        }
        error => QueryError::Infrastructure(error.to_string()),
    }
}

fn classify_postgres_error(db: &dyn sqlx::error::DatabaseError) -> QueryError {
    let code = db.code();
    let code = code.as_deref();
    if code == Some("57014") {
        return QueryError::Timeout;
    }
    classify_sqlstate(code, db.message())
}

/// MySQL error numbers that mean the *server* is in trouble, not the query.
/// These all report SQLSTATE HY000, the same as ordinary query-authoring
/// mistakes, so they have to be named explicitly — see [`classify_mysql_error`].
const MYSQL_INFRASTRUCTURE: [u16; 10] = [
    3,    // ER_ERROR_ON_WRITE: a temp-file write failed, typically a full disk
    1021, // ER_DISK_FULL
    1030, // ER_GET_ERRNO: "Got error N from storage engine" — MyISAM's disk path
    1041, // ER_OUT_OF_RESOURCES
    1053, // ER_SERVER_SHUTDOWN
    1105, // ER_UNKNOWN_ERROR
    1114, // ER_RECORD_FILE_FULL
    1205, // ER_LOCK_WAIT_TIMEOUT
    1206, // ER_LOCK_TABLE_FULL
    126,  // ER_NOT_KEYFILE: a corrupted MyISAM index — every table here is MyISAM
];

/// Message fragments that mark resource exhaustion, as a backstop for the
/// numbers above: the storage engine can wrap the same condition under codes
/// not worth enumerating, and misreading a full disk as a bad query would
/// score every remaining question as a model failure and finish the run with
/// plausible-looking but meaningless accuracy.
const MYSQL_EXHAUSTION_HINTS: [&str; 5] = [
    "No space left",
    "Disk full",
    "Out of memory",
    "from storage engine",
    "server shutdown",
];

/// MySQL overloads the catch-all SQLSTATE HY000 for both query-authoring
/// mistakes and genuine infrastructure faults. Most of what lands there is the
/// query's fault — every recursive-CTE restriction, window-function misuse and
/// JSON error — so HY000 defaults to a model fault and the server-side
/// conditions are named explicitly instead. That way a new kind of bad query
/// costs the model a retry rather than aborting the run, while a full disk
/// still stops it loudly rather than being blamed on the model.
fn classify_mysql_error(db: &dyn sqlx::error::DatabaseError) -> QueryError {
    let message = db.message();
    if let Some(mysql) = db.try_downcast_ref::<sqlx::mysql::MySqlDatabaseError>() {
        let number = mysql.number();
        match number {
            // ER_QUERY_TIMEOUT: max_execution_time elapsed.
            3024 => return QueryError::Timeout,
            // ER_QUERY_INTERRUPTED: a KILL QUERY, which is how an external
            // watchdog rather than max_execution_time would cap a statement.
            1317 => return QueryError::Timeout,
            _ => {}
        }
        if MYSQL_INFRASTRUCTURE.contains(&number)
            || MYSQL_EXHAUSTION_HINTS
                .iter()
                .any(|hint| message.contains(hint))
        {
            return QueryError::Infrastructure(message.to_string());
        }
        if db.code().as_deref() == Some("HY000") {
            return QueryError::Syntax(message.to_string());
        }
    }
    let code = db.code();
    classify_sqlstate(code.as_deref(), message)
}

/// SQLSTATE class decides fault ownership, and the standard classes mean the
/// same thing on both engines. The model owns everything its query text can
/// cause: feature-not-supported (0A), cardinality (21), data exceptions (22),
/// constraint violations (23), invalid transaction state incl. read-only
/// violations (25), syntax/access (42), program limits (54), and PL/pgSQL
/// raises (P0). Everything else — connection, resource, internal — is
/// infrastructure. MySQL leans on 42 for both syntax (42000) and
/// unknown table/column (42S02/42S22), so the same prefix test covers it.
fn classify_sqlstate(code: Option<&str>, message: &str) -> QueryError {
    let Some(code) = code else {
        return QueryError::Infrastructure(message.to_string());
    };
    match code.get(..2) {
        Some("0A") | Some("21") | Some("22") | Some("23") | Some("25") | Some("42")
        | Some("54") | Some("P0") => QueryError::Syntax(message.to_string()),
        _ => QueryError::Infrastructure(message.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sqlstate_classes_decide_fault_ownership() {
        // Syntax error and undefined table: the model's fault.
        assert!(matches!(
            classify_sqlstate(Some("42601"), "syntax error at or near"),
            QueryError::Syntax(_)
        ));
        assert!(matches!(
            classify_sqlstate(Some("42P01"), "relation does not exist"),
            QueryError::Syntax(_)
        ));
        // MySQL's spellings of the same two faults land in the same class.
        assert!(matches!(
            classify_sqlstate(Some("42000"), "You have an error in your SQL syntax"),
            QueryError::Syntax(_)
        ));
        assert!(matches!(
            classify_sqlstate(Some("42S02"), "Table 'reactome.nope' doesn't exist"),
            QueryError::Syntax(_)
        ));
        assert!(matches!(
            classify_sqlstate(Some("42S22"), "Unknown column 'nope' in 'field list'"),
            QueryError::Syntax(_)
        ));
        // A write attempt bounces off the read-only connection — also the
        // model's fault, with a message telling it what happened.
        assert!(matches!(
            classify_sqlstate(Some("25006"), "cannot execute INSERT in a read-only transaction"),
            QueryError::Syntax(m) if m.contains("read-only")
        ));
        // Division by zero is a data exception: model fault.
        assert!(matches!(
            classify_sqlstate(Some("22012"), "division by zero"),
            QueryError::Syntax(_)
        ));
        // Unsupported features and PL/pgSQL raises are things the query
        // caused — not infrastructure.
        assert!(matches!(
            classify_sqlstate(Some("0A000"), "feature not supported"),
            QueryError::Syntax(_)
        ));
        assert!(matches!(
            classify_sqlstate(Some("P0001"), "raise_exception"),
            QueryError::Syntax(_)
        ));
        // Connection/resource classes are infrastructure.
        assert!(matches!(
            classify_sqlstate(Some("08006"), "connection failure"),
            QueryError::Infrastructure(_)
        ));
        assert!(matches!(
            classify_sqlstate(Some("53300"), "too many connections"),
            QueryError::Infrastructure(_)
        ));
        assert!(matches!(
            classify_sqlstate(None, "unknown"),
            QueryError::Infrastructure(_)
        ));
        // MySQL's generic class stays infrastructure by SQLSTATE alone; the
        // timeout numbers are rescued by classify_mysql_error's downcast, which
        // needs a real driver error and so is covered by the live test.
        assert!(matches!(
            classify_sqlstate(Some("HY000"), "Query execution was interrupted"),
            QueryError::Infrastructure(_)
        ));
    }

    #[test]
    fn url_scheme_selects_the_engine() {
        assert!(matches!(
            Sql::new("postgres://localhost/bench", None, None)
                .unwrap()
                .backend,
            Backend::Postgres { .. }
        ));
        assert!(matches!(
            Sql::new("postgresql://localhost/bench", None, None)
                .unwrap()
                .backend,
            Backend::Postgres { .. }
        ));
        assert!(matches!(
            Sql::new("mysql://localhost/reactome", None, None)
                .unwrap()
                .backend,
            Backend::MySql { .. }
        ));
        assert!(matches!(
            Sql::new("mariadb://localhost/reactome", None, None)
                .unwrap()
                .backend,
            Backend::MySql { .. }
        ));
    }

    #[test]
    fn invalid_url_fails_eagerly() {
        assert!(Sql::new("not a url", None, None).is_err());
        // A scheme we don't speak fails at startup rather than at first query.
        // Matched rather than unwrap_err'd: `Sql` deliberately has no Debug
        // impl, since its connect options hold credentials.
        let Err(error) = Sql::new("sqlite://bench.db", None, None) else {
            panic!("expected an unsupported-scheme error");
        };
        assert!(error.contains("postgres://"));
        assert!(Sql::new("postgres://localhost/bench", None, None).is_ok());
    }

    #[tokio::test]
    // No dataset stack runs Postgres any more, so this needs a server of your
    // own with a SELECT-only `bench_ro` role (no CREATE on the schema).
    #[ignore = "requires a running Postgres server (SQL_URL) with a SELECT-only bench_ro role"]
    async fn queries_a_live_server() {
        let url = std::env::var("SQL_URL")
            .unwrap_or_else(|_| "postgres://bench_ro:bench_ro@localhost/bench".to_string());
        let db = Sql::new(&url, None, None).unwrap();
        let value = db.send_query("SELECT 1 + 1").await.unwrap();
        assert_eq!(value, Value::Int(2));
        // The read-only session default rejects writes as a model fault.
        let error = db
            .send_query("CREATE TABLE nope (id INT)")
            .await
            .unwrap_err();
        assert!(matches!(error, QueryError::Syntax(m) if m.contains("read-only")));
        // Even if a generated SET disables the session default, the
        // SELECT-only role still blocks writes (grants are the hard layer).
        db.send_query("SET default_transaction_read_only = off")
            .await
            .unwrap();
        let error = db
            .send_query("CREATE TABLE nope (id INT)")
            .await
            .unwrap_err();
        assert!(matches!(error, QueryError::Syntax(_)));
    }

    #[tokio::test]
    #[ignore = "requires a running MySQL server (MYSQL_URL); see databases/reactome"]
    async fn queries_a_live_mysql_server() {
        let url = std::env::var("MYSQL_URL")
            .unwrap_or_else(|_| "mysql://bench_ro:bench_ro@localhost/reactome".to_string());
        let db = Sql::new(&url, None, None).unwrap();
        assert_eq!(db.send_query("SELECT 1 + 1").await.unwrap(), Value::Int(2));
        // The unsigned widths a real schema keys on, and the u64 overflow path.
        assert_eq!(
            db.send_query("SELECT CAST(42 AS UNSIGNED)").await.unwrap(),
            Value::Int(42)
        );
        assert_eq!(
            db.send_query("SELECT CAST(18446744073709551615 AS UNSIGNED)")
                .await
                .unwrap(),
            Value::String("18446744073709551615".to_string())
        );
        assert_eq!(db.send_query("SELECT NULL").await.unwrap(), Value::Null);
        // SUM over an integer column comes back DECIMAL, not BIGINT.
        assert_eq!(
            db.send_query("SELECT SUM(x) FROM (SELECT 1 AS x UNION ALL SELECT 2) t")
                .await
                .unwrap(),
            Value::Float(3.0)
        );
        // The SELECT-only grant is the hard layer; a write is the model's fault.
        let error = db
            .send_query("CREATE TABLE nope (id INT)")
            .await
            .unwrap_err();
        assert!(matches!(error, QueryError::Syntax(_)));

        // Malformed recursive CTEs report SQLSTATE HY000, which reads as
        // infrastructure unless the native error number is inspected. They are
        // the model's fault and must be retryable, not run-aborting.
        for (label, query) in [
            (
                "recursive table referenced twice",
                "WITH RECURSIVE c AS (SELECT 1 n UNION ALL SELECT c1.n+1 FROM c c1, c c2 WHERE c1.n<3) SELECT * FROM c",
            ),
            (
                "recursive reference inside a subquery",
                "WITH RECURSIVE c AS (SELECT 1 n UNION ALL SELECT n+1 FROM (SELECT n FROM c) x WHERE n<3) SELECT * FROM c",
            ),
            (
                "aggregation in the recursive block",
                "WITH RECURSIVE c AS (SELECT 1 n UNION ALL SELECT COUNT(*) FROM c) SELECT * FROM c",
            ),
            (
                "recursive block placed first",
                "WITH RECURSIVE c AS (SELECT n+1 FROM c UNION ALL SELECT 1 n) SELECT * FROM c",
            ),
            (
                "unbounded recursion hits the depth limit",
                "WITH RECURSIVE c AS (SELECT 1 n UNION ALL SELECT n+1 FROM c) SELECT COUNT(*) FROM c",
            ),
        ] {
            let error = db.send_query(query).await.unwrap_err();
            assert!(
                matches!(error, QueryError::Syntax(_)),
                "{label}: expected a model fault, got {error:?}"
            );
        }
    }
}
