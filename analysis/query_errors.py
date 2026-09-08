#!/usr/bin/env python3
"""What kind of errors the failing runs raise, per DB: syntax vs semantic.

Classifies the first-attempt error of every failing run into the same four
buckets for each DB, so the failure profiles of the three languages can be set
side by side:

- syntax     the query does not parse
- type       static checking rejected it: unknown type labels, columns,
             functions, variables or parameters, or mismatched value types
- semantic   well-formed but wrong query structure: variable scoping across
             pipeline stages, aggregation grouping, duplicate result columns,
             invalid recursion, invalid collation
- runtime    the query compiled but failed while running: division by zero,
             list-to-string coercion, the transaction memory cap

How each bucket is recognised differs per DB:

- typedb   by the TypeDB error code in the message (the first bracketed code,
           the outermost and most specific in the cause chain): TQL is syntax;
           INF, QUA, FIN, CEX are type; REP, FER, FUN, FRP are semantic; EEV,
           REX, PEX, QPL, MCP, ECP are runtime.
- neo4j    by the Neo.* status code, except that Neo4j's SyntaxError code is a
           catch-all also covering unknown functions, undefined variables and
           grouping mistakes, so it is split further on the message. Neo4j
           has a fifth bucket of its own, `version preamble`: the vendored
           Cypher skill tells the model to open every query with `CYPHER 25`,
           which the benchmark's Neo4j 5.26 rejects. Runs made before the
           prompt named the server version carry many of these; they are a
           configuration artefact, not a language one. Note that Neo4j does
           not reject an unknown label or property at all (the match is
           silently empty), so those never appear here.
- sql      MySQL puts no code in its message, so by message pattern.

Attempts with no DB error are harness-level outcomes, reported alongside:
`timeout`, `no query` (none extracted from the response — in practice the model
hit its output-token limit before emitting a fenced query), `unanswerable`
(declared UNANSWERABLE on an answerable question), and `shape`/`other`.

Reported overall (with per-code or per-message counts), then broken down by
skills x examples so the effect of in-context resources on each error kind is
visible — rates are shares of ALL first attempts, keeping denominators
comparable across configs. Answerable questions only.

Usage: analysis/query_errors.py [results.json] [db=typedb|neo4j|sql] [model=<substring>]
"""
import os
import re
import sys
from collections import Counter

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import _common as C

CATEGORIES = ["syntax", "type", "semantic", "runtime", "version preamble",
              "timeout", "no query", "unanswerable", "shape", "other"]

TYPEDB_PREFIXES = {
    "TQL": "syntax",
    "INF": "type", "QUA": "type", "FIN": "type", "CEX": "type",
    "REP": "semantic", "FER": "semantic", "FUN": "semantic", "FRP": "semantic",
    "EEV": "runtime", "REX": "runtime", "PEX": "runtime",
    "QPL": "runtime", "MCP": "runtime", "ECP": "runtime",
}

# Neo4j's SyntaxError code, split by message: anything not listed is a real
# parse error ("Invalid input ...").
NEO4J_SYNTAX_SPLIT = [
    (r"Unknown function", "type", "unknown function"),
    (r"Variable `\w+` not defined", "type", "undefined variable"),
    (r"Type mismatch", "type", "type mismatch"),
    (r"Aggregation column contains implicit grouping", "semantic", "implicit grouping"),
    (r"Multiple result columns with the same name", "semantic", "duplicate columns"),
    (r"Query cannot conclude with", "semantic", "bad final clause"),
    (r"Dynamic Label and Types are only allowed", "semantic", "dynamic label"),
]

MYSQL_PATTERNS = [
    (r"You have an error in your SQL syntax", "syntax", "parse error"),
    (r"Unknown column", "type", "unknown column"),
    (r"Unknown table|doesn't exist", "type", "unknown table"),
    (r"FUNCTION .* does not exist|Unknown function", "type", "unknown function"),
    (r"Illegal mix of collations|COLLATION .* is not valid", "semantic", "collation"),
    (r"Recursive|recursive", "semantic", "recursive CTE"),
    (r"Data too long|Out of range|Division by 0|Incorrect .* value|Truncated", "runtime", "value error"),
]


