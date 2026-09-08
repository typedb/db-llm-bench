#!/usr/bin/env python3
"""Derive the prompt's Neo4j schema from a running Reactome graph.

Backbone is `CALL db.schema.visualization()`, but its raw output is unusable as
a prompt: Reactome nodes carry several labels each (a Reaction is also a
ReactionLikeEvent, an Event and a DatabaseObject), and visualization emits one
pattern per label pair, giving 5669 patterns for 88 relationship types.

So the label hierarchy is derived first — label A is an ancestor of B when
every label set containing B also contains A — and each pattern is then
reduced to its most specific endpoints. That drops 5669 patterns to ~1300
without losing a single real relationship, because the general forms are all
implied by the hierarchy, which the file states explicitly.

Usage: extract_schema.py [bolt-url] [out.txt]     (defaults: bolt://localhost:7687)
"""

import pathlib
import subprocess
import sys

COMPOSE_DIR = pathlib.Path(__file__).resolve().parents[3] / "databases" / "reactome"
# Mixins that sit outside the domain hierarchy; naming them keeps the "is a"
# lines about the data model rather than about curation bookkeeping.
MIXINS = {"Trackable", "Deletable"}


def cypher(query: str) -> list[str]:
    """Run a query through the compose stack's cypher-shell, one row per line."""
    out = subprocess.run(
        ["docker", "compose", "exec", "-T", "neo4j", "cypher-shell",
         "-u", "neo4j", "-p", "password", "--format", "plain", query],
        capture_output=True, text=True, cwd=COMPOSE_DIR,
    )
    if out.returncode != 0:
        raise SystemExit(f"cypher-shell failed:\n{out.stderr or out.stdout}")
    rows = [r.strip().strip('"') for r in out.stdout.splitlines() if r.strip()]
    return rows[1:]  # drop the header


def main() -> None:
    dst = pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else pathlib.Path(__file__).with_name("schema.txt")

    combos = []
    for row in cypher("MATCH (n) WITH labels(n) AS ls, count(*) AS c "
                      "RETURN reduce(s='', x IN ls | s + x + ',') + '\t' + toString(c) AS row"):
        labels, count = row.split("\t")
        combos.append((set(x for x in labels.split(",") if x), int(count)))

    all_labels = sorted({l for s, _ in combos for l in s})
    holds = {l: {i for i, (s, _) in enumerate(combos) if l in s} for l in all_labels}
    ancestors = {b: {a for a in all_labels if a != b and holds[b] <= holds[a]} for b in all_labels}
    descendants = {a: {b for b in all_labels if a in ancestors[b]} for a in all_labels}

    def at_or_below(x: str, y: str) -> bool:
        return x == y or x in descendants[y]

    triples = set()
    for row in cypher(
        "CALL db.schema.visualization() YIELD nodes, relationships UNWIND relationships AS r "
        "WITH nodes, r, "
        " head([n IN nodes WHERE elementId(n)=elementId(startNode(r)) | labels(n)[0]]) AS a, "
        " head([n IN nodes WHERE elementId(n)=elementId(endNode(r))   | labels(n)[0]]) AS b "
        "RETURN DISTINCT a + '\t' + type(r) + '\t' + b AS triple ORDER BY triple"
    ):
        parts = row.split("\t")
        if len(parts) == 3 and all(parts):
            triples.add(tuple(parts))

    by_rel: dict[str, set] = {}
    for a, rel, b in triples:
        by_rel.setdefault(rel, set()).add((a, b))
    kept = sorted(
        (a, rel, b)
        for rel, pairs in by_rel.items()
        for (a, b) in pairs
        if not any((x, y) != (a, b) and at_or_below(x, a) and at_or_below(y, b) for (x, y) in pairs)
    )

    props: dict[str, dict[str, str]] = {}
    for row in cypher(
        "CALL db.schema.nodeTypeProperties() YIELD nodeType, propertyName, propertyTypes "
        "RETURN replace(replace(nodeType,':`','+'),'`','') + '\t' + coalesce(propertyName,'') "
        "+ '\t' + reduce(s='',t IN propertyTypes | s+t+'|') AS row"
    ):
        node_type, name, types = (row.split("\t") + ["", ""])[:3]
        if not name:
            continue
        labels = {x for x in node_type.split("+") if x}
        specific = sorted(l for l in labels if not any(l in ancestors[o] for o in labels if o != l))
        for label in specific:
            props.setdefault(label, {})[name] = types.strip("|").replace("|", " or ")

    lines = [
        "# Reactome graph schema, derived from CALL db.schema.visualization().",
        "# Every node also carries the labels of its ancestors, listed under 'is a'.",
        "# A pattern written with a general label also matches its subtypes.",
        "",
        "## Label hierarchy",
        "",
    ]
    for label in all_labels:
        parents = sorted(ancestors[label] - MIXINS)
        if parents:
            lines.append(f"{label} is a {', '.join(parents)}")
    lines += ["", "## Node properties", ""]
    for label in sorted(props):
        lines.append(f"(:{label})")
        for name in sorted(props[label]):
            lines.append(f"  {name}: {props[label][name]}")
        lines.append("")
    lines += ["## Relationships", ""]
    for a, rel, b in kept:
        lines.append(f"(:{a})-[:{rel}]->(:{b})")

    text = "\n".join(lines) + "\n"
    dst.write_text(text, encoding="utf8")
    print(f"wrote {dst} — {len(all_labels)} labels, {len(kept)} patterns "
          f"(from {len(triples)} raw), {len(text)} chars, ~{len(text)//4} est. tokens")


if __name__ == "__main__":
    main()
