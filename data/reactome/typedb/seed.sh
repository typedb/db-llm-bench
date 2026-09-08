#!/usr/bin/env bash
# Export Reactome from Neo4j and load it into TypeDB, end to end.
#
# Three steps, because each depends on the previous one's output:
#   1. export.py          Neo4j  -> per-pass CSVs (reassembling the n-ary facts)
#   2. generate_passes.py CSVs   -> one loader .tql per CSV
#   3. load.sh            CSVs + passes -> TypeDB
#
# Passes are regenerated rather than taken from git because each one's `given`
# block is derived from its CSV's header: a Reactome release that adds a column
# would otherwise load against a stale template.
#
# Config is via the same env vars the three scripts already read, so this adds
# no configuration of its own. See load.sh for the TypeDB ones and export.py
# for NEO4J_HTTP / NEO4J_USER / NEO4J_PASS.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK="${WORK:-$HERE/work}"

NEO4J_HTTP="${NEO4J_HTTP:-http://localhost:7474/db/neo4j/tx/commit}"
if ! curl -sf -u "${NEO4J_USER:-neo4j}:${NEO4J_PASS:-password}" \
        -H 'Content-Type: application/json' \
        -d '{"statements":[{"statement":"RETURN 1"}]}' "$NEO4J_HTTP" >/dev/null; then
    echo "Neo4j is not reachable at $NEO4J_HTTP — the export reads from it." >&2
    echo "Start it first:  docker compose up -d --wait neo4j" >&2
    exit 1
fi

echo "== exporting from Neo4j =="
python3 "$HERE/export.py" "$WORK"

echo "== generating loader passes =="
python3 "$HERE/generate_passes.py" "$WORK" "$HERE/passes"

echo "== loading into TypeDB =="
WORK="$WORK" exec "$HERE/load.sh"
