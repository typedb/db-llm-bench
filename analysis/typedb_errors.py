#!/usr/bin/env python3
"""What kind of errors the TypeDB runs raise: syntax vs semantic.

Classifies the first-attempt error of every failing TypeDB run by the TypeDB
error code in its message (the first bracketed code, which is the outermost and
most specific in the cause chain), bucketed by code prefix:

- syntax        TQL   — the query does not parse
- type          INF, QUA, FIN, CEX — type inference rejected it (unknown type
                labels, non-attribute ownership, mismatched value types)
- semantic      REP, FER, FUN, FRP — well-formed but wrong query structure:
                variable scoping across pipeline stages, unknown functions,
                invalid recursion
- runtime       EEV, REX, PEX, QPL, MCP, ECP — the query compiled but failed
                during planning or execution (e.g. division by zero)

Attempts with no TypeDB code are harness-level outcomes, reported alongside:
`timeout`, `no query` (none extracted from the response — in practice the model
hit its output-token limit before emitting a fenced query), `unanswerable`
(declared UNANSWERABLE on an answerable question), and `shape`/`other`.

Reported overall (with per-code counts), then broken down by skills x examples
so the effect of in-context resources on each error kind is visible — rates are
shares of ALL first attempts, keeping denominators comparable across configs.
Answerable questions only; pass model=<substring> to filter.

Usage: analysis/typedb_errors.py [results.json] [model=<substring>]
"""
import os
import re
import sys
from collections import Counter

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import _common as C

PREFIXES = {
    "TQL": "syntax",
    "INF": "type", "QUA": "type", "FIN": "type", "CEX": "type",
    "REP": "semantic", "FER": "semantic", "FUN": "semantic", "FRP": "semantic",
    "EEV": "runtime", "REX": "runtime", "PEX": "runtime",
    "QPL": "runtime", "MCP": "runtime", "ECP": "runtime",
}
CATEGORIES = ["syntax", "type", "semantic", "runtime",
              "timeout", "no query", "unanswerable", "shape", "other"]


def classify(error):
    """(category, code) for one attempt's error string."""
    m = re.search(r"\[([A-Z]{2,4})\d+\]", error)
    if m:
        return PREFIXES.get(m.group(1), "other"), re.search(r"\[([A-Z]{2,4}\d+)\]", error).group(1)
    if error.startswith("query timed out"):
        return "timeout", None
    if error.startswith("no query found"):
        return "no query", None
    if error.startswith("declared UNANSWERABLE"):
        return "unanswerable", None
    if error.startswith("result had the wrong shape"):
        return "shape", None
    return "other", None


def main():
    args = sys.argv[1:]
    model_filter = None
    paths = []
    for a in args:
        if a.startswith("model="):
            model_filter = a.split("=", 1)[1]
        else:
            paths.append(a)
    path = paths[0] if paths else "results-reactome.json"

    records = [r for r in C.load_records(path)
               if r["db"] == "typedb" and not r["unanswerable"] and r["maxRetries"] == 0]
    if model_filter:
        records = [r for r in records if model_filter in r["model"]]
    if not records:
        sys.exit(f"no typedb records in {path}")

    classified = []  # (record, category, code)
    for r in records:
        if r["accurate"] or not r["error"]:
            continue
        cat, code = classify(r["error"])
        classified.append((r, cat, code))

    print(f"{path}  (TypeDB first attempts, answerable questions; models pooled"
          + (f", filtered to *{model_filter}*" if model_filter else "") + ")\n")

    cats = [c for c in CATEGORIES if any(cat == c for _, cat, _ in classified)]

    print(f"Errored first attempts by kind ({len(classified)} of {len(records)} runs)")
    rows = []
    for c in cats:
        n = sum(1 for _, cat, _ in classified if cat == c)
        codes = Counter(code for _, cat, code in classified if cat == c and code)
        detail = "  ".join(f"{k}:{v}" for k, v in codes.most_common())
        rows.append([c, f"{n} ({100 * n / len(classified):.0f}%)", detail])
    print(C.render_table(["kind", "attempts", "codes"], rows, label_cols=1))

    print("\nBy variation: share of all first attempts")
    by_var = {}
    for r, cat, _ in classified:
        by_var.setdefault((r["skills"], r["examples"]), Counter())[cat] += 1
    rows = []
    for skills in (False, True):
        for examples in sorted({r["examples"] for r in records}):
            runs = sum(1 for r in records
                       if r["skills"] == skills and r["examples"] == examples)
            counts = by_var.get((skills, examples), Counter())
            rows.append(["on" if skills else "off", examples, runs,
                         *[f"{100 * counts[c] / runs:.0f}% ({counts[c]})" for c in cats]])
    print(C.render_table(["skills", "examples", "runs", *cats], rows, label_cols=2))


if __name__ == "__main__":
    main()
