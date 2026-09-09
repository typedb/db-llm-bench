# Analysis

Scripts for summarising a benchmark results JSON (as written by `bench-cli`).
Python 3, standard library only. Each takes the results path as its argument
and defaults to `results-reactome.json`.

| Script | Reports |
| ------ | ------- |
| `accuracy_by_db.py` | Accuracy per DB × difficulty, collapsing all run variations (highest retry level; averaged over example count, skills, repetitions). |
| `accuracy_by_variation.py` | Accuracy per (model, db, skills, examples, retries) × difficulty — one row per variation, so you can see the effect of skill injection, few-shot count, and retry budget. |
| `incorrect_queries.py` | Every failing run: the question, its config, the expected and generated queries, and the expected vs actual answer. For debugging *what* the model got wrong. Accepts `key=value` filters (`db=`, `difficulty=`, `model=`, `examples=`, `skills=on\|off`). |
| `query_errors.py` | What kind of errors the failing first attempts raise, for one DB (`db=typedb\|neo4j\|sql`, default typedb) — syntax vs type vs semantic vs runtime, classified by TypeDB error code, Neo4j status code plus message, or MySQL message pattern — plus harness-level outcomes (truncation, timeout, spurious UNANSWERABLE), overall and by skills × examples. `model=<substring>` filters. |
| `failure_modes.py` | How the failing runs fail, per DB: visible error vs silently wrong answer, on the first attempt and after the full retry budget, plus a first-attempt breakdown by skills × examples showing whether in-context resources reduce the error rate. Answerable questions only; `model=<substring>` filters. |
| `token_usage.py` | Total model tokens used (input/output), broken down by model and DB, plus run and call counts. `retries=<n>` picks the retry level (default highest; `0` counts first attempts only); `by=variation` splits further by skills × examples with mean output tokens per run; `skills=`, `examples=`, `model=` filter. |
| `query_time.py` | How long the generated queries took to execute, by model and DB (median/mean/p90/slowest). Accurate runs only — a wrong query's execution time is meaningless — counting the final attempt of each. Pass `baseline=<path>` from `verify --timings` for a `vs ref` ratio column. |
| `merge_results.py` | Merges two results files into one: questions matched on text, per-DB run lists concatenated, questions present in only one file carried through. Takes base, incoming and output paths. |
| `format_questions.py` | Renders a *questions* file (not a results file) as Markdown for review, breaking the one-line reference queries across clause boundaries so the joins are actually checkable. Takes the questions path and an output path. |

```sh
analysis/accuracy_by_db.py results-reactome.json
analysis/accuracy_by_variation.py results-reactome.json
analysis/incorrect_queries.py results-reactome.json db=sql difficulty=hard
analysis/query_errors.py results-reactome.json db=neo4j
analysis/token_usage.py results-reactome.json
analysis/query_time.py results-reactome.json baseline=ref-timings.json
analysis/format_questions.py data/reactome/questions.json data/reactome/questions-review.md
analysis/merge_results.py results-old.json results-rerun.json results-merged.json
```

Notes (see `_common.py`):

- **Accuracy** is the share of runs whose generated query returned the expected
  result. `unanswerable` is its own difficulty tier (bucketed from the question's
  `unanswerable` flag); that column measures UNANSWERABLE-detection accuracy, a
  different skill from query generation.
- **Return shape counts.** A query that computes the right answer but hands back
  the wrong shape — an extra column, say — is a miss. Producing the asked-for
  shape is part of using a language, and a language that makes it awkward should
  score worse for it: Cypher won't `ORDER BY` an aggregate that isn't projected,
  so argmax answers come back as two columns unless the model adds a further
  clause. Use `incorrect_queries.py` to see which failures are of this kind.
- **Retry levels** are derived: the runner executes each run once at the highest
  configured retry level and truncates the attempt trace for the lower ones. So
  summing/averaging across levels double-counts — `accuracy_by_db` and
  `token_usage`/`query_time` count at the highest level only; `accuracy_by_variation` groups
  by it; `incorrect_queries` reports the highest-level (final) outcome.
