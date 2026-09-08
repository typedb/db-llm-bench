#!/usr/bin/env python3
"""How the failing runs fail: with a visible error, or silently wrong.

A run that fails can end in two ways: the query errors (syntax error, timeout,
wrong shape — anything the harness can see and feed back for a retry), or it
executes cleanly and returns a wrong answer, which nothing downstream can tell
from a right one. This script reports that split per DB, twice:

- at retry level 0 (the first attempt), classifying only the failing runs —
  the raw failure profile of each language; and
- at the highest retry level (the final outcome), over ALL runs — correct vs
  errored vs silently wrong after the retry loop has done what it can.

A third table breaks the first attempt down by DB x skills x examples, over all
runs, to show whether in-context resources reduce the error rate itself (fewer
invalid queries) or merely shift failures between loud and silent.

Only answerable questions count: an unanswerable question has no wrong-answer
failure mode. All tables pool models and variations; pass model=<substring>,
skills=on|off and/or examples=<n> to narrow them.

Usage: analysis/failure_modes.py [results.json] [model=<substring>] [skills=on|off] [examples=<n>]
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import _common as C


def pct(n, total):
    return f"{100 * n / total:.0f}% ({n}/{total})" if total else "-"


def main():
    args = sys.argv[1:]
    filters = {}
    paths = []
    for a in args:
        if "=" in a:
            k, v = a.split("=", 1)
            if k not in ("model", "skills", "examples"):
                sys.exit(f"unknown filter {k!r}; expected model=, skills= or examples=")
            filters[k] = v
        else:
            paths.append(a)
    path = paths[0] if paths else "results-reactome.json"

    records = [r for r in C.load_records(path) if not r["unanswerable"]]
    if "model" in filters:
        records = [r for r in records if filters["model"] in r["model"]]
    if "skills" in filters:
        want = filters["skills"].lower() == "on"
        records = [r for r in records if bool(r["skills"]) == want]
    if "examples" in filters:
        records = [r for r in records if str(r["examples"]) == filters["examples"]]
    if not records:
        sys.exit(f"no records in {path} match the filters")
    dbs = sorted({r["db"] for r in records})
    max_retry = max(r["maxRetries"] for r in records)

    desc = ["answerable questions"]
    desc.append(f"model *{filters['model']}*" if "model" in filters else "models pooled")
    desc.append(f"skills {filters['skills']}" if "skills" in filters else "skills pooled")
    desc.append(f"{filters['examples']} examples" if "examples" in filters else "example counts pooled")
    print(f"{path}  ({'; '.join(desc)})\n")

    print("First attempt (retry level 0): how the failing runs fail")
    rows = []
    for db in dbs:
        fails = [r for r in records
                 if r["db"] == db and r["maxRetries"] == 0 and not r["accurate"]]
        errored = sum(1 for r in fails if r["error"])
        rows.append([db, len(fails), pct(errored, len(fails)),
                     pct(len(fails) - errored, len(fails))])
    print(C.render_table(["DB", "failures", "visible error", "silently wrong"], rows))

    print(f"\nFinal outcome (retry level {max_retry}): all runs")
    rows = []
    for db in dbs:
        runs = [r for r in records if r["db"] == db and r["maxRetries"] == max_retry]
        correct = sum(1 for r in runs if r["accurate"])
        errored = sum(1 for r in runs if not r["accurate"] and r["error"])
        silent = len(runs) - correct - errored
        fails = errored + silent
        rows.append([db, len(runs), pct(correct, len(runs)), pct(errored, len(runs)),
                     pct(silent, len(runs)), pct(errored, fails)])
    print(C.render_table(
        ["DB", "runs", "correct", "error", "silently wrong", "error share of failures"], rows))

    print("\nFirst attempt by variation: all runs")
    rows = []
    for db in dbs:
        for skills in sorted({r["skills"] for r in records}):
            for examples in sorted({r["examples"] for r in records}):
                runs = [r for r in records
                        if r["db"] == db and r["maxRetries"] == 0
                        and r["skills"] == skills and r["examples"] == examples]
                correct = sum(1 for r in runs if r["accurate"])
                errored = sum(1 for r in runs if not r["accurate"] and r["error"])
                silent = len(runs) - correct - errored
                rows.append([db, "on" if skills else "off", examples, len(runs),
                             pct(correct, len(runs)), pct(errored, len(runs)),
                             pct(silent, len(runs))])
    print(C.render_table(
        ["DB", "skills", "examples", "runs", "correct", "error", "silently wrong"], rows,
        label_cols=3))


if __name__ == "__main__":
    main()
