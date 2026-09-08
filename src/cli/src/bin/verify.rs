//! Reference-query checker. Runs each answerable question's pre-written
//! ground-truth query against the live DBs and reports every result that does
//! not match the question's `expected` value.
//!
//! It reuses the benchmark's own DB packages and [`Value::matches_question`],
//! so it exercises the identical execute → coerce → compare path the benchmark
//! scores with: a query that passes here is one the benchmark would count
//! accurate, and a mismatch here is a genuine bug in the query or `expected`.
//!
//! Usage: `verify <config.yml> [questions.json] [--timings <path>]`
//! Questions default to the config's `questionsPath`. The DBs named in the
//! config must be up (e.g. via `databases/reactome/docker-compose.yml`).
//!
//! Every reference query is timed as it runs, and `--timings` writes those
//! times out as `{db: {question: ms}}` — the baseline `analysis/query_time.py`
//! judges a generated query against, since a model's 2.3s only means something
//! next to what the reference costs for the same question on the same machine.
//! Timings are one measurement each, on whatever cache state the run happened
//! to have, so treat them as an order-of-magnitude reference rather than a
//! benchmark of the stores against each other.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use bench_cli::build_db;
use bench_config::{Config, load_questions};

const USAGE: &str = "usage: verify <config.yml> [questions.json] [--timings <path>]";

#[tokio::main]
async fn main() -> anyhow::Result<ExitCode> {
    let mut positional = Vec::new();
    let mut timings_path: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--timings" => {
                timings_path =
                    Some(PathBuf::from(args.next().ok_or_else(|| {
                        anyhow::anyhow!("--timings needs a path\n{USAGE}")
                    })?));
            }
            other if other.starts_with("--") => anyhow::bail!("unknown flag {other}\n{USAGE}"),
            other => positional.push(PathBuf::from(other)),
        }
    }
    let mut positional = positional.into_iter();
    let config_path = positional
        .next()
        .ok_or_else(|| anyhow::anyhow!("{USAGE}"))?;
    let config = Config::load(&config_path)?;
    let questions_path = positional
        .next()
        .unwrap_or_else(|| config.questions_path.clone());
    let questions = load_questions(&questions_path)?;
    println!(
        "Verifying {} against {}",
        questions_path.display(),
        config_path.display()
    );

    // Reference-query execution time, keyed by DB then question text — the
    // baseline a generated query's time is judged against, since "2.3s" only
    // means something next to what the reference costs for the same question.
    let mut timings: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
    let mut total_checked = 0usize;
    let mut total_failed = 0usize;
    for (db_id, db_cfg) in config.db_entries() {
        let db = build_db(db_id, db_cfg)?;
        let lang = db.query_language();
        db.health_check()
            .await
            .map_err(|e| anyhow::anyhow!("DB {db_id} failed its health check: {e}"))?;

        let mut checked = 0usize;
        let mut failed = 0usize;
        for question in &questions.questions {
            if question.unanswerable {
                continue;
            }
            let Some(query) = question.queries.get(lang) else {
                println!(
                    "  [MISSING] {db_id}: no {lang} query — {}",
                    question.question
                );
                failed += 1;
                continue;
            };
            let expected = question
                .expected_for(db_id)
                .expect("answerable question has an expected value (enforced at load)");
            checked += 1;
            let started = Instant::now();
            let outcome = db.send_query(query).await;
            let elapsed_ms = started.elapsed().as_millis() as u64;
            // Recorded whatever the outcome: a reference query that errors or
            // times out is a fact about the question worth keeping, and the
            // pass/fail line above already says which it was.
            timings
                .entry(db_id.to_string())
                .or_default()
                .insert(question.question.clone(), elapsed_ms);
            match outcome {
                Ok(value) if value.matches_question(expected, question.ordered) => {}
                Ok(value) => {
                    failed += 1;
                    println!(
                        "  [FAIL] {db_id}: {}\n         expected: {expected:?}\n         got:      {value:?}",
                        question.question
                    );
                }
                Err(e) => {
                    failed += 1;
                    println!("  [ERROR] {db_id}: {}\n         {e}", question.question);
                }
            }
        }
        let mut times: Vec<u64> = timings
            .get(db_id)
            .map(|t| t.values().copied().collect())
            .unwrap_or_default();
        times.sort_unstable();
        if let Some(&slowest) = times.last() {
            println!(
                "{db_id}: {}/{checked} matched  (query time median {}, slowest {})",
                checked - failed,
                human_ms(times[times.len() / 2]),
                human_ms(slowest),
            );
        } else {
            println!("{db_id}: {}/{checked} matched", checked - failed);
        }
        total_checked += checked;
        total_failed += failed;
    }

    if let Some(path) = timings_path {
        std::fs::write(&path, serde_json::to_string_pretty(&timings)? + "\n")?;
        println!("\nWrote reference query timings to {}", path.display());
    }

    if total_failed == 0 {
        println!("\nAll {total_checked} checks passed.");
        Ok(ExitCode::SUCCESS)
    } else {
        println!("\n{total_failed} of {total_checked} checks FAILED.");
        Ok(ExitCode::FAILURE)
    }
}

/// Milliseconds under a second, seconds above — reference query times span
/// three orders of magnitude here.
fn human_ms(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{:.1}s", ms as f64 / 1000.0)
    }
}
