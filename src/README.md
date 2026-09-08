# Benchmark runner

A Rust workspace that takes a config, a question set and a list of models, has
each model write a query per question and per DB, executes it, and scores the
result against the question's expected value.

```sh
cargo run -p bench-cli -- src/reactome.yml results-reactome.json   # run
cargo run -p bench-cli --bin verify -- src/reactome.yml             # check the reference queries
```

The DBs named in the config must be up first (see
[`../databases`](../databases/README.md)). `reactome.yml` is the format
reference for the config; the sections below describe what it controls.

## Crates

| Crate | Role |
| --- | --- |
| `core` | The canonical result `Value`, the `Database` and `ModelProvider` traits, the question and output types. |
| `config` | Loads and validates the YAML config and the questions file. |
| `dbs/{typedb,neo4j,sql}` | One package per DB: runs a query read-only under a timeout and coerces the driver's result into `Value`. `sql` speaks MySQL and PostgreSQL, picked from the URL scheme. |
| `providers/{claude,openai-compatible}` | One package per model API, each a small `reqwest` client exposing `send_prompt`. `openai-compatible` covers anything speaking the chat-completions API (OpenAI, Groq, OpenRouter, local Ollama/vLLM). |
| `runner` | Runs the questions for one DB × model × example count × skills setup, with the retry loop. |
| `output` | Marshals runs into the results file and derives the lower retry levels from the attempt trace. |
| `cli` | The `db-llm-bench` and `verify` binaries. |

The DB and provider packages sit behind shared traits, so adding a store or a
model API is a new crate plus an ID in the config, and result coercion for
each DB is a compiler-checked mapping into one value type.

## Config

- **`dbs`** — a list of `<id>: {...}` entries, ID one of `typedb`, `neo4j`,
  `sql`. Each has a `url`, optional `database` (for servers that namespace by
  database) and `auth`, a `schema` file handed to the model, a `prompts`
  template, an optional `examples` folder of `example-1.txt`, `example-2.txt`,
  … (contiguous from 1; without one the DB can only run at zero examples), and
  an optional `skills` folder whose `.md` files fill the template's skills slot.
- **`models`** — a list of `<provider>: {...}` entries. `claude` takes `model`,
  `max_tokens`, optional `thinking`/`effort`, and reads `ANTHROPIC_API_KEY`
  unless `api_key` is set. `openai-compatible` takes `base_url`, `model`,
  optional `api_key_env` (omit for unauthenticated local servers) and
  `max_tokens_field` for OpenAI's newer models that want
  `max_completion_tokens`. A provider may appear several times; each entry's
  `label` (defaulting to the model name) must be unique.
- **`questionsPath`** — the questions file (below).
- **`exampleCounts`** — the few-shot counts to run at; each puts that many of
  the DB's example files into the prompt.
- **`maxRetryCounts`** — the retry budgets to report at. Runs execute once at
  the highest; the lower levels are derived by truncating the attempt trace.
- **`skillsBaseline`** (default `true`) — whether a DB that configures skills
  also runs skills-off, for the on/off comparison.

Prompt templates are dataset-independent (`data/prompts/<db>.txt`) and use the
slots `{{question}}`, `{{schema}}`, `{{examples}}` and `{{skills}}`. The
examples slot renders as an `Examples:` heading plus the examples, or nothing
at all at zero examples. The template must ask for exactly one fenced code
block holding the query, or the token `UNANSWERABLE` alone on a line.

Everything is validated before any model is called — providers and DB clients
are built, each DB answers a health check, prompt assets load, every DB has
enough example files, templates carry the slots the run needs, and every
answerable question has a reference query for each configured DB's language —
so a bad configuration fails at once rather than partway through.

## Questions

A JSON file with a `questions` list. Each answerable question has `question`,
`difficulty`, `expected`, and `queries` — the reference query per language,
keyed `typeql` / `cypher` / `sql`. Optional fields:

