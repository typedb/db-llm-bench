//! Benchmark runner: one runner per DB + model + example count + skills
//! setup. Runs each question at the highest configured retry count; lower
//! retry levels are derived from the attempt trace during output marshalling.

use std::time::{Duration, Instant};

use bench_core::{
    Attempt, Database, Message, ModelProvider, ModelResponse, ProviderError, QueryError, Question,
    RecordResult, ResultRecord, TokenUsage, Value, attempt_totals,
};
use thiserror::Error;

/// Repetitions per question/setup cell, to account for LLM non-determinism.
pub const REPETITIONS: u32 = 3;

/// Harness-level retries for transient provider errors (rate limits, network
/// blips) and provider timeouts. These never count against the model's retry
/// budget. Doubling from a 10s base gives ~310s of total patience — enough to
/// ride out transient errors.
pub const TRANSIENT_RETRIES: u32 = 5;
const TRANSIENT_BACKOFF: Duration = Duration::from_secs(10);

/// Harness-level retries for DB infrastructure errors (dropped connections,
/// restarts) — the DB-side mirror of the transient provider policy.
pub const INFRA_RETRIES: u32 = 5;
const INFRA_BACKOFF: Duration = Duration::from_millis(500);

/// Defensive ceilings only: packages own the real timeouts. These convert a
/// hung backend into a diagnosable failure instead of a frozen run: a
/// provider timeout costs the repetition, a DB one is an infrastructure
/// failure.
const PROVIDER_TIMEOUT: Duration = Duration::from_secs(600);
const DB_TIMEOUT: Duration = Duration::from_secs(240);

/// Literal token the prompt instructs the model to emit, alone on its own
/// line, when it believes the question cannot be answered against the
/// schema. The own-line rule keeps prose that merely mentions the token
/// (e.g. "I shouldn't say UNANSWERABLE here") from reading as a decline.
pub const UNANSWERABLE_TOKEN: &str = "UNANSWERABLE";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Extraction {
    Query(String),
    /// Terminal — never retried. Correct for deliberately-unanswerable
    /// questions, a failure otherwise.
    Unanswerable,
    /// Neither a code block nor the UNANSWERABLE marker. Retryable, since
    /// declining has an explicit channel — retrying doesn't pressure the
    /// model into hallucinating a query.
    Malformed,
}

/// Extract the query from a model response: the last fenced code block.
pub fn extract_query(response: &str) -> Extraction {
    let mut blocks: Vec<String> = Vec::new();
    let mut current: Option<Vec<&str>> = None;
    for line in response.lines() {
        if line.trim_start().starts_with("```") {
            match current.take() {
                Some(lines) => blocks.push(lines.join("\n")),
                None => current = Some(Vec::new()),
            }
        } else if let Some(lines) = current.as_mut() {
            lines.push(line);
        }
    }
    // An unterminated trailing fence still counts: a truncated response
    // yields its partial query (and a real execution error to iterate on)
    // rather than "no query found".
    if let Some(lines) = current {
        blocks.push(lines.join("\n"));
    }
    if let Some(block) = blocks.pop() {
        let query = block.trim();
        if !query.is_empty() {
            if query == UNANSWERABLE_TOKEN {
                return Extraction::Unanswerable;
            }
            return Extraction::Query(query.to_string());
        }
    }
    if response
        .lines()
        .any(|line| line.trim() == UNANSWERABLE_TOKEN)
    {
        return Extraction::Unanswerable;
    }
    // Fallback: a reply that is nothing but a query, unfenced. Some models
    // ignore the fencing instruction consistently, which would otherwise score
    // them at zero for a formatting habit rather than for their queries. The
    // rule is deliberately narrow — the whole reply must begin with a keyword
    // that opens a query in one of the benchmarked languages — so a fenced
    // reply, prose, or a refusal is unaffected.
    let trimmed = response.trim();
    if starts_a_query(trimmed) {
        return Extraction::Query(trimmed.to_string());
    }
    Extraction::Malformed
}

/// First words that open a query in SQL, Cypher or TypeQL. Write keywords are
/// included on purpose: an unfenced `INSERT` should reach the DB and be
/// refused by the read-only role, scoring as the model's fault, rather than
/// being filed as a formatting error.
const QUERY_OPENERS: [&str; 18] = [
    // SQL
    "SELECT", "WITH", "VALUES", "TABLE", // Cypher
    "MATCH", "OPTIONAL", "RETURN", "UNWIND", "CALL", "SHOW", // TypeQL
    "DEFINE", "UNDEFINE", "INSERT", "DELETE", "UPDATE", "PUT", "FETCH", "REDUCE",
];

fn starts_a_query(response: &str) -> bool {
    let Some(first) = response.split_whitespace().next() else {
        return false;
    };
    // Trim punctuation a model might attach, e.g. a stray "SELECT," is still
    // recognisably a query opener.
    let first = first.trim_matches(|c: char| !c.is_ascii_alphabetic());
    QUERY_OPENERS
        .iter()
        .any(|opener| first.eq_ignore_ascii_case(opener))
}

/// Fill the prompt template's slots. `skills` is empty when this setup runs
/// with skills off. The examples slot brings its own "Examples:" heading so
/// that a zero-example prompt doesn't contain a heading with nothing under
/// it.
pub fn assemble_prompt(
    template: &str,
    question: &str,
    schema: &str,
    examples: &[String],
    skills: &[String],
) -> String {
    let examples_section = if examples.is_empty() {
        String::new()
    } else {
        format!("Examples:\n{}", examples.join("\n\n"))
    };
    template
        .replace("{{question}}", question)
        .replace("{{schema}}", schema)
        .replace("{{examples}}", &examples_section)
        .replace("{{skills}}", &skills.join("\n\n"))
}

pub struct BenchmarkRunner<'a> {
    pub db: &'a dyn Database,
    /// The config's id for `db`, used to pick a question's per-store expected
    /// value where the stores disagree.
    pub db_id: &'a str,
    pub model: &'a dyn ModelProvider,
    pub prompt_template: String,
    pub schema: String,
    /// Examples inserted into the prompt, already truncated to this setup's
    /// example count.
    pub examples: Vec<String>,
    /// Skills content when this setup runs with skills enabled.
    pub skills: Option<Vec<String>>,
    /// The highest configured retry count — the only level actually run.
    pub max_retries: u32,
}

