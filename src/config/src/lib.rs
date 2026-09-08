//! Config file processor: parses `config.yml` and loads the questions file.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use bench_core::question::QuestionFile;
use serde::{Deserialize, Deserializer};
use serde_yaml2::wrapper::YamlNodeWrapper;
use thiserror::Error;

/// serde_yaml2 can't deserialize a YAML mapping directly into
/// `serde_json::Value`, so free-form fields deserialize as its
/// `YamlNodeWrapper` and are transcoded through Serialize.
fn transcode(node: YamlNodeWrapper) -> Result<serde_json::Value, String> {
    serde_json::to_value(node).map_err(|e| e.to_string())
}

fn yaml_any_opt<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<serde_json::Value>, D::Error> {
    Option::<YamlNodeWrapper>::deserialize(deserializer)?
        .map(|node| transcode(node).map_err(serde::de::Error::custom))
        .transpose()
}

fn yaml_any_entries<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<BTreeMap<String, serde_json::Value>>, D::Error> {
    Vec::<BTreeMap<String, YamlNodeWrapper>>::deserialize(deserializer)?
        .into_iter()
        .map(|entry| {
            entry
                .into_iter()
                .map(|(id, node)| Ok((id, transcode(node).map_err(serde::de::Error::custom)?)))
                .collect()
        })
        .collect()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    /// The YAML's list-of-single-key-maps shape is a serialization artifact;
    /// access goes through [`Config::db_entries`].
    dbs: Vec<BTreeMap<String, DbConfig>>,
    /// Each entry maps a provider ID to its provider-specific config, which
    /// the matching provider package interprets (extension stays localised).
    /// Held as `serde_json::Value` so the YAML library never appears in this
    /// crate's API. Access goes through [`Config::model_entries`].
    #[serde(deserialize_with = "yaml_any_entries")]
    models: Vec<BTreeMap<String, serde_json::Value>>,
    pub questions_path: PathBuf,
    pub example_counts: Vec<u32>,
    /// Runs only execute at the highest count; lower levels are derived from
    /// the attempt trace.
    pub max_retry_counts: Vec<u32>,
    /// Whether a DB that configures a skill also runs a skills-off baseline
    /// (for on/off comparison). Defaults to true; set false to run skilled DBs
    /// with the skill only. DBs without a skill are unaffected — skills-off is
    /// their only mode either way.
    #[serde(default = "default_true")]
    pub skills_baseline: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct DbConfig {
    /// The prompt template file. Templates are dataset-independent, so the
    /// dataset-specific examples live in [`DbConfig::examples`] instead.
    pub prompts: PathBuf,
    /// Optional folder of example-N.txt files; omitted when a dataset has no
    /// examples, which confines that dataset to an example count of 0.
    pub examples: Option<PathBuf>,
    pub url: String,
    /// Database name within the server, for DBs that namespace by database
    /// (e.g. TypeDB, Neo4j).
    pub database: Option<String>,
    /// DB-specific auth, interpreted by the matching DB package.
    #[serde(default, deserialize_with = "yaml_any_opt")]
    pub auth: Option<serde_json::Value>,
    /// Optional folder of .md skills loaded when generating queries.
    pub skills: Option<PathBuf>,
    /// Schema description inserted into the prompt's schema slot.
    pub schema: PathBuf,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse {source_name}: {message}")]
    Parse {
        source_name: String,
        message: String,
    },
    #[error("invalid config: {0}")]
    Invalid(String),
}

impl FromStr for Config {
    type Err = ConfigError;

    fn from_str(yaml: &str) -> Result<Config, ConfigError> {
        let config: Config = serde_yaml2::from_str(yaml).map_err(|e| ConfigError::Parse {
            source_name: "<string>".to_string(),
            message: e.to_string(),
        })?;
        config.validate()?;
        Ok(config)
    }
}

impl Config {
    pub fn load(path: &Path) -> Result<Config, ConfigError> {
        fs::read_to_string(path)
            .map_err(|source| ConfigError::Io {
                path: path.to_path_buf(),
                source,
            })?
            .parse::<Config>()
            .map_err(|e| match e {
                ConfigError::Parse { message, .. } => ConfigError::Parse {
                    source_name: path.display().to_string(),
                    message,
                },
                other => other,
            })
    }

    pub fn db_entries(&self) -> impl Iterator<Item = (&str, &DbConfig)> {
        self.dbs
            .iter()
            .flat_map(|entry| entry.iter().map(|(id, cfg)| (id.as_str(), cfg)))
    }

    pub fn model_entries(&self) -> impl Iterator<Item = (&str, &serde_json::Value)> {
        self.models
            .iter()
            .flat_map(|entry| entry.iter().map(|(id, cfg)| (id.as_str(), cfg)))
    }

    fn validate(&self) -> Result<(), ConfigError> {
        fn no_duplicates<'a>(
            kind: &str,
            ids: impl Iterator<Item = &'a str>,
        ) -> Result<(), ConfigError> {
            let mut seen = std::collections::BTreeSet::new();
            for id in ids {
                if !seen.insert(id) {
                    return Err(ConfigError::Invalid(format!("duplicate {kind} id: {id}")));
                }
            }
            if seen.is_empty() {
                return Err(ConfigError::Invalid(format!("no {kind}s configured")));
            }
            Ok(())
        }
        no_duplicates("DB", self.db_entries().map(|(id, _)| id))?;
        // Model provider IDs may repeat (e.g. several `claude` blocks
        // benchmarking different models or settings); uniqueness is
        // enforced on the built providers' model labels instead.
        if self.model_entries().next().is_none() {
            return Err(ConfigError::Invalid("no models configured".to_string()));
        }
        // Duplicate counts would silently duplicate whole benchmark runs
        // (or, for retry levels, duplicate derived records) — reject them
        // rather than dedup, so the config says what actually runs.
        fn unique_counts(kind: &str, counts: &[u32]) -> Result<(), ConfigError> {
            if counts.is_empty() {
                return Err(ConfigError::Invalid(format!("{kind} is empty")));
            }
            let mut seen = std::collections::BTreeSet::new();
            for &count in counts {
                if !seen.insert(count) {
                    return Err(ConfigError::Invalid(format!(
                        "duplicate {kind} entry: {count}"
                    )));
                }
            }
            Ok(())
        }
        unique_counts("exampleCounts", &self.example_counts)?;
        unique_counts("maxRetryCounts", &self.max_retry_counts)?;
        Ok(())
    }
}