- `expected_by_db` — a per-DB expected value where the stores' dumps genuinely
  disagree; the store is scored against its own value. `divergence` documents
  the cause.
- `ordered: true` — the order of a top-level list result is part of the answer.
  By default lists compare as bags; nested lists always compare ordered.
- `unanswerable: true` — the question deliberately has no answer against the
  schema. It then carries no `expected` or `queries` (a half-edited mix is
  rejected at load), and the only correct response is `UNANSWERABLE`.

The runner appends a return-shape instruction to every answerable prompt,
derived from `expected`'s type: a scalar gives "Return a single integer/number/
text value/boolean", a list of scalars "Return a list of … values", and an
object or list of row objects "Return a single row / one row per result. Name
the output fields exactly: …" — so `expected` is the single source of truth for
the shape, and a right answer in the wrong shape is a miss. Two leniencies
keep that about shape rather than labelling: the Neo4j package strips plain
property-access prefixes (`c.brand` → `brand`) so unaliased Cypher is not
penalised for that alone, and a one-field object (TypeDB `fetch { "count": $n
}`, a Cypher map) compares as its value wherever a bare value was expected —
at top level or per list element — since the field name was the model's to
choose. Objects expected as objects keep their field names.

## What a run does

For each DB × model × example count × skills setting, every question runs
three times (repetitions), because models are non-deterministic:

1. Fill the template and send it to the model.
2. Take the last fenced code block as the query (an unterminated trailing block
   still counts, so a truncated response yields a real execution error rather
   than "no query"). An explicit `UNANSWERABLE` is terminal: correct for an
   unanswerable question, a failure otherwise. A response with neither is a
   retryable "no query found".
3. Execute it. Anything that cannot be a right answer — syntax error, timeout,
   wrong result shape — is fed back to the model with the prior conversation,
   up to the highest configured retry count. Empty results are not errors.
   Infrastructure failures (connection drops, provider rate limits) are retried
   by the harness with backoff and do not count against the model.
4. Compare to `expected` with the per-DB coercion and the rules above. Floats
   compare exactly.

## Output

One JSON file, keyed by question then DB ID (not language, so two DBs sharing
a language would not collide). Each record carries the model label, retry
level, retries used, example count, skills flag, repetition, the generated
query, the full attempt trace with per-attempt tokens and latency (model plus
DB time; harness backoff excluded), the coerced result and `accurate`. If a run
aborts on an unrecoverable failure, everything completed so far is still
written. The `analysis/` scripts read this file.

```json
{
  "questions": [
    {
      "question": "How many cars are there?",
      "difficulty": "easy",
      "expected": 3,
      "dbs": {
        "sql": {
          "language": "sql",
          "correct": "SELECT COUNT(*) FROM Cars;",
          "results": [
            {
              "model": "claude-sonnet-5",
              "maxRetries": 0,
              "retriesUsed": 0,
              "examples": 0,
              "skills": false,
              "repetition": 1,
              "generated": "SELECT COUNT(*) FROM Cars;",
              "attempts": [
                {
                  "query": "SELECT COUNT(*) FROM Cars;",
                  "tokens": { "input": 1150, "output": 30 },
                  "latencyMs": 850,
                  "dbLatencyMs": 12,
                  "error": null
                }
              ],
              "tokens": { "input": 1150, "output": 30 },
              "latencyMs": 850,
              "dbLatencyMs": 12,
              "result": 3,
              "accurate": true
            }
          ]
        }
      }
    }
  ]
}
```

## verify

`verify <config.yml> [questions.json] [--timings <path>]` runs every
answerable question's reference query against the live DBs and reports each
result that does not match `expected` (or `expected_by_db`), through the same
execute → coerce → compare path the benchmark scores with. `--timings` writes
each query's execution time as `{db: {question: ms}}`, the baseline
`analysis/query_time.py` compares generated queries against.
