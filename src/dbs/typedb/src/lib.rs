//! TypeDB package: queries go through the official `typedb-driver`, run in
//! read transactions (generated queries cannot mutate the dataset) with a
//! package-level query timeout and result-size cap.

use std::time::Duration;

use async_trait::async_trait;
use bench_core::temporal::{canonical_date, canonical_datetime};
use bench_core::value::shape_rows;
use bench_core::{Database, QueryError, Value};
use futures::StreamExt;
use serde::Deserialize;
use tokio::sync::OnceCell;
use typedb_driver::answer::QueryAnswer;
use typedb_driver::concept::Concept;
use typedb_driver::concept::value::Value as TypeDbValue;
use typedb_driver::{
    Addresses, Credentials, DriverOptions, DriverTlsConfig, TransactionType, TypeDBDriver,
};

/// Pathological queries surface as model-fault timeouts rather than hanging
/// the run (the runner's 240s ceiling stays a last resort). Set to catch
/// runaway queries, not slow ones: a legitimate query that needs a minute is
/// a fact about the language, not a fault, so the cap is well clear of the
/// slowest reference query. The three DB packages hold the same cap, or a
/// query that is merely slow would fail in one language and pass in another.
///
/// Raised from 120s: the Reactome reference query for the three-way input
/// intersection beneath R-HSA-168249 measured 94.0s, 94.6s, 99.1s, 102.1s and
/// one timeout across five runs on the same machine, so a 120s cap scored a
/// correct query as a failure perhaps half the time. That question is
/// legitimately ~25x slower here than in Cypher and SQL, which is a
/// measurement the benchmark exists to make, not a fault to cut off.
const QUERY_TIMEOUT: Duration = Duration::from_secs(180);
const MAX_ROWS: usize = 10_000;

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TypeDbAuth {
    pub username: String,
    pub password: String,
    /// TLS with the system's native trust roots when true. Defaults to OFF
    /// for local benchmark servers — credentials travel in plaintext, so
    /// only point this at trusted/local deployments without setting it.
    pub tls: bool,
}

impl Default for TypeDbAuth {
    fn default() -> Self {
        Self {
            username: "admin".to_string(),
            password: "password".to_string(),
            tls: false,
        }
    }
}

pub struct TypeDb {
    addresses: Addresses,
    /// Kept for error messages.
    address: String,
    database: String,
    auth: TypeDbAuth,
    driver: OnceCell<TypeDBDriver>,
}

impl TypeDb {
    /// Validates the address eagerly so a config typo fails at startup,
    /// not at the first query.
    pub fn new(
        address: impl Into<String>,
        database: impl Into<String>,
        auth: TypeDbAuth,
    ) -> Result<Self, String> {
        let address = address.into();
        let addresses = Addresses::try_from_address_str(&address)
            .map_err(|e| format!("invalid TypeDB address `{address}` (expected host:port): {e}"))?;
        Ok(Self {
            addresses,
            address,
            database: database.into(),
            auth,
            driver: OnceCell::new(),
        })
    }

    /// Connect lazily so constructing the package (before any question runs)
    /// can't fail.
    async fn driver(&self) -> Result<&TypeDBDriver, QueryError> {
        self.driver
            .get_or_try_init(|| async {
                let tls = if self.auth.tls {
                    DriverTlsConfig::enabled_with_native_root_ca()
                } else {
                    DriverTlsConfig::disabled()
                };
                let driver = TypeDBDriver::new(
                    self.addresses.clone(),
                    Credentials::new(&self.auth.username, &self.auth.password),
                    DriverOptions::new(tls),
                )
                .await?;
                // Fail fast — and as infrastructure — on a missing database:
                // a config typo must not be scored as model failures.
                if !driver.databases().contains(&self.database).await? {
                    return Err(typedb_driver::Error::Other(format!(
                        "database `{}` does not exist on {}",
                        self.database, self.address
                    )));
                }
                Ok(driver)
            })
            .await
            .map_err(|e| QueryError::Infrastructure(e.to_string()))
    }
}