pub fn load_questions(path: &Path) -> Result<QuestionFile, ConfigError> {
    let raw = fs::read_to_string(path).map_err(|source| ConfigError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    parse_questions(&raw, &path.display().to_string())
}

fn parse_questions(raw: &str, source_name: &str) -> Result<QuestionFile, ConfigError> {
    let questions: QuestionFile = serde_json::from_str(raw).map_err(|e| ConfigError::Parse {
        source_name: source_name.to_string(),
        message: e.to_string(),
    })?;
    if questions.questions.is_empty() {
        return Err(ConfigError::Invalid(format!(
            "{source_name} contains no questions"
        )));
    }
    // An unanswerable question's whole point is that no expected value or
    // ground-truth query exists; the fields must agree so a half-edited
    // question fails loudly instead of scoring nonsense.
    for question in &questions.questions {
        if question.unanswerable {
            if question.expected.is_some()
                || !question.queries.is_empty()
                || question.ordered
                || !question.expected_by_db.is_empty()
            {
                return Err(ConfigError::Invalid(format!(
                    "unanswerable question `{}` must not set expected, ordered, or queries",
                    question.question
                )));
            }
        } else if question.expected.is_none() {
            return Err(ConfigError::Invalid(format!(
                "question `{}` has no expected value (set \"unanswerable\": true if it deliberately has no answer)",
                question.question
            )));
        }
        // A per-store override silences the cross-store agreement that would
        // otherwise catch a wrong reference query, so it has to carry its
        // justification — an unexplained override is indistinguishable from a
        // bug someone papered over.
        if !question.expected_by_db.is_empty() && question.divergence.is_none() {
            return Err(ConfigError::Invalid(format!(
                "question `{}` sets expected_by_db without a `divergence` note explaining why the stores disagree",
                question.question
            )));
        }
        if question.expected_by_db.is_empty() && question.divergence.is_some() {
            return Err(ConfigError::Invalid(format!(
                "question `{}` has a `divergence` note but no expected_by_db",
                question.question
            )));
        }
    }
    Ok(questions)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_flattens_entries() {
        let config: Config = "\
dbs:
  - dummy:
      prompts: prompts/dummy
      url: unused
      auth:
        user: admin
      schema: schema.txt
models:
  - dummy:
      responses: [\"select 1\"]
questionsPath: questions.json
exampleCounts: [0, 1]
maxRetryCounts: [0, 2]
"
        .parse()
        .unwrap();

        let dbs: Vec<_> = config.db_entries().collect();
        assert_eq!(dbs.len(), 1);
        assert_eq!(dbs[0].0, "dummy");
        assert_eq!(dbs[0].1.url, "unused");
        assert_eq!(dbs[0].1.auth, Some(serde_json::json!({"user": "admin"})));
        assert!(dbs[0].1.skills.is_none());
        assert!(config.skills_baseline, "defaults to true when omitted");

        let models: Vec<_> = config.model_entries().collect();
        assert_eq!(models.len(), 1);
        assert_eq!(models[0].0, "dummy");
        assert_eq!(models[0].1, &serde_json::json!({"responses": ["select 1"]}));
    }

    #[test]
    fn skills_baseline_can_be_disabled() {
        let yaml = "\
dbs:
  - dummy:
      prompts: p
      url: u
      schema: s
models:
  - dummy:
      responses: [\"x\"]
questionsPath: q.json
exampleCounts: [0]
maxRetryCounts: [0]
skillsBaseline: false
";
        let config: Config = yaml.parse().unwrap();
        assert!(!config.skills_baseline);
    }

    #[test]
    fn invalid_yaml_is_a_parse_error() {
        assert!(matches!(
            "not: [valid".parse::<Config>(),
            Err(ConfigError::Parse { .. })
        ));
    }

    #[test]
    fn repeated_model_providers_are_allowed() {
        let yaml = "\
dbs:
  - dummy:
      prompts: p
      url: u
      schema: s
models:
  - claude:
      model: claude-opus-4-8
  - claude:
      model: claude-haiku-4-5
questionsPath: q.json
exampleCounts: [0]
maxRetryCounts: [0]
";
        let config: Config = yaml.parse().unwrap();
        assert_eq!(config.model_entries().count(), 2);
        assert!(
            config
                .model_entries()
                .all(|(provider, _)| provider == "claude")
        );
    }

    #[test]
    fn duplicate_db_ids_are_rejected() {
        let yaml = "\
dbs:
  - dummy:
      prompts: p
      url: u
      schema: s
  - dummy:
      prompts: p
      url: u
      schema: s
models:
  - dummy:
      responses: []
questionsPath: q.json
exampleCounts: [0]
maxRetryCounts: [0]
";
        assert!(matches!(
            yaml.parse::<Config>(),
            Err(ConfigError::Invalid(msg)) if msg.contains("duplicate DB id")
        ));
    }

    #[test]
    fn empty_example_counts_are_rejected() {
        let yaml = "\
dbs:
  - dummy:
      prompts: p
      url: u
      schema: s
models:
  - dummy:
      responses: []
questionsPath: q.json
exampleCounts: []
maxRetryCounts: [0]
";
        assert!(matches!(
            yaml.parse::<Config>(),
            Err(ConfigError::Invalid(msg)) if msg.contains("exampleCounts")
        ));
    }

    #[test]
    fn duplicate_counts_are_rejected() {
        let yaml = "\
dbs:
  - dummy:
      prompts: p
      url: u
      schema: s
models:
  - dummy:
      responses: []
questionsPath: q.json
exampleCounts: [0, 3, 3]
maxRetryCounts: [0]
";
        assert!(matches!(
            yaml.parse::<Config>(),
            Err(ConfigError::Invalid(msg)) if msg.contains("duplicate exampleCounts entry: 3")
        ));

        let yaml = yaml
            .replace("exampleCounts: [0, 3, 3]", "exampleCounts: [0]")
            .replace("maxRetryCounts: [0]", "maxRetryCounts: [2, 2]");
        assert!(matches!(
            yaml.parse::<Config>(),
            Err(ConfigError::Invalid(msg)) if msg.contains("duplicate maxRetryCounts entry: 2")
        ));
    }

    #[test]
    fn empty_question_files_are_rejected() {
        assert!(matches!(
            parse_questions(r#"{"questions": []}"#, "q.json"),
            Err(ConfigError::Invalid(msg)) if msg.contains("contains no questions")
        ));
    }

    #[test]
    fn unanswerable_questions_parse_without_expected_or_queries() {
        let questions = parse_questions(
            r#"{"questions": [{"question": "What colour is each car?", "difficulty": "easy", "unanswerable": true}]}"#,
            "q.json",
        )
        .unwrap();
        let question = &questions.questions[0];
        assert!(question.unanswerable);
        assert!(question.expected.is_none());
        assert!(question.queries.is_empty());
    }

    #[test]
    fn unanswerable_questions_must_not_set_answer_fields() {
        for extra in [
            r#""expected": 3"#,
            r#""queries": {"sql": "SELECT 1"}"#,
            r#""ordered": true"#,
        ] {
            let raw = format!(
                r#"{{"questions": [{{"question": "q", "difficulty": "easy", "unanswerable": true, {extra}}}]}}"#
            );
            assert!(
                matches!(
                    parse_questions(&raw, "q.json"),
                    Err(ConfigError::Invalid(msg)) if msg.contains("must not set")
                ),
                "accepted unanswerable question with {extra}"
            );
        }
    }

    #[test]
    fn answerable_questions_require_an_expected_value() {
        assert!(matches!(
            parse_questions(
                r#"{"questions": [{"question": "q", "difficulty": "easy", "queries": {}}]}"#,
                "q.json",
            ),
            Err(ConfigError::Invalid(msg)) if msg.contains("no expected value")
        ));
    }
}
