#!/usr/bin/env python3
"""List every incorrectly generated query with the config that produced it.

For each run whose generated query did NOT match the expected answer (at the
highest retry level — the final outcome after all retries), prints the question,
the run's config (db, skills, example count, model), the expected ground-truth
query, and the expected vs actual answer. When the run used retries, every
attempt's query (and its error) is listed in order; otherwise just the single
generated query. Repetitions that produced the identical attempt trace are
collapsed into one entry.

Rendered as labeled blocks rather than a single-line table because the queries
are multi-line and would be unreadable truncated into columns.

Usage: analysis/incorrect_queries.py [results.json] [key=value ...]
  Optional filters: db=, difficulty=, model=, examples=, skills=on|off
  e.g. analysis/incorrect_queries.py results-reactome.json db=sql difficulty=hard
"""
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import _common as C



def first_line(text):
    """The first line of an error message, plus the next non-blank line when
    the first is only a bare prefix ("syntax error:") — TypeDB driver errors
    put a newline straight after the prefix, so splitlines()[0] alone would
    hide the actual message."""
    lines = [l.strip() for l in text.splitlines() if l.strip()]
    if not lines:
        return ""
    if lines[0].endswith(":") and len(lines) > 1:
        # TypeQL parse errors carry the useful part ("parsing error: expected
        # ...", "Near 9:0:") on the lines after the generic TQL03 headline.
        head = lines[1:4] if "[TQL03]" in lines[1] else lines[1:2]
        return lines[0] + " " + " | ".join(head)
    return lines[0]

def parse_filters(args):
    filters = {}
    for a in args:
        if "=" not in a:
            sys.exit(f"bad filter {a!r}; expected key=value")
        k, v = a.split("=", 1)
        filters[k] = v
    return filters


def keep(r, filters):
    for k, v in filters.items():
        if k == "skills":
            if bool(r["skills"]) != (v.lower() == "on"):
                return False
        elif str(r.get(k, "")).lower() != v.lower():
            return False
    return True


def config_str(r, multi_model):
    parts = ([r["model"]] if multi_model else []) + [
        r["db"],
        "skills on" if r["skills"] else "skills off",
        f"{r['examples']} examples",
    ]
    return " · ".join(parts)


def block(text, prefix="      "):
    if text is None or text == "":
        return prefix + "(none)"
    return "\n".join(prefix + line for line in str(text).splitlines())


def fmt_answer(v):
    """Compact repr of an answer — long lists/strings are truncated so a wrong
    query returning thousands of rows doesn't flood the output."""
    if isinstance(v, list) and len(v) > 8:
        head = ", ".join(repr(x) for x in v[:6])
        return f"[{head}, … ({len(v)} items)]"
    s = repr(v)
    return s if len(s) <= 200 else s[:200] + f"… ({len(s)} chars)"


def main():
    args = sys.argv[1:]
    path = args[0] if args and "=" not in args[0] else "results-reactome.json"
    filters = parse_filters([a for a in args if "=" in a])

    records = C.load_records(path)
    if not records:
        sys.exit(f"no records in {path}")
    max_retry = max(r["maxRetries"] for r in records)
    wrong = [r for r in records if r["maxRetries"] == max_retry and not r["accurate"] and keep(r, filters)]
    multi_model = len({r["model"] for r in records}) > 1

    # Collapse repetitions that produced the identical attempt trace.
    groups = {}
    for r in wrong:
        trace = tuple(a["query"] for a in r["attempts"])
        key = (r["model"], r["db"], r["skills"], r["examples"], r["question"], trace, str(r["actual"]))
        groups.setdefault(key, []).append(r)

    filt = (" matching " + ", ".join(f"{k}={v}" for k, v in filters.items())) if filters else ""
    print(f"{path}  —  {len(groups)} distinct incorrect generations from "
          f"{len(wrong)} failing runs{filt} (retry budget {max_retry})\n")

    for _, recs in sorted(groups.items(), key=lambda kv: (
        kv[1][0]["question"], kv[1][0]["model"], kv[1][0]["db"], kv[1][0]["examples"], kv[1][0]["skills"])):
        r = recs[0]
        reps = f"   [{len(recs)} reps]" if len(recs) > 1 else ""
        expected = "UNANSWERABLE (no query expected)" if r["unanswerable"] else fmt_answer(r["expected"])
        if r["actual"] == "error" and r["error"]:
            actual = "error — " + first_line(r["error"])
        else:
            actual = fmt_answer(r["actual"])
        used = ", ".join(str(u) for u in sorted({rec["retriesUsed"] for rec in recs}))
        print("=" * 90)
        print(f"Q: {r['question']}")
        print(f"   config:   {config_str(r, multi_model)}{reps}")
        print(f"   retries:  {used} of {max_retry} used")
        print(f"   expected: {expected}")
        print(f"   actual:   {actual}")
        print("   expected query:")
        print(block(r["correct_query"]))
        attempts = r["attempts"]
        if len(attempts) <= 1:
            print("   generated query:")
            print(block(attempts[0].get("response") if attempts and not attempts[0].get("query")
                        else r["generated"]))
        else:
            for i, a in enumerate(attempts, 1):
                label = f"attempt {i} (final)" if i == len(attempts) else f"attempt {i}"
                # No query means extraction failed; show the raw reply instead,
                # which is the only thing that explains the failure.
                if a.get("query"):
                    print(f"   generated query, {label}:")
                    print(block(a["query"]))
                else:
                    print(f"   raw response (no query extracted), {label}:")
                    print(block(a.get("response") or "(not recorded)"))
                if a["error"]:
                    print("        error: " + first_line(a["error"]))
    print()


if __name__ == "__main__":
    main()
