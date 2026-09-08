#!/usr/bin/env python3
"""Accuracy by database and every run variation, plus difficulty.

One row per (model, db, skills, examples, retries) combination present in the
file, so you can see how skill injection, few-shot example count, and retry
budget each move accuracy. The model column is shown only when more than one
model is present.

Retry levels are cumulative-budget views derived from a single execution
(see _common.py), so grouping by them here is valid — each row is "accuracy
within that retry budget". The `unanswerable` difficulty column reports
UNANSWERABLE-detection accuracy, not query accuracy.

Usage: analysis/accuracy_by_variation.py [results.json]
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

    multi_model = len({r["model"] for r in records}) > 1
    diffs = C.diff_order(records)
    key_cols = (["model"] if multi_model else []) + ["db", "skills", "examples", "retries"]
    headers = [*key_cols, *diffs, "overall"]

    groups = {}
    for r in records:
        key = (
            (r["model"],) if multi_model else ()
        ) + (r["db"], "on" if r["skills"] else "off", r["examples"], r["maxRetries"])
        groups.setdefault(key, []).append(r)

    # Sort by db, then examples, retries, skills (and model first if shown).
    def sort_key(key):
        vals = list(key)
        model = vals.pop(0) if multi_model else ""
        db, skills, examples, retries = vals
        return (model, db, examples, retries, skills)

    rows = []
    for key in sorted(groups, key=sort_key):
        g = groups[key]
        cells = [*key[:-3], key[-3], str(key[-2]), str(key[-1])]  # key cols as strings
        cells += [C.cell([r for r in g if r["difficulty"] == d]) for d in diffs]
        cells.append(C.cell(g))
        rows.append(cells)

    print(f"{path}  (one row per {' x '.join(key_cols)})\n")
    print(C.render_table(headers, rows, label_cols=len(key_cols)))


if __name__ == "__main__":
    main()