#[derive(Debug)]
pub struct QuestionRun {
    pub question_index: usize,
    pub records: Vec<ResultRecord>,
}

/// Errors that abort the run (provider timeouts are the exception: they are
/// intercepted in `run_once` and cost only the repetition in flight).
/// Not the model's fault as far as scoring goes —
/// these never count as a wrong answer — but the attempt that hit one is still
/// recorded, because a DB "infrastructure" failure is frequently a generated
/// query that took the server down, and that query is the evidence.
///
/// Both transports get harness-level backoff first; reaching this type means
/// the backoff budget was spent (or the error was fatal).
#[derive(Debug, Error)]
pub enum RunError {
    #[error(transparent)]
    Provider(#[from] ProviderError),
    #[error("infrastructure error: {0}")]
    Infrastructure(String),
}

/// An aborted run, carrying everything completed before the failure so the
/// tokens already spent aren't discarded.
#[derive(Debug, Error)]
#[error("question {question_index} repetition {repetition}: {error}")]
pub struct RunFailure {
    pub error: RunError,
    pub question_index: usize,
    pub repetition: u32,
    /// Fully completed question runs, plus a partial run for the failing
    /// question: its finished repetitions, and the repetition that died if it
    /// got far enough to produce an attempt.
    pub completed: Vec<QuestionRun>,
}

/// A model-fault outcome: the error message that goes in the attempt trace,
/// tagged by whether the model gets another try.
enum Fault {
    Retryable(String),
    Terminal(String),
}

impl Fault {
    fn message(&self) -> &str {
        match self {
            Fault::Retryable(message) | Fault::Terminal(message) => message,
        }
    }
}

/// A terminal, successful attempt outcome.
enum Success {
    /// A query executed and produced a value (accuracy still to be judged).
    Value(Value),
    /// The model declined a deliberately-unanswerable question — the
    /// correct response, by definition.
    Unanswerable,
}

/// A run-ending failure, plus the attempt that was in flight when it hit.
///
/// The attempt is the point: an "infrastructure" error is often a generated
/// query that took the DB down with it, and without this the query that did
/// the damage is discarded along with the run.
struct AbortedAttempt {
    error: RunError,
    /// None when the run died before a query existed — a provider failure has
    /// nothing worth recording.
    partial: Option<Attempt>,
}

/// A run-ending failure, plus the record for the repetition it interrupted.
struct AbortedRun {
    error: RunError,
    /// None when the repetition had produced nothing worth recording — a
    /// provider outage before any query exists leaves an empty trace, and an
    /// attempt-less record would be noise in the results file.
    record: Option<ResultRecord>,
}

/// A DB failure that ends the run, with the timing of the call that failed, so
/// the aborted attempt still reports how long the query ran.
struct DbAbort {
    error: RunError,
    db_latency_ms: u64,
}

/// Everything one prompt-extract-execute round trip produced.
struct AttemptOutcome {
    query: Option<String>,
    tokens: TokenUsage,
    latency_ms: u64,
    /// The DB's share of `latency_ms`; zero when no query was executed.
    db_latency_ms: u64,
    response_text: String,
    outcome: Result<Success, Fault>,
}

/// The question text, plus a standardized instruction describing the required
/// return shape — every model and DB gets the identical sentence, derived from
/// `expected`, so return type is scored uniformly with `expected` as the single
/// source of truth (like field naming for objects). Unanswerable questions have
/// no `expected`, so they get the bare question.
///
/// A right answer in the wrong shape is scored as a miss, deliberately. Shaping
/// the output is part of using a query language, and where a language makes that
/// awkward the difficulty is the thing being measured: Cypher rejects `ORDER BY
/// count(x)` unless the count is also projected, so models repair the error by
/// returning the count alongside the answer and hand back a two-column row where
/// one value was asked for. That is the ergonomic gap showing up in the score,
/// not a scoring accident.
fn question_text(question: &Question) -> String {
    match question.expected.as_ref() {
        Some(expected) => format!(
            "{}\n{}",
            question.question,
            return_shape_instruction(expected)
        ),
        None => question.question.clone(),
    }
}

/// The return-shape instruction implied by the expected value's type. Objects
/// (and row objects in a list) additionally get the exact field names, which
/// the model must reproduce for the result to match.
fn return_shape_instruction(expected: &Value) -> String {
    let field_names = |keys: std::collections::btree_map::Keys<'_, String, Value>| {
        keys.cloned().collect::<Vec<_>>().join(", ")
    };
    match expected {
        Value::Bool(_) => "Return a single boolean value, true or false.".to_string(),
        Value::Int(_) => "Return a single integer.".to_string(),
        Value::Float(_) => "Return a single number.".to_string(),
        Value::String(_) => "Return a single text value.".to_string(),
        Value::Null => "Return a single value.".to_string(),
        Value::Object(object) => format!(
            "Return a single row. Name the output fields exactly: {}.",
            field_names(object.keys())
        ),
        Value::List(rows) => match rows.iter().find_map(|row| match row {
            Value::Object(object) => Some(object),
            _ => None,
        }) {
            Some(object) => format!(
                "Return one row per result. Name the output fields exactly: {}.",
                field_names(object.keys())
            ),
            None => {
                let kind = match rows.first() {
                    Some(Value::Int(_)) => "integer ",
                    Some(Value::Float(_)) => "numeric ",
                    Some(Value::Bool(_)) => "boolean ",
                    Some(Value::String(_)) => "text ",
                    _ => "",
                };
                format!("Return a list of {kind}values.")
            }
        },
    }
}

/// Feedback for a retryable fault: honest about whether a query ran and
/// failed, or no query could be extracted at all.
fn retry_feedback(query_ran: bool, message: &str) -> String {
    if query_ran {
        format!(
            "The query failed with the following error:\n\n{message}\n\n\
             Please respond with a corrected query."
        )
    } else {
        "Your response did not contain a fenced code block. Respond with \
         exactly one fenced code block containing only the query, or the \
         literal token UNANSWERABLE alone on its own line."
            .to_string()
    }
}

