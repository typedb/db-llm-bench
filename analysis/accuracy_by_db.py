#!/usr/bin/env python3
"""Accuracy by database and difficulty, collapsing every run variation.

Uses the highest configured retry level (the final outcome after all retries)
and averages over example counts, skills, and repetitions, so each DB gets one
row of accuracy per difficulty tier (easy / medium / hard / expert / unanswerable) plus
overall. The `unanswerable` column is UNANSWERABLE-detection accuracy, not query
accuracy. For the per-variation breakdown, use accuracy_by_variation.py.

Usage: analysis/accuracy_by_db.py [results.json]
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

    # Collapse the retry dimension to its highest level to avoid double-counting
    # the derived lower-level records.
    max_retry = max(r["maxRetries"] for r in records)
    records = [r for r in records if r["maxRetries"] == max_retry]
    models = sorted({r["model"] for r in records})
    print(f"{path}  (retry budget {max_retry}; averaged over examples, skills, repetitions)\n")

    diffs = C.diff_order(records)
    for model in models:
        if len(models) > 1:
            print(f"model: {model}")
        mr = [r for r in records if r["model"] == model]
        headers = ["DB", *diffs, "overall"]
        rows = []
        for db in sorted({r["db"] for r in mr}):
            dbr = [r for r in mr if r["db"] == db]
            rows.append([db, *[C.cell([r for r in dbr if r["difficulty"] == d]) for d in diffs], C.cell(dbr)])
        print(C.render_table(headers, rows))
        print()


if __name__ == "__main__":
    main()
