#!/usr/bin/env python3
"""Total model tokens used across a results file, broken down by model and DB.

Only the highest retry level is counted. The runner executes each run once at
that level and derives the lower levels by truncating the attempt trace, so a
record's `tokens` at a lower level is the same calls recounted — summing every
record would multi-count (e.g. a `[0, 2, 4]` sweep triples the first attempt).
Counting at the top level alone gives each actual model call exactly once.

Usage: analysis/token_usage.py [results.json]
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import _common as C


def main():
    path = sys.argv[1] if len(sys.argv) > 1 else "results-reactome.json"
    records = C.load_records(path)
    if not records:
        sys.exit(f"no records in {path}")

    max_retry = max(r["maxRetries"] for r in records)
    runs = [r for r in records if r["maxRetries"] == max_retry]

    groups = {}
    for r in runs:
        groups.setdefault((r["model"], r["db"]), []).append(r)

    def n(x):
        return f"{x:,}"

    headers = ["model", "db", "runs", "calls", "input", "output", "total"]
    rows = []
    tot_in = tot_out = tot_runs = tot_calls = 0
    for key in sorted(groups):
        g = groups[key]
        gi = sum(r["tokens"]["input"] for r in g)
        go = sum(r["tokens"]["output"] for r in g)
        gc = sum(len(r["attempts"]) for r in g)
        rows.append([key[0], key[1], n(len(g)), n(gc), n(gi), n(go), n(gi + go)])
        tot_in += gi
        tot_out += go
        tot_runs += len(g)
        tot_calls += gc
    rows.append(["TOTAL", "", n(tot_runs), n(tot_calls), n(tot_in), n(tot_out), n(tot_in + tot_out)])

    print(f"{path}  (retry budget {max_retry}; each model call counted once)\n")
    print(C.render_table(headers, rows, label_cols=2))
    print(f"\nTotal tokens used: {n(tot_in + tot_out)}  ({n(tot_in)} in + {n(tot_out)} out)")


if __name__ == "__main__":
    main()