impl BenchmarkRunner<'_> {
    /// For each question, REPETITIONS times: assemble the prompt, send it to
    /// the model, extract and execute the query, feeding model-fault errors
    /// back until success or the retry budget is spent, then compare against
    /// the expected result.
    pub async fn run(&self, questions: &[Question]) -> Result<Vec<QuestionRun>, RunFailure> {
        let mut runs = Vec::with_capacity(questions.len());
        for (question_index, question) in questions.iter().enumerate() {
            let mut records = Vec::with_capacity(REPETITIONS as usize);
            for repetition in 1..=REPETITIONS {
                eprintln!(
                    "Running question: {} - {} of {} repetition {repetition} of {}",
                    question.question,
                    question_index + 1,
                    questions.len(),
                    REPETITIONS
                );
                match self.run_once(question, repetition).await {
                    Ok(record) => records.push(record),
                    Err(AbortedRun { error, record }) => {
                        records.extend(record);
                        let mut completed = runs;
                        if !records.is_empty() {
                            completed.push(QuestionRun {
                                question_index,
                                records,
                            });
                        }
                        return Err(RunFailure {
                            error,
                            question_index,
                            repetition,
                            completed,
                        });
                    }
                }
            }
            runs.push(QuestionRun {
                question_index,
                records,
            });
        }
        Ok(runs)
    }

    /// Attempts the question up to `max_retries + 1` times, feeding each
    /// model-fault error (with the prior conversation) back to the model.
    /// UNANSWERABLE and wrong-but-valid results are terminal: only errors
    /// that can't possibly be correct earn a retry.
    async fn run_once(
        &self,
        question: &Question,
        repetition: u32,
    ) -> Result<ResultRecord, AbortedRun> {
        let mut conversation = vec![Message::user(self.assemble(question))];
        let mut attempts: Vec<Attempt> = Vec::new();

        loop {
            let AttemptOutcome {
                query,
                tokens,
                latency_ms,
                db_latency_ms,
                response_text,
                outcome,
            } = match self.attempt(&conversation, question.unanswerable).await {
                Ok(outcome) => outcome,
                // A provider that stayed timed out through the whole backoff
                // budget ends the repetition, not the run: the stall is
                // recorded as an error attempt (zero cost — nothing was
                // received) and the benchmark moves on.
                Err(AbortedAttempt {
                    error: RunError::Provider(ProviderError::Timeout(message)),
                    partial,
                }) => {
                    attempts.extend(partial);
                    attempts.push(Attempt {
                        query: None,
                        response: None,
                        tokens: TokenUsage::default(),
                        latency_ms: 0,
                        db_latency_ms: 0,
                        error: Some(message),
                    });
                    return Ok(self.build_record(repetition, attempts, RecordResult::Error, false));
                }
                // The run is over, but the attempt that ended it still gets a
                // record, so the query that did it reaches the results file.
                Err(AbortedAttempt { error, partial }) => {
                    attempts.extend(partial);
                    let record = (!attempts.is_empty()).then(|| {
                        self.build_record(repetition, attempts, RecordResult::Error, false)
                    });
                    return Err(AbortedRun { error, record });
                }
            };
            attempts.push(Attempt {
                query: query.clone(),
                response: query.is_none().then(|| response_text.clone()),
                tokens,
                latency_ms,
                db_latency_ms,
                error: outcome
                    .as_ref()
                    .err()
                    .map(|fault| fault.message().to_string()),
            });

            eprintln!("Outcome received");

            match outcome {
                Ok(Success::Value(value)) => {
                    // An unanswerable question has no expected value, so any
                    // returned value is inaccurate.
                    let accurate = question
                        .expected_for(self.db_id)
                        .is_some_and(|expected| value.matches_question(expected, question.ordered));
                    return Ok(self.build_record(
                        repetition,
                        attempts,
                        RecordResult::Value(value),
                        accurate,
                    ));
                }
                Ok(Success::Unanswerable) => {
                    return Ok(self.build_record(
                        repetition,
                        attempts,
                        RecordResult::Unanswerable,
                        true,
                    ));
                }
                Err(Fault::Terminal(_)) => {
                    return Ok(self.build_record(repetition, attempts, RecordResult::Error, false));
                }
                Err(Fault::Retryable(message)) => {
                    if attempts.len() > self.max_retries as usize {
                        return Ok(self.build_record(
                            repetition,
                            attempts,
                            RecordResult::Error,
                            false,
                        ));
                    }
                    eprintln!("Retrying...");
                    conversation.push(Message::assistant(response_text));
                    conversation.push(Message::user(retry_feedback(query.is_some(), &message)));
                }
            }
        }
    }

    fn assemble(&self, question: &Question) -> String {
        assemble_prompt(
            &self.prompt_template,
            &question_text(question),
            &self.schema,
            &self.examples,
            self.skills.as_deref().unwrap_or_default(),
        )
    }

    /// One prompt-extract-execute round trip; never touches retry
    /// bookkeeping. `unanswerable` is the question's flag: it decides
    /// whether a declined response is the correct answer or a terminal
    /// failure.
    async fn attempt(
        &self,
        conversation: &[Message],
        unanswerable: bool,
    ) -> Result<AttemptOutcome, AbortedAttempt> {
        eprintln!("Sending conversation to model...");
        let (response, provider_latency_ms) =
            self.send_with_backoff(conversation)
                .await
                .map_err(|error| AbortedAttempt {
                    error,
                    partial: None,
                })?;
        eprintln!("Extracting query...");
        let (query, db_latency_ms, outcome) = match extract_query(&response.text) {
            Extraction::Query(query) => match self.query_with_backoff(&query).await {
                Ok((outcome, db_latency_ms)) => {
                    (Some(query), db_latency_ms, outcome.map(Success::Value))
                }
                Err(DbAbort {
                    error,
                    db_latency_ms,
                }) => {
                    let message = error.to_string();
                    return Err(AbortedAttempt {
                        error,
                        partial: Some(Attempt {
                            query: Some(query),
                            response: None,
                            tokens: response.tokens,
                            latency_ms: provider_latency_ms + db_latency_ms,
                            db_latency_ms,
                            error: Some(message),
                        }),
                    });
                }
            },
            Extraction::Unanswerable if unanswerable => (None, 0, Ok(Success::Unanswerable)),
            Extraction::Unanswerable => (
                None,
                0,
                Err(Fault::Terminal("declared UNANSWERABLE".to_string())),
            ),
            // An abnormal stop (refusal, truncation) makes the trace say
            // why there was no query, not just that there wasn't one.
            Extraction::Malformed => {
                let message = match &response.stop {
                    Some(reason) => format!("no query found in response (stop reason: {reason})"),
                    None => "no query found in response".to_string(),
                };
                (None, 0, Err(Fault::Retryable(message)))
            }
        };
        Ok(AttemptOutcome {
            query,
            tokens: response.tokens,
            // Model+DB work only: harness backoff sleeps and failed
            // transport calls are infra noise and excluded.
            latency_ms: provider_latency_ms + db_latency_ms,
            db_latency_ms,
            response_text: response.text,
            outcome,
        })
    }

    /// Retry transient provider errors and timeouts with exponential
    /// backoff; fatal errors and exhausted retries end the attempt (the
    /// caller decides whether that ends the repetition or the run). Returns
    /// the response and the latency of the successful call only.
    async fn send_with_backoff(
        &self,
        conversation: &[Message],
    ) -> Result<(ModelResponse, u64), RunError> {
        let mut transient_failures = 0;
        loop {
            let started = Instant::now();
            let outcome =
                tokio::time::timeout(PROVIDER_TIMEOUT, self.model.send_prompt(conversation))
                    .await
                    .unwrap_or_else(|_| {
                        Err(ProviderError::Timeout(format!(
                            "provider exceeded the harness ceiling of {PROVIDER_TIMEOUT:?}"
                        )))
                    });
            let latency_ms = started.elapsed().as_millis() as u64;
            match outcome {
                Ok(response) => return Ok((response, latency_ms)),
                Err(ProviderError::Transient(_) | ProviderError::Timeout(_))
                    if transient_failures < TRANSIENT_RETRIES =>
                {
                    tokio::time::sleep(TRANSIENT_BACKOFF * 2u32.pow(transient_failures)).await;
                    transient_failures += 1;
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    /// Execute the query, retrying infrastructure errors with exponential
    /// backoff — the DB-side mirror of `send_with_backoff`. Model-fault
    /// errors return immediately as retryable faults for the model. The
    /// latency covers the final (non-infra) call only.
    async fn query_with_backoff(
        &self,
        query: &str,
    ) -> Result<(Result<Value, Fault>, u64), DbAbort> {
        let mut infra_failures = 0;
        loop {
            let started = Instant::now();
            let outcome = tokio::time::timeout(DB_TIMEOUT, self.db.send_query(query))
                .await
                .unwrap_or_else(|_| {
                    Err(QueryError::Infrastructure(format!(
                        "query exceeded the harness ceiling of {DB_TIMEOUT:?}"
                    )))
                });
            let latency_ms = started.elapsed().as_millis() as u64;
            match outcome {
                Ok(value) => return Ok((Ok(value), latency_ms)),
                Err(e) if e.is_model_fault() => {
                    return Ok((Err(Fault::Retryable(e.to_string())), latency_ms));
                }
                Err(e) if infra_failures < INFRA_RETRIES => {
                    eprintln!("Received infra failure {}: {}", infra_failures, e);
                    tokio::time::sleep(INFRA_BACKOFF * 2u32.pow(infra_failures)).await;
                    infra_failures += 1;
                }
                Err(e) => {
                    return Err(DbAbort {
                        error: RunError::Infrastructure(e.to_string()),
                        db_latency_ms: latency_ms,
                    });
                }
            }
        }
    }

    fn build_record(
        &self,
        repetition: u32,
        attempts: Vec<Attempt>,
        result: RecordResult,
        accurate: bool,
    ) -> ResultRecord {
        let (tokens, latency_ms, db_latency_ms) = attempt_totals(&attempts);
        ResultRecord {
            model: self.model.model_id(),
            max_retries: self.max_retries,
            retries_used: attempts.len().saturating_sub(1) as u32,
            examples: self.examples.len() as u32,
            skills: self.skills.is_some(),
            repetition,
            // The last extractable query, matching derive_retry_level's rule.
            generated: attempts
                .iter()
                .rev()
                .find_map(|a| a.query.clone())
                .unwrap_or_default(),
            attempts,
            tokens,
            latency_ms,
            db_latency_ms,
            result,
            accurate,
        }
    }
}

#[cfg(test)]
mod run_tests {
    use std::collections::BTreeMap;

    use bench_core::{QueryError, Value};
    use db_dummy::DummyDb;
    use provider_dummy::DummyProvider;

    use super::*;

    fn question(expected: Value) -> Question {
        Question {
            question: "How many cars are there?".to_string(),
            difficulty: "easy".to_string(),
            unanswerable: false,
            expected: Some(expected),
            ordered: false,
            queries: BTreeMap::new(),
            expected_by_db: BTreeMap::new(),
            divergence: None,
        }
    }

    fn unanswerable_question() -> Question {
        Question {
            question: "What colour is each car?".to_string(),
            difficulty: "easy".to_string(),
            unanswerable: true,
            expected: None,
            ordered: false,
            queries: BTreeMap::new(),
            expected_by_db: BTreeMap::new(),
            divergence: None,
        }
    }

    #[test]
    fn question_text_appends_a_return_shape_instruction() {
        let q = "How many cars are there?";
        // Scalars: type + single-value cardinality.
        assert_eq!(
            question_text(&question(Value::Int(3))),
            format!("{q}\nReturn a single integer.")
        );
        assert_eq!(
            question_text(&question(Value::String("Tesla".into()))),
            format!("{q}\nReturn a single text value.")
        );
        assert_eq!(
            question_text(&question(Value::Bool(true))),
            format!("{q}\nReturn a single boolean value, true or false.")
        );
        // A list of scalars: element type + list cardinality.
        assert_eq!(
            question_text(&question(Value::List(vec![Value::String("Ford".into())]))),
            format!("{q}\nReturn a list of text values.")
        );
        // Objects keep the exact field-naming instruction (load-bearing for
        // matching), now with single-row cardinality.
        let object = Value::Object(BTreeMap::from([
            ("model".to_string(), Value::String("Model 3".into())),
            ("brand".to_string(), Value::String("Tesla".into())),
        ]));
        assert_eq!(
            question_text(&question(object.clone())),
            format!("{q}\nReturn a single row. Name the output fields exactly: brand, model.")
        );
        // A list of row objects: one row per result, same field naming.
        assert_eq!(
            question_text(&question(Value::List(vec![object]))),
            format!(
                "{q}\nReturn one row per result. Name the output fields exactly: brand, model."
            )
        );
        // Unanswerable (no expected) gets the bare question, no instruction.
        let unanswerable = unanswerable_question();
        assert_eq!(question_text(&unanswerable), unanswerable.question);
    }

    fn runner<'a>(db: &'a DummyDb, provider: &'a DummyProvider) -> BenchmarkRunner<'a> {
        runner_with_retries(db, provider, 0)
    }

    fn runner_with_retries<'a>(
        db: &'a DummyDb,
        provider: &'a DummyProvider,
        max_retries: u32,
    ) -> BenchmarkRunner<'a> {
        BenchmarkRunner {
            db,
            db_id: "dummy",
            model: provider,
            prompt_template: "{{schema}}\n{{examples}}\n{{skills}}\nQ: {{question}}".to_string(),
            schema: "cars have ages".to_string(),
            examples: vec![],
            skills: None,
            max_retries,
        }
    }

    /// The dummy provider errors when its script runs out, so every test
    /// scripts one response per repetition.
    fn per_repetition(response: &str) -> DummyProvider {
        DummyProvider::new(vec![response; REPETITIONS as usize])
    }

    #[tokio::test]
    async fn accurate_when_result_matches_expected() {
        let db = DummyDb::new();
        let provider = per_repetition("The count:\n```\n3\n```");
        let runs = runner(&db, &provider)
            .run(&[question(Value::Int(3))])
            .await
            .unwrap();

        assert_eq!(runs.len(), 1);
        let records = &runs[0].records;
        assert_eq!(records.len(), REPETITIONS as usize);
        assert!(records.iter().all(|r| r.accurate));
        assert!(records.iter().all(|r| r.attempts[0].error.is_none()));
        assert_eq!(records[0].generated, "3");
        assert_eq!(records[0].model, "dummy");
        assert!(records[0].tokens.output > 0);
        for (index, record) in records.iter().enumerate() {
            assert_eq!(record.repetition, index as u32 + 1);
        }
        // The assembled prompt carried the question and schema.
        let first_prompt = &provider.conversations()[0][0].content;
        assert!(first_prompt.contains("How many cars are there?"));
        assert!(first_prompt.contains("cars have ages"));
    }

    #[tokio::test]
    async fn wrong_result_is_recorded_but_inaccurate() {
        let db = DummyDb::new();
        let provider = per_repetition("```\n4\n```");
        let runs = runner(&db, &provider)
            .run(&[question(Value::Int(3))])
            .await
            .unwrap();

        let record = &runs[0].records[0];
        assert!(!record.accurate);
        // A wrong-but-valid result is not an error: the query ran fine.
        assert!(record.attempts[0].error.is_none());
        assert!(matches!(&record.result, RecordResult::Value(Value::Int(4))));
    }

    #[tokio::test]
    async fn model_fault_query_error_is_recorded() {
        let db = DummyDb::new();
        let provider = per_repetition("```\nnot valid json\n```");
        let runs = runner(&db, &provider)
            .run(&[question(Value::Int(3))])
            .await
            .unwrap();

        let record = &runs[0].records[0];
        assert!(!record.accurate);
        assert!(matches!(record.result, RecordResult::Error));
        let error = record.attempts[0].error.as_deref().unwrap();
        assert!(error.contains("syntax error"), "got: {error}");
        assert_eq!(record.generated, "not valid json");
    }

    #[tokio::test]
    async fn malformed_response_is_recorded_without_a_query() {
        let db = DummyDb::new();
        let provider = per_repetition("I think the answer is 3.");
        let runs = runner(&db, &provider)
            .run(&[question(Value::Int(3))])
            .await
            .unwrap();

        let record = &runs[0].records[0];
        assert!(!record.accurate);
        assert!(record.attempts[0].query.is_none());
        assert_eq!(record.generated, "");
        assert_eq!(
            record.attempts[0].error.as_deref(),
            Some("no query found in response")
        );
    }

    #[tokio::test]
    async fn unanswerable_is_scored_as_failure() {
        let db = DummyDb::new();
        let provider = per_repetition("UNANSWERABLE");
        let runs = runner(&db, &provider)
            .run(&[question(Value::Int(3))])
            .await
            .unwrap();

        let record = &runs[0].records[0];
        assert!(!record.accurate);
        assert_eq!(
            record.attempts[0].error.as_deref(),
            Some("declared UNANSWERABLE")
        );
    }

    #[tokio::test]
    async fn declining_an_unanswerable_question_is_accurate() {
        let db = DummyDb::new();
        let provider = per_repetition("UNANSWERABLE");
        let runs = runner(&db, &provider)
            .run(&[unanswerable_question()])
            .await
            .unwrap();

        for record in &runs[0].records {
            assert!(record.accurate);
            assert!(matches!(record.result, RecordResult::Unanswerable));
            assert!(record.attempts[0].error.is_none());
            assert!(record.attempts[0].query.is_none());
            assert_eq!(record.generated, "");
        }
    }

    #[tokio::test]
    async fn a_value_on_an_unanswerable_question_is_inaccurate_and_terminal() {
        let db = DummyDb::new();
        let provider = DummyProvider::new(["```\n3\n```"]).repeating();
        let runs = runner_with_retries(&db, &provider, 2)
            .run(&[unanswerable_question()])
            .await
            .unwrap();

        let record = &runs[0].records[0];
        assert!(!record.accurate);
        // The query ran fine, so this is a wrong result, not an error —
        // and wrong-but-valid results are never retried.
        assert!(matches!(&record.result, RecordResult::Value(Value::Int(3))));
        assert!(record.attempts[0].error.is_none());
        assert_eq!(record.attempts.len(), 1);
    }

    #[tokio::test]
    async fn query_errors_on_an_unanswerable_question_still_earn_retries() {
        let db = DummyDb::new();
        // A failing query first, then the model realizes and declines.
        let provider = DummyProvider::new(["```\nnot json\n```", "UNANSWERABLE"]).repeating();
        let runs = runner_with_retries(&db, &provider, 2)
            .run(&[unanswerable_question()])
            .await
            .unwrap();

        for record in &runs[0].records {
            assert!(record.accurate);
            assert!(matches!(record.result, RecordResult::Unanswerable));
            assert_eq!(record.attempts.len(), 2);
            assert!(record.attempts[0].error.is_some());
            assert!(record.attempts[1].error.is_none());
        }
    }

    #[tokio::test(start_paused = true)]
    async fn infrastructure_error_recovers_with_backoff() {
        let db = DummyDb::new();
        // One dropped connection, then the JSON-echo fallback succeeds.
        db.script(Err(QueryError::Infrastructure(
            "connection reset".to_string(),
        )));
        let provider = per_repetition("```\n3\n```");
        let runs = runner(&db, &provider)
            .run(&[question(Value::Int(3))])
            .await
            .unwrap();

        assert!(runs[0].records.iter().all(|r| r.accurate));
        // Infra retries are invisible to the trace and the model's budget.
        assert!(runs[0].records.iter().all(|r| r.attempts.len() == 1));
    }

    #[tokio::test(start_paused = true)]
    async fn persistent_infrastructure_errors_abort_with_partials() {
        // Question 1 succeeds for all repetitions; question 2 hits infra
        // errors beyond the harness budget on its first repetition.
        let db = DummyDb::new();
        for _ in 0..REPETITIONS {
            db.script(Ok(Value::Int(3)));
        }
        for _ in 0..=INFRA_RETRIES {
            db.script(Err(QueryError::Infrastructure(
                "db unreachable".to_string(),
            )));
        }
        let provider = DummyProvider::new(["```\n3\n```"]).repeating();
        let failure = runner(&db, &provider)
            .run(&[question(Value::Int(3)), question(Value::Int(3))])
            .await
            .unwrap_err();

        assert!(matches!(failure.error, RunError::Infrastructure(_)));
        assert_eq!(failure.question_index, 1);
        assert_eq!(failure.repetition, 1);
        // The completed question survives the abort.
        assert_eq!(failure.completed.len(), 2);
        assert_eq!(failure.completed[0].records.len(), REPETITIONS as usize);
        assert!(failure.completed[0].records.iter().all(|r| r.accurate));
        // ...and so does the repetition that died, carrying its query.
        let aborted = &failure.completed[1];
        assert_eq!(aborted.question_index, 1);
        assert_eq!(aborted.records.len(), 1);
        assert_eq!(aborted.records[0].generated, "3");
        assert!(!aborted.records[0].accurate);
    }

    #[tokio::test]
    async fn provider_error_aborts_the_run() {
        let db = DummyDb::new();
        let provider = DummyProvider::new(Vec::<String>::new());
        let failure = runner(&db, &provider)
            .run(&[question(Value::Int(3))])
            .await
            .unwrap_err();
        assert!(matches!(failure.error, RunError::Provider(_)));
        assert!(failure.completed.is_empty());
    }

    #[tokio::test]
    async fn retry_recovers_from_model_fault() {
        let db = DummyDb::new();
        // Each repetition consumes both entries: syntax error, then success.
        let provider = DummyProvider::new(["```\nnot json\n```", "```\n3\n```"]).repeating();
        let runs = runner_with_retries(&db, &provider, 2)
            .run(&[question(Value::Int(3))])
            .await
            .unwrap();

        for record in &runs[0].records {
            assert!(record.accurate);
            assert_eq!(record.max_retries, 2);
            assert_eq!(record.retries_used, 1);
            assert_eq!(record.attempts.len(), 2);
            assert!(
                record.attempts[0]
                    .error
                    .as_deref()
                    .unwrap()
                    .contains("syntax error")
            );
            assert!(record.attempts[1].error.is_none());
            assert_eq!(record.generated, "3");
            // Totals sum both attempts.
            let expected_output: u64 = record.attempts.iter().map(|a| a.tokens.output).sum();
            assert_eq!(record.tokens.output, expected_output);
        }
        // The retry conversation carried the original prompt, the model's
        // response, and the error feedback.
        let retry_conversation = &provider.conversations()[1];
        assert_eq!(retry_conversation.len(), 3);
        assert!(retry_conversation[2].content.contains("syntax error"));
    }

    #[tokio::test]
    async fn budget_exhaustion_is_an_error_record() {
        let db = DummyDb::new();
        let provider = DummyProvider::new(["no code here"]).repeating();
        let runs = runner_with_retries(&db, &provider, 2)
            .run(&[question(Value::Int(3))])
            .await
            .unwrap();

        let record = &runs[0].records[0];
        assert!(!record.accurate);
        assert!(matches!(record.result, RecordResult::Error));
        assert_eq!(record.retries_used, 2);
        assert_eq!(record.attempts.len(), 3);
        // Malformed responses get formatting feedback, not a bogus "query
        // failed" message.
        let feedback = &provider.conversations()[1][2].content;
        assert!(
            feedback.contains("did not contain a fenced code block"),
            "got: {feedback}"
        );
    }

    #[tokio::test]
    async fn unanswerable_is_never_retried() {
        let db = DummyDb::new();
        let provider = DummyProvider::new(["UNANSWERABLE"]).repeating();
        let runs = runner_with_retries(&db, &provider, 2)
            .run(&[question(Value::Int(3))])
            .await
            .unwrap();

        let record = &runs[0].records[0];
        assert_eq!(record.attempts.len(), 1);
        assert_eq!(record.retries_used, 0);
        assert!(!record.accurate);
    }

    #[tokio::test]
    async fn wrong_but_valid_result_is_never_retried() {
        let db = DummyDb::new();
        let provider = DummyProvider::new(["```\n4\n```"]).repeating();
        let runs = runner_with_retries(&db, &provider, 2)
            .run(&[question(Value::Int(3))])
            .await
            .unwrap();

        let record = &runs[0].records[0];
        assert_eq!(record.attempts.len(), 1);
        assert!(matches!(&record.result, RecordResult::Value(Value::Int(4))));
    }

    #[tokio::test(start_paused = true)]
    async fn transient_provider_errors_are_backed_off_and_retried() {
        let db = DummyDb::new();
        let provider = DummyProvider::new(Vec::<String>::new());
        for _ in 0..REPETITIONS {
            provider.push_transient_error("rate limited");
            provider.push_response("```\n3\n```");
        }
        let runs = runner(&db, &provider)
            .run(&[question(Value::Int(3))])
            .await
            .unwrap();

        assert!(runs[0].records.iter().all(|r| r.accurate));
        // Transient failures don't touch the model's budget or the trace.
        assert!(runs[0].records.iter().all(|r| r.retries_used == 0));
        assert_eq!(provider.conversations().len(), (REPETITIONS * 2) as usize);
    }

    #[tokio::test(start_paused = true)]
    async fn persistent_transient_errors_eventually_abort() {
        let db = DummyDb::new();
        let provider = DummyProvider::new(Vec::<String>::new());
        // One more than the harness's transient budget.
        for _ in 0..=TRANSIENT_RETRIES {
            provider.push_transient_error("rate limited");
        }
        let failure = runner(&db, &provider)
            .run(&[question(Value::Int(3))])
            .await
            .unwrap_err();
        assert!(matches!(
            failure.error,
            RunError::Provider(ProviderError::Transient(_))
        ));
    }

    #[tokio::test(start_paused = true)]
    async fn provider_timeouts_are_backed_off_and_retried() {
        let db = DummyDb::new();
        let provider = DummyProvider::new(Vec::<String>::new());
        for _ in 0..REPETITIONS {
            provider.push_timeout_error("took too long");
            provider.push_response("```\n3\n```");
        }
        let runs = runner(&db, &provider)
            .run(&[question(Value::Int(3))])
            .await
            .unwrap();

        assert!(runs[0].records.iter().all(|r| r.accurate));
        // Timeouts don't touch the model's budget or the trace.
        assert!(runs[0].records.iter().all(|r| r.retries_used == 0));
    }

    #[tokio::test(start_paused = true)]
    async fn persistent_provider_timeouts_end_the_repetition_not_the_run() {
        let db = DummyDb::new();
        let provider = DummyProvider::new(Vec::<String>::new());
        // Repetition 1: a model-fault attempt, then timeouts past the
        // harness budget.
        provider.push_response("```\nnot valid json\n```");
        for _ in 0..=TRANSIENT_RETRIES {
            provider.push_timeout_error("took too long");
        }
        // The remaining repetitions recover.
        for _ in 1..REPETITIONS {
            provider.push_response("```\n3\n```");
        }
        let runs = runner_with_retries(&db, &provider, 1)
            .run(&[question(Value::Int(3))])
            .await
            .unwrap();

        let records = &runs[0].records;
        assert_eq!(records.len(), REPETITIONS as usize);
        // The timed-out repetition is an error record carrying both the
        // earlier attempt and the timeout that ended it.
        let timed_out = &records[0];
        assert!(!timed_out.accurate);
        assert!(matches!(timed_out.result, RecordResult::Error));
        assert_eq!(timed_out.attempts.len(), 2);
        assert!(
            timed_out.attempts[0]
                .error
                .as_deref()
                .unwrap()
                .contains("syntax error")
        );
        let last = timed_out.attempts.last().unwrap();
        assert!(last.query.is_none());
        assert_eq!(last.error.as_deref(), Some("took too long"));
        assert_eq!(last.tokens, TokenUsage::default());
        // The run carried on to the remaining repetitions.
        assert!(records[1..].iter().all(|r| r.accurate));
    }

    #[tokio::test(start_paused = true)]
    async fn hung_provider_costs_the_repetitions_not_the_run() {
        struct HangingProvider;

        #[async_trait::async_trait]
        impl ModelProvider for HangingProvider {
            fn model_id(&self) -> String {
                "hang".to_string()
            }

            async fn send_prompt(
                &self,
                _conversation: &[Message],
            ) -> Result<ModelResponse, ProviderError> {
                std::future::pending().await
            }
        }

        let db = DummyDb::new();
        let provider = HangingProvider;
        let runs = BenchmarkRunner {
            db: &db,
            db_id: "dummy",
            model: &provider,
            prompt_template: "{{question}}".to_string(),
            schema: String::new(),
            examples: vec![],
            skills: None,
            max_retries: 0,
        }
        .run(&[question(Value::Int(3))])
        .await
        .unwrap();

        // Every repetition hit the harness ceiling, was recorded, and the
        // run still completed.
        let records = &runs[0].records;
        assert_eq!(records.len(), REPETITIONS as usize);
        for record in records {
            assert!(!record.accurate);
            assert!(matches!(record.result, RecordResult::Error));
            assert!(
                record.attempts[0]
                    .error
                    .as_deref()
                    .is_some_and(|e| e.contains("harness ceiling")),
                "the attempt should say why there is no response: {:?}",
                record.attempts[0].error
            );
        }
    }

    #[tokio::test(start_paused = true)]
    async fn hung_db_becomes_a_diagnosable_infrastructure_failure() {
        struct HangingDb;

        #[async_trait::async_trait]
        impl Database for HangingDb {
            fn query_language(&self) -> &'static str {
                "dummy"
            }

            async fn send_query(&self, _query: &str) -> Result<Value, QueryError> {
                std::future::pending().await
            }
        }

        let db = HangingDb;
        let provider = per_repetition("```\n3\n```");
        let failure = BenchmarkRunner {
            db: &db,
            db_id: "dummy",
            model: &provider,
            prompt_template: "{{question}}".to_string(),
            schema: String::new(),
            examples: vec![],
            skills: None,
            max_retries: 0,
        }
        .run(&[question(Value::Int(3))])
        .await
        .unwrap_err();

        match &failure.error {
            RunError::Infrastructure(message) => {
                assert!(message.contains("harness ceiling"), "got: {message}")
            }
            other => panic!("expected infrastructure error, got {other:?}"),
        }

        // The query that ended the run survives into the results. An
        // infrastructure failure is often a generated query that took the DB
        // down, so discarding it would hide the only evidence of what did it.
        let records: Vec<_> = failure
            .completed
            .iter()
            .flat_map(|run| &run.records)
            .collect();
        assert_eq!(
            records.len(),
            1,
            "the aborted repetition should be recorded"
        );
        let record = records[0];
        assert_eq!(record.generated, "3");
        assert!(matches!(record.result, RecordResult::Error));
        assert!(!record.accurate);
        let last = record.attempts.last().expect("the failing attempt");
        assert_eq!(last.query.as_deref(), Some("3"));
        assert!(
            last.error
                .as_deref()
                .is_some_and(|e| e.contains("harness ceiling")),
            "the attempt should carry why the run ended: {:?}",
            last.error
        );
    }

    /// The core claim of the run-at-max/derive-lower design: deriving a
    /// lower level from a max-level trace produces the same record an
    /// actual run at that level would (latency aside — wall-clock isn't
    /// reproducible).
    #[tokio::test]
    async fn derived_levels_match_actual_low_budget_runs() {
        let script = ["no code here", "```\nnot json\n```", "```\n3\n```"];

        let mut actual_by_level = Vec::new();
        for max_retries in [0, 2] {
            let db = DummyDb::new();
            let provider = DummyProvider::new(script).repeating();
            let runs = runner_with_retries(&db, &provider, max_retries)
                .run(&[question(Value::Int(3))])
                .await
                .unwrap();
            // Repetition 1 starts at the same script position at any level.
            actual_by_level.push(runs[0].records[0].clone());
        }

        let db = DummyDb::new();
        let provider = DummyProvider::new(script).repeating();
        let high = runner_with_retries(&db, &provider, 4)
            .run(&[question(Value::Int(3))])
            .await
            .unwrap();
        let max_record = &high[0].records[0];

        for (actual, level) in actual_by_level.iter().zip([0, 2]) {
            let derived = bench_output::derive_retry_level(max_record, level);
            assert_eq!(
                strip_latency(actual),
                strip_latency(&derived),
                "level {level} derived record diverged from an actual run"
            );
        }
    }

    fn strip_latency(record: &ResultRecord) -> serde_json::Value {
        let mut value = serde_json::to_value(record).unwrap();
        value["latencyMs"] = 0.into();
        for attempt in value["attempts"].as_array_mut().unwrap() {
            attempt["latencyMs"] = 0.into();
        }
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_last_fenced_block() {
        let response = "First try:\n```sql\nSELECT 1;\n```\nActually:\n```sql\nSELECT 2;\n```\n";
        assert_eq!(
            extract_query(response),
            Extraction::Query("SELECT 2;".to_string())
        );
    }

    #[test]
    fn unanswerable_token_on_its_own_line_is_unanswerable() {
        assert_eq!(
            extract_query("UNANSWERABLE\nThe schema has no such attribute."),
            Extraction::Unanswerable
        );
    }

    #[test]
    fn token_mentioned_in_prose_is_malformed() {
        assert_eq!(
            extract_query("I shouldn't say UNANSWERABLE here, but there is no query."),
            Extraction::Malformed
        );
    }

    #[test]
    fn no_block_and_no_token_is_malformed() {
        assert_eq!(
            extract_query("The query you want is SELECT 1;"),
            Extraction::Malformed
        );
    }

    #[test]
    fn bare_query_without_a_fence_is_accepted_in_each_language() {
        for query in [
            "SELECT COUNT(*) FROM person;",
            "WITH t AS (SELECT 1) SELECT * FROM t;",
            "MATCH (p:Person) RETURN count(p)",
            "OPTIONAL MATCH (p:Person) RETURN p",
            "match $p isa person; reduce $count = count;",
            "with fun f() -> integer: match $x isa thing; return count;",
        ] {
            assert_eq!(
                extract_query(query),
                Extraction::Query(query.to_string()),
                "should have accepted bare query: {query}"
            );
        }
    }

    #[test]
    fn bare_query_fallback_leaves_the_other_paths_alone() {
        // A fence still wins, even when prose before it opens with a keyword.
        assert_eq!(
            extract_query("SELECT is the keyword you want.\n```sql\nSELECT 2;\n```"),
            Extraction::Query("SELECT 2;".to_string())
        );
        // Declining still beats the fallback.
        assert_eq!(extract_query("UNANSWERABLE"), Extraction::Unanswerable);
        // Prose that merely mentions a query is still malformed: the reply has
        // to *begin* with an opener.
        assert_eq!(
            extract_query("The query you want is SELECT 1;"),
            Extraction::Malformed
        );
        assert_eq!(
            extract_query("I cannot help with that."),
            Extraction::Malformed
        );
        assert_eq!(extract_query(""), Extraction::Malformed);
    }

    #[test]
    fn unterminated_trailing_fence_still_counts() {
        assert_eq!(
            extract_query("Here you go:\n```sql\nSELECT 1;"),
            Extraction::Query("SELECT 1;".to_string())
        );
    }

    #[test]
    fn examples_heading_appears_only_with_examples() {
        let template = "{{schema}}\n{{examples}}\n{{question}}";
        let without = assemble_prompt(template, "q", "s", &[], &[]);
        assert!(!without.contains("Examples:"));
        let with = assemble_prompt(
            template,
            "q",
            "s",
            &["e1".to_string(), "e2".to_string()],
            &[],
        );
        assert!(with.contains("Examples:\ne1\n\ne2"));
    }
}
