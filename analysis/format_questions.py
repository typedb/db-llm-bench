#!/usr/bin/env python3
"""Render a questions JSON as readable Markdown for review.

Reference queries are stored as single-line strings so the JSON stays diffable,
which makes them near-unreadable. This breaks them across lines at clause
boundaries and indents by parenthesis depth, so a reviewer can actually check
the joins.

Usage: format_questions.py [questions.json] [out.md]
"""

import json
import pathlib
import re
import sys

# Clause keywords that start a new line. Longest-first so "GROUP BY" wins over
# "GROUP", and "LEFT JOIN" over "JOIN".
BREAK = [
    "WITH RECURSIVE", "UNION ALL", "GROUP BY", "ORDER BY", "LEFT JOIN",
    "INNER JOIN", "CROSS JOIN", "UNION", "SELECT", "FROM", "WHERE", "HAVING",
    "LIMIT", "JOIN", "AND", "OR",
]
BREAK_RE = re.compile(r"(?<![A-Za-z0-9_])(" + "|".join(BREAK) + r")(?![A-Za-z0-9_])")
INDENT = "    "

# Cypher's clause keywords. Conventionally uppercase in the reference queries,
# but matched case-insensitively since the language does not require it.
CYPHER_BREAK = [
    "OPTIONAL MATCH", "DETACH DELETE", "ORDER BY", "UNION ALL", "UNWIND",
    "RETURN", "UNION", "MATCH", "MERGE", "CREATE", "DELETE", "WHERE", "WITH",
    "CALL", "SKIP", "LIMIT", "SET", "AND", "OR",
]
CYPHER_RE = re.compile(
    r"(?<![A-Za-z0-9_])(" + "|".join(CYPHER_BREAK) + r")(?![A-Za-z0-9_])", re.I
)

# TypeQL keywords. Two groups: those that introduce a block whose statements
# belong on following lines, and pipeline operators that read better with their
# own argument on the same line.
TYPEQL_BLOCK = [
    "with", "match", "insert", "delete", "update", "put", "define", "redefine",
    "undefine", "fetch",
]
TYPEQL_INLINE = [
    "select", "reduce", "sort", "offset", "limit", "require", "distinct",
    "return", "fun",
]
# `$`-prefixed lookbehind as well as word characters: a variable named
# `$matches` must not be mistaken for the `match` keyword.
TYPEQL_RE = re.compile(
    r"(?<![A-Za-z0-9_$-])("
    + "|".join(sorted(TYPEQL_BLOCK + TYPEQL_INLINE, key=len, reverse=True))
    + r")(?![A-Za-z0-9_-])"
)


# ", <name> AS (" starts the next CTE and deserves its own line.
NEXT_CTE_RE = re.compile(r"\s*,\s*([A-Za-z_][A-Za-z0-9_]*)\s+AS\s*\(", re.I)


def format_sql(sql: str) -> str:
    """Break a one-line query at clause boundaries, indenting by paren depth.

    Parentheses are never broken on: doing so splits `COUNT(*)` across lines.
    They only move the indent level, so subquery bodies sit one step in while
    function arguments stay put.
    """
    out, line, depth, i = [], "", 0, 0
    # Indent is decided by the depth the line STARTED at, so a line that closes
    # a CTE stays aligned with the rest of that CTE's body instead of dedenting.
    line_depth = 0

    def flush() -> None:
        nonlocal line
        if line.strip():
            out.append(INDENT * line_depth + " ".join(line.split()))
        line = ""

    while i < len(sql):
        # A new CTE: close off the previous one and start fresh at depth 0.
        m = NEXT_CTE_RE.match(sql, i) if depth == 0 else None
        if m:
            line += ","
            flush()
            line_depth = 0
            line = f"{m.group(1)} AS ("
            depth = 1
            i = m.end()
            continue
        ch = sql[i]
        if ch == "(":
            depth += 1
            line += ch
            i += 1
            continue
        if ch == ")":
            depth = max(0, depth - 1)
            line += ch
            i += 1
            continue
        m = BREAK_RE.match(sql, i)
        if m:
            flush()
            line_depth = depth
            line = m.group(1)
            i = m.end()
            continue
        line += ch
        i += 1
    flush()
    return "\n".join(out)


def indent_for(line: str, level: int) -> str:
    """Indent a collapsed line, dedenting one step per leading closing bracket.

    A line's indent is decided by the depth it started at, which is right for
    the common case but leaves `};` closing a block indented as if it were
    still inside it. Backing off one step per leading closer lines the brace up
    with whatever opened it.
    """
    collapsed = " ".join(line.split())
    closers = len(collapsed) - len(collapsed.lstrip("})"))
    return INDENT * max(0, level - closers) + collapsed


