#!/usr/bin/env python3
"""Total model tokens used across a results file, broken down by model and DB.

Only one retry level is counted, the highest by default. The runner executes
each run once at that level and derives the lower levels by truncating the
attempt trace, so a record's `tokens` at a lower level is the same calls
recounted — summing every record would multi-count (e.g. a `[0, 2, 4]` sweep
triples the first attempt). Counting at one level gives each model call at
most once; `retries=0` counts only first attempts.

`by=variation` splits each model x DB row further by skills x examples, with
mean output tokens per run, so the cost of each in-context resource level is
visible. `skills=on|off`, `examples=<n>` and `model=<substring>` narrow the
records.

Usage: analysis/token_usage.py [results.json] [retries=<n>] [by=variation]
                               [skills=on|off] [examples=<n>] [model=<substring>]
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import _common as C


def main():
    filters = {}
    paths = []
    for a in sys.argv[1:]:
        if "=" in a:
            k, v = a.split("=", 1)
            if k not in ("retries", "by", "skills", "examples", "model"):
                sys.exit(f"unknown option {k!r}")
            filters[k] = v
        else:
            paths.append(a)
    path = paths[0] if paths else "results-reactome.json"
    records = C.load_records(path)
    if not records:
        sys.exit(f"no records in {path}")

    retry = int(filters["retries"]) if "retries" in filters else max(r["maxRetries"] for r in records)
    runs = [r for r in records if r["maxRetries"] == retry]
    if "model" in filters:
        runs = [r for r in runs if filters["model"] in r["model"]]
    if "skills" in filters:
        want = filters["skills"].lower() == "on"
        runs = [r for r in runs if bool(r["skills"]) == want]
    if "examples" in filters:
        runs = [r for r in runs if str(r["examples"]) == filters["examples"]]
    if not runs:
        sys.exit(f"no records in {path} match")
    by_variation = filters.get("by") == "variation"

    groups = {}
    for r in runs:
        key = (r["model"], r["db"])
        if by_variation:
            key += ("on" if r["skills"] else "off", r["examples"])
        groups.setdefault(key, []).append(r)

    def n(x):
        return f"{x:,}"

    headers = ["model", "db"] + (["skills", "examples"] if by_variation else []) \
        + ["runs", "calls", "input", "output", "total", "output/run"]
    rows = []
    tot_in = tot_out = tot_runs = tot_calls = 0
    for key in sorted(groups):
        g = groups[key]
        gi = sum(r["tokens"]["input"] for r in g)
        go = sum(r["tokens"]["output"] for r in g)
        gc = sum(len(r["attempts"]) for r in g)
        rows.append([*key, n(len(g)), n(gc), n(gi), n(go), n(gi + go), n(round(go / len(g)))])
        tot_in += gi
        tot_out += go
        tot_runs += len(g)
        tot_calls += gc
    rows.append(["TOTAL", *([""] * (3 if by_variation else 1)), n(tot_runs), n(tot_calls),
                 n(tot_in), n(tot_out), n(tot_in + tot_out), n(round(tot_out / tot_runs))])

    desc = [f"retry budget {retry}"]
    for k in ("model", "skills", "examples"):
        if k in filters:
            desc.append(f"{k} {filters[k]}")
    print(f"{path}  ({'; '.join(desc)}; each model call counted once)\n")
    print(C.render_table(headers, rows, label_cols=4 if by_variation else 2))
    print(f"\nTotal tokens used: {n(tot_in + tot_out)}  ({n(tot_in)} in + {n(tot_out)} out)")


if __name__ == "__main__":
    main()
