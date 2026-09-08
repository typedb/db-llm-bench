#!/usr/bin/env python3
"""Merge two benchmark results JSONs (as written by `bench-cli`) into one.

Questions are matched on their question text. Where both files carry a
question, its per-DB `results` lists are concatenated (base file's runs
first); question- and DB-level metadata (difficulty, expected, reference
queries, ...) is taken from the incoming file, on the assumption that the
incoming file is the newer run against the current questions file. Questions
present in only one file are carried through unchanged.

No dedup is attempted: merging two files that share runs will double-count
them, exactly as it would double-count repetitions.

Usage: merge_results.py <base.json> <incoming.json> <out.json>
"""

import json
import sys


def merge(base, incoming):
    incoming_by_text = {q["question"]: q for q in incoming["questions"]}
    merged = []
    for q in base["questions"]:
        new = incoming_by_text.pop(q["question"], None)
        if new is None:
            merged.append(q)
            continue
        combined = dict(new)  # incoming metadata wins
        combined["dbs"] = dict(new["dbs"])
        for db, info in q["dbs"].items():
            if db in combined["dbs"]:
                new_info = dict(combined["dbs"][db])
                new_info["results"] = info["results"] + new_info["results"]
                combined["dbs"][db] = new_info
            else:
                combined["dbs"][db] = info
        merged.append(combined)
    # Questions only in the incoming file, in their original order.
    merged.extend(incoming_by_text.values())
    return {**base, **incoming, "questions": merged}


def main():
    if len(sys.argv) != 4:
        sys.exit(__doc__.strip().splitlines()[-1])
    base_path, incoming_path, out_path = sys.argv[1:]
    merged = merge(json.load(open(base_path)), json.load(open(incoming_path)))
    with open(out_path, "w") as f:
        json.dump(merged, f, indent=2, ensure_ascii=False)
        f.write("\n")
    runs = sum(
        len(info["results"]) for q in merged["questions"] for info in q["dbs"].values()
    )
    print(f"wrote {out_path} ({len(merged['questions'])} questions, {runs} runs)")


if __name__ == "__main__":
    main()