def format_cypher(query: str) -> str:
    """Break a Cypher query at clause boundaries, indenting by bracket depth.

    Node and relationship patterns — `(c:Complex)`, `{displayName:'x'}` — open
    and close within a line, and indent is decided by the depth a line STARTED
    at, so they cost nothing. Only brackets left open across a break indent,
    which is what `EXISTS { ... }` subqueries want.
    """
    out, line, depth, i = [], "", 0, 0
    line_depth = 0

    def flush() -> None:
        nonlocal line
        if line.strip():
            out.append(indent_for(line, line_depth))
        line = ""

    while i < len(query):
        ch = query[i]
        if ch in "({":
            depth += 1
            line += ch
            i += 1
            continue
        if ch in ")}":
            depth = max(0, depth - 1)
            line += ch
            i += 1
            continue
        m = CYPHER_RE.match(query, i)
        if m:
            flush()
            line_depth = depth
            line = m.group(1)
            i = m.end()
            continue
        line += ch
        i += 1
    flush()
    return "\n".join(out)


def format_typeql(query: str) -> str:
    """Break a TypeQL query after each `;` and before each clause keyword.

    Indentation combines three sources: brace depth for `{ ... }` blocks, one
    step for statements belonging to a block clause (the patterns under a
    `match`), and one step for the body of a `with fun ...` preamble. The
    function body ends at its `return`, which is what closes that last step.

    Inline braces — a `-> { event, integer }` signature, a `return { $e, $d }`
    — open and close within one line, and indent is decided by the depth a line
    started at, so they cost nothing.
    """
    out, line, depth, i = [], "", 0, 0
    line_depth = 0
    body = 0  # statements under a block clause
    base = 0  # inside a `fun` body
    pending_base = None

    def flush() -> None:
        nonlocal line, base, pending_base
        if line.strip():
            out.append(indent_for(line, line_depth + base + body))
        line = ""
        if pending_base is not None:
            base, pending_base = pending_base, None

    while i < len(query):
        ch = query[i]
        if ch == "{":
            depth += 1
            line += ch
            i += 1
            continue
        if ch == "}":
            depth = max(0, depth - 1)
            line += ch
            i += 1
            continue
        if ch == ";":
            line += ch
            ends_fun = line.lstrip().startswith("return")
            flush()
            # A function's `return` is its last statement; what follows belongs
            # to the enclosing query again.
            if ends_fun:
                base, body = 0, 0
            line_depth = depth
            i += 1
            continue
        m = TYPEQL_RE.match(query, i)
        if m:
            keyword = m.group(1)
            flush()
            line_depth = depth
            if keyword in TYPEQL_BLOCK:
                body = 0
                line = keyword
                flush()  # the keyword sits alone; its statements indent under it
                body = 1
            else:
                body = 0
                line = keyword
                if keyword == "fun":
                    # The signature stays on this line; the body indents.
                    pending_base = 1
            i = m.end()
            continue
        line += ch
        i += 1
    flush()
    return "\n".join(out)


# Reference queries are stored one-per-line as JSON strings; each language
# needs its own line-breaking rules, and its own fence so the review renders
# with the right highlighting.
FORMATTERS = {
    "sql": format_sql,
    "cypher": format_cypher,
    "typeql": format_typeql,
}


def main() -> None:
    src = pathlib.Path(sys.argv[1] if len(sys.argv) > 1 else "data/reactome/questions.json")
    dst = pathlib.Path(sys.argv[2] if len(sys.argv) > 2 else src.with_suffix(".md"))
    questions = json.loads(src.read_text(encoding="utf8"))["questions"]

    lines = [f"# {src.name} — {len(questions)} questions", ""]
    lines += ["Generated by `analysis/format_questions.py`; edit the JSON, not this file.", ""]
    for n, q in enumerate(questions, 1):
        lines.append(f"## {n}. {q['difficulty']}")
        lines.append("")
        lines.append(q["question"])
        lines.append("")
        if q.get("unanswerable"):
            lines += ["**Unanswerable** — the only correct response is the UNANSWERABLE token.", ""]
            continue
        shape = "ordered list" if q.get("ordered") else type(q["expected"]).__name__
        by_db = q.get("expected_by_db") or {}
        # A per-store override means the reference queries no longer agree on
        # one number, so the reviewer needs to see which store expects what and
        # why — that disagreement is exactly what normally signals a bad query.
        label = "**Expected** — baseline" if by_db else "**Expected**"
        lines.append(f"{label} ({shape}):")
        lines.append("")
        lines.append("```json")
        lines.append(json.dumps(q["expected"], indent=2, ensure_ascii=False))
        lines.append("```")
        lines.append("")
        if by_db:
            lines.append("**Stores that disagree:**")
            lines.append("")
            for db, value in sorted(by_db.items()):
                lines.append(f"- `{db}`: {json.dumps(value, ensure_ascii=False)}")
            lines.append("")
            lines.append(f"> {q.get('divergence', '(no divergence note recorded)')}")
            lines.append("")
        for lang, query in sorted(q.get("queries", {}).items()):
            lines.append(f"**{lang}**")
            lines.append("")
            lines.append(f"```{lang}")
            lines.append(FORMATTERS.get(lang, format_sql)(query))
            lines.append("```")
            lines.append("")
    dst.write_text("\n".join(lines) + "\n", encoding="utf8")
    print(f"wrote {dst} ({len(questions)} questions)")


if __name__ == "__main__":
    main()