def classify_harness(error):
    if error.startswith("query timed out"):
        return "timeout", None
    if error.startswith("no query found"):
        return "no query", None
    if error.startswith("declared UNANSWERABLE"):
        return "unanswerable", None
    if error.startswith("result had the wrong shape"):
        return "shape", None
    return None


def classify_typedb(error):
    m = re.search(r"\[([A-Z]{2,4})(\d+)\]", error)
    if m:
        return TYPEDB_PREFIXES.get(m.group(1), "other"), m.group(1) + m.group(2)
    return classify_harness(error) or ("other", error[:40])


def classify_neo4j(error):
    m = re.search(r"Neo\.[A-Za-z]+\.[A-Za-z]+\.[A-Za-z]+", error)
    if not m:
        return classify_harness(error) or ("other", error[:40])
    code = m.group(0)
    short = code.rsplit(".", 1)[-1]
    if short == "SyntaxError":
        for pattern, cat, label in NEO4J_SYNTAX_SPLIT:
            if re.search(pattern, error):
                return cat, label
        return "syntax", "parse error"
    if short == "ArgumentError":
        if "cypher version" in error:
            return "version preamble", "CYPHER 25"
        return "semantic", short
    if short == "ParameterMissing":
        return "type", "missing parameter"
    if short == "TypeError":
        return "runtime", short
    if ".TransientError." in code or ".DatabaseError." in code:
        return "runtime", short
    return "other", code


def classify_sql(error):
    harness = classify_harness(error)
    if harness:
        return harness
    message = re.sub(r"^syntax error: ", "", error)
    for pattern, cat, label in MYSQL_PATTERNS:
        if re.search(pattern, message):
            return cat, label
    return "other", message[:40]


CLASSIFIERS = {"typedb": classify_typedb, "neo4j": classify_neo4j, "sql": classify_sql}


def main():
    args = sys.argv[1:]
    filters = {}
    paths = []
    for a in args:
        if "=" in a:
            k, v = a.split("=", 1)
            if k not in ("db", "model"):
                sys.exit(f"unknown filter {k!r}; expected db= or model=")
            filters[k] = v
        else:
            paths.append(a)
    path = paths[0] if paths else "results-reactome.json"
    db = filters.get("db", "typedb")
    if db not in CLASSIFIERS:
        sys.exit(f"db must be one of {', '.join(CLASSIFIERS)}")
    classify = CLASSIFIERS[db]

    records = [r for r in C.load_records(path)
               if r["db"] == db and not r["unanswerable"] and r["maxRetries"] == 0]
    if "model" in filters:
        records = [r for r in records if filters["model"] in r["model"]]
    if not records:
        sys.exit(f"no {db} records in {path}")

    classified = []  # (record, category, detail)
    for r in records:
        if r["accurate"] or not r["error"]:
            continue
        cat, detail = classify(r["error"])
        classified.append((r, cat, detail))

    print(f"{path}  ({db} first attempts, answerable questions; models pooled"
          + (f", filtered to *{filters['model']}*" if "model" in filters else "") + ")\n")

    cats = [c for c in CATEGORIES if any(cat == c for _, cat, _ in classified)]

    print(f"Errored first attempts by kind ({len(classified)} of {len(records)} runs)")
    rows = []
    for c in cats:
        n = sum(1 for _, cat, _ in classified if cat == c)
        details = Counter(d for _, cat, d in classified if cat == c and d)
        detail = "  ".join(f"{k}:{v}" for k, v in details.most_common())
        rows.append([c, f"{n} ({100 * n / len(classified):.0f}%)", detail])
    print(C.render_table(["kind", "attempts", "detail"], rows, label_cols=1))

    print("\nBy variation: share of all first attempts")
    by_var = {}
    for r, cat, _ in classified:
        by_var.setdefault((r["skills"], r["examples"]), Counter())[cat] += 1
    rows = []
    for skills in sorted({r["skills"] for r in records}):
        for examples in sorted({r["examples"] for r in records}):
            runs = sum(1 for r in records
                       if r["skills"] == skills and r["examples"] == examples)
            if not runs:
                continue
            counts = by_var.get((skills, examples), Counter())
            rows.append(["on" if skills else "off", examples, runs,
                         *[f"{100 * counts[c] / runs:.0f}% ({counts[c]})" for c in cats]])
    print(C.render_table(["skills", "examples", "runs", *cats], rows, label_cols=2))


if __name__ == "__main__":
    main()
