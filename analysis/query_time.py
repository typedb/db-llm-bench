#!/usr/bin/env python3
"""How long the generated queries took to execute, by model and DB.

Accuracy says whether a model can answer the question; this says what its
answer costs to run. A model that reaches the right number through a query the
store takes 40s to execute has not written the same quality of answer as one
that gets there in 200ms.

Three things this deliberately does NOT do:

- **Inaccurate runs are excluded.** The execution time of a wrong query means
  nothing — a query that returns the wrong answer quickly is not fast, it is
  wrong. Timed-out queries are wrong for this purpose too, and including them
  would let a model look slow for questions it simply failed.
- **Only the final attempt counts.** A record's `dbLatencyMs` sums every
  attempt, so a model that failed twice before succeeding would be charged for
  queries that were discarded. What we want is the cost of the query that
  actually answered.
- **Only the highest retry level is read.** Lower levels are derived by
  truncating the same trace, so summing across levels recounts the same
  executions.

Compare the median, not the mean: one pathological query against a 120s cap
drags a mean far more than it reflects typical behaviour.

Pass a baseline written by `verify --timings <path>` to get a `vs ref` column:
the median of each run's time divided by the reference query's time for the
same question and DB. That ratio is the useful number — absolute milliseconds
say more about the machine than about the model, whereas 3.2x says the model's
query costs three times what a well-written one does.

Usage: analysis/query_time.py [results.json] [baseline=timings.json]
"""
import json
import os
import statistics
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import _common as C


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("baseline=")]
    baseline_arg = next((a for a in sys.argv[1:] if a.startswith("baseline=")), None)
    path = args[0] if args else "results-reactome.json"
    baseline = json.load(open(baseline_arg.split("=", 1)[1])) if baseline_arg else {}
    records = C.load_records(path)
    if not records:
        sys.exit(f"no records in {path}")

    max_retry = max(r["maxRetries"] for r in records)
    # An unanswerable question's correct response runs no query at all, so it
    # has no execution time to report.
    runs = [
        r
        for r in records
        if r["maxRetries"] == max_retry
        and r["accurate"]
        and r["difficulty"] != "unanswerable"
        and r["attemptDbLatencyMs"]
    ]
    if not runs:
        sys.exit(f"no accurate runs with query timings in {path}")

    groups = {}
    ratios = {}
    for r in runs:
        # The last attempt is the one that succeeded.
        took = r["attemptDbLatencyMs"][-1]
        groups.setdefault((r["model"], r["db"]), []).append(took)
        ref = baseline.get(r["db"], {}).get(r["question"])
        # A reference that ran in under a millisecond makes the ratio
        # meaningless, so those questions sit out rather than dividing by zero.
        if ref:
            ratios.setdefault((r["model"], r["db"]), []).append(took / ref)

    headers = ["model", "db", "runs", "median", "mean", "p90", "slowest"]
    if baseline:
        headers.append("vs ref")
    rows = []
    for key in sorted(groups):
        times = sorted(groups[key])
        p90 = times[min(len(times) - 1, int(round(0.9 * (len(times) - 1))))]
        row = [
            key[0],
            key[1],
            str(len(times)),
            ms(statistics.median(times)),
            ms(statistics.mean(times)),
            ms(p90),
            ms(times[-1]),
        ]
        if baseline:
            got = ratios.get(key)
            row.append(f"{statistics.median(got):.1f}x" if got else "-")
        rows.append(row)
    print(C.render_table(headers, rows, label_cols=2))

    skipped = sum(
        1
        for r in records
        if r["maxRetries"] == max_retry
        and not r["accurate"]
        and r["difficulty"] != "unanswerable"
    )
    print(f"\n{len(runs)} accurate runs timed; {skipped} inaccurate runs excluded.")


def ms(value):
    """Milliseconds under a second, seconds above — query times here span
    three orders of magnitude, and raw ms is unreadable at the top end."""
    return f"{value:.0f}ms" if value < 1000 else f"{value / 1000:.1f}s"


if __name__ == "__main__":
    main()
