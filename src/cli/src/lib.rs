//! Shared helpers for the bench CLI binaries (`db-llm-bench`, `verify`):
//! constructing a live [`Database`] from its config entry.

use anyhow::Context;
use bench_config::DbConfig;
use bench_core::Database;

/// Parse a DB's free-form auth block into the package's typed auth struct.
fn parse_auth<T: serde::de::DeserializeOwned>(
    cfg: &DbConfig,
    db_id: &str,
) -> anyhow::Result<Option<T>> {
    cfg.auth
        .as_ref()
        .map(|value| {
            serde_json::from_value(value.clone())
                .with_context(|| format!("parsing {db_id} auth (expects username/password)"))
        })
        .transpose()
}

/// Each valid DB ID gets its own package; adding a DB means adding a crate
/// and an arm here.
pub fn build_db(id: &str, cfg: &DbConfig) -> anyhow::Result<Box<dyn Database>> {
    Ok(match id {
        "dummy" => Box::new(db_dummy::DummyDb::new()),
        "typedb" => {
            let database = cfg
                .database
                .clone()
                .ok_or_else(|| anyhow::anyhow!("typedb requires `database` in its config"))?;
            let auth = parse_auth::<db_typedb::TypeDbAuth>(cfg, "typedb")?.unwrap_or_default();
            Box::new(
                db_typedb::TypeDb::new(cfg.url.clone(), database, auth)
                    .map_err(anyhow::Error::msg)?,
            )
        }
        "neo4j" => {
            let auth = parse_auth::<db_neo4j::Neo4jAuth>(cfg, "neo4j")?;
            Box::new(
                db_neo4j::Neo4j::new(&cfg.url, cfg.database.as_deref(), auth.as_ref())
                    .map_err(anyhow::Error::msg)?,
            )
        }
        "sql" => {
            let auth = parse_auth::<db_sql::SqlAuth>(cfg, "sql")?;
            Box::new(
                db_sql::Sql::new(&cfg.url, cfg.database.as_deref(), auth.as_ref())
                    .map_err(anyhow::Error::msg)?,
            )
        }
        other => anyhow::bail!("unknown DB id: {other}"),
    })
}