#[async_trait]
impl Database for TypeDb {
    fn query_language(&self) -> &'static str {
        "typeql"
    }

    async fn send_query(&self, query: &str) -> Result<Value, QueryError> {
        let driver = self.driver().await?;
        let transaction = driver
            .transaction(&self.database, TransactionType::Read)
            .await
            .map_err(|e| QueryError::Infrastructure(e.to_string()))?;
        // The timeout covers execution AND streaming: pathological queries
        // usually answer fast and then stream forever, so guarding only the
        // query() call would miss them.
        tokio::time::timeout(QUERY_TIMEOUT, async {
            let answer = transaction.query(query).await.map_err(map_driver_error)?;
            coerce_answer(answer).await
        })
        .await
        .map_err(|_| QueryError::Timeout)?
    }

    /// Driving the lazy connection validates the address, credentials, and
    /// database existence.
    async fn health_check(&self) -> Result<(), QueryError> {
        self.driver().await.map(|_| ())
    }
}

/// The model owns errors raised about the query itself (parse, analysis,
/// server-side rejection). Everything else — transport, driver internals,
/// client-side concept API misuse — is infrastructure, and is never blamed
/// on the model.
fn map_driver_error(error: typedb_driver::Error) -> QueryError {
    use typedb_driver::Error;
    match error {
        error @ (Error::Analyze(_) | Error::Server(_)) => QueryError::Syntax(error.to_string()),
        error => QueryError::Infrastructure(error.to_string()),
    }
}

async fn coerce_answer(answer: QueryAnswer) -> Result<Value, QueryError> {
    match answer {
        QueryAnswer::Ok(_) => Ok(Value::Null),
        QueryAnswer::ConceptRowStream(header, stream) => {
            coerce_row_stream(&header.column_names, stream).await
        }
        QueryAnswer::ConceptDocumentStream(_, stream) => coerce_document_stream(stream).await,
    }
}

async fn coerce_row_stream(
    columns: &[String],
    mut stream: impl futures::Stream<
        Item = typedb_driver::Result<typedb_driver::answer::concept_row::ConceptRow>,
    > + Unpin,
) -> Result<Value, QueryError> {
    let mut table = Vec::new();
    while let Some(row) = stream.next().await {
        let row = row.map_err(map_driver_error)?;
        if table.len() >= MAX_ROWS {
            return Err(QueryError::WrongShape(format!(
                "result exceeded {MAX_ROWS} rows"
            )));
        }
        let mut cells = Vec::with_capacity(columns.len());
        for name in columns {
            // A column the header promised but the row lacks is a
            // driver/harness defect, never the model's.
            let concept = row.get(name).map_err(|e| {
                QueryError::Infrastructure(format!("row missing column `{name}`: {e}"))
            })?;
            cells.push(coerce_concept(name, concept)?);
        }
        table.push(cells);
    }
    Ok(shape_rows(columns, table))
}

async fn coerce_document_stream(
    mut stream: impl futures::Stream<
        Item = typedb_driver::Result<typedb_driver::answer::concept_document::ConceptDocument>,
    > + Unpin,
) -> Result<Value, QueryError> {
    let mut docs = Vec::new();
    while let Some(document) = stream.next().await {
        let document = document.map_err(map_driver_error)?;
        if docs.len() >= MAX_ROWS {
            return Err(QueryError::WrongShape(format!(
                "result exceeded {MAX_ROWS} documents"
            )));
        }
        // The driver's JSON type round-trips through its string form into
        // our canonical Value.
        let json: serde_json::Value = serde_json::from_str(&document.into_json().to_string())
            .map_err(|e| QueryError::WrongShape(format!("undecodable document: {e}")))?;
        docs.push(Value::from(json));
    }
    Ok(if docs.len() == 1 {
        docs.pop().unwrap()
    } else {
        Value::List(docs)
    })
}

/// Only values and attributes are comparable benchmark results; a variable
/// bound to an entity/relation/type is a wrong shape, and the message tells
/// the model how to fix it.
fn coerce_concept(name: &str, concept: Option<&Concept>) -> Result<Value, QueryError> {
    let Some(concept) = concept else {
        return Ok(Value::Null);
    };
    match concept.try_get_value() {
        Some(value) => Ok(coerce_value(value)),
        None => Err(QueryError::WrongShape(format!(
            "variable ${name} is bound to {}, which is not a value; \
             return attributes or values instead",
            concept.get_label()
        ))),
    }
}

fn coerce_value(value: &TypeDbValue) -> Value {
    match value {
        TypeDbValue::Boolean(b) => Value::Bool(*b),
        TypeDbValue::Integer(i) => Value::Int(*i),
        TypeDbValue::Double(d) => Value::Float(*d),
        // Integer part plus fractional part in units of 10^-19; lossy into
        // f64 by design (the canonical Value has no decimal type).
        TypeDbValue::Decimal(d) => Value::Float(d.integer as f64 + d.fractional as f64 / 1e19),
        TypeDbValue::String(s) => Value::String(s.clone()),
        // The framework-wide canonical forms from bench_core::temporal,
        // shared with the other DB packages so one `expected` string works
        // across DBs (pinned by tests).
        TypeDbValue::Date(date) => Value::String(canonical_date(*date)),
        TypeDbValue::Datetime(datetime) => Value::String(canonical_datetime(*datetime)),
        // Remaining kinds (tz datetimes, durations, structs) fall back to
        // the driver's Display form until a canonical rule is settled.
        other => Value::String(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
    use typedb_driver::concept::value::Decimal;

    use super::*;

    #[test]
    fn coerces_primitive_values() {
        assert_eq!(coerce_value(&TypeDbValue::Boolean(true)), Value::Bool(true));
        assert_eq!(coerce_value(&TypeDbValue::Integer(42)), Value::Int(42));
        assert_eq!(coerce_value(&TypeDbValue::Double(2.5)), Value::Float(2.5));
        assert_eq!(
            coerce_value(&TypeDbValue::String("ka".to_string())),
            Value::String("ka".to_string())
        );
    }

    #[test]
    fn coerces_decimals_including_negative() {
        let half = 5 * 10u64.pow(18);
        assert_eq!(
            coerce_value(&TypeDbValue::Decimal(Decimal {
                integer: 1,
                fractional: half
            })),
            Value::Float(1.5)
        );
        // -1.5 is integer -2 plus fractional +0.5 (fractional is always a
        // positive offset).
        assert_eq!(
            coerce_value(&TypeDbValue::Decimal(Decimal {
                integer: -2,
                fractional: half
            })),
            Value::Float(-1.5)
        );
    }

    /// Pins the canonical string forms question authors must use in
    /// `expected` for temporal answers.
    #[test]
    fn temporal_values_coerce_to_canonical_strings() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        assert_eq!(
            coerce_value(&TypeDbValue::Date(date)),
            Value::String("2024-01-15".to_string())
        );
        // Datetimes carry full nanosecond precision in the driver's form.
        let datetime = NaiveDateTime::new(date, NaiveTime::from_hms_opt(10, 30, 0).unwrap());
        assert_eq!(
            coerce_value(&TypeDbValue::Datetime(datetime)),
            Value::String("2024-01-15T10:30:00.000000000".to_string())
        );
    }

    #[test]
    fn unbound_variables_coerce_to_null() {
        assert_eq!(coerce_concept("x", None).unwrap(), Value::Null);
    }

    #[tokio::test]
    #[ignore = "requires a seeded TypeDB server (TYPEDB_ADDRESS, TYPEDB_DATABASE); see databases/reactome"]
    async fn queries_a_live_server() {
        let address =
            std::env::var("TYPEDB_ADDRESS").unwrap_or_else(|_| "127.0.0.1:1729".to_string());
        let database =
            std::env::var("TYPEDB_DATABASE").unwrap_or_else(|_| "reactome".to_string());
        let db = TypeDb::new(address, database, TypeDbAuth::default()).unwrap();

        // The loaded Reactome data: a reduce count unwraps to a scalar. Homo
        // sapiens is one species, whatever the release.
        let count = db
            .send_query("match $x isa species, has display-name \"Homo sapiens\"; reduce $count = count;")
            .await
            .unwrap();
        assert_eq!(count, Value::Int(1));

        // Read transactions reject writes as a model fault.
        let error = db
            .send_query("insert $x isa species, has db-id -1;")
            .await
            .unwrap_err();
        assert!(matches!(error, QueryError::Syntax(_)));
    }
}
