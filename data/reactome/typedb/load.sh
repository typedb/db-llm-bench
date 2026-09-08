#!/usr/bin/env bash
# Load Reactome into TypeDB, one pass per exported CSV.
#
# Order matters: every entity is loaded before any relation, because a relation
# pass matches its role players by db-id and a missing player is a rejected row,
# not a retry.
#
# The database is deleted first. `--create-db true` is a no-op on an existing
# database, so without the delete a re-run would either collide on db-id (for
# entities, which are keyed) or silently insert a second copy of every relation
# (which are keyless, so nothing would catch it).
#
# Config via env vars, with defaults:
#   ADDRESS=localhost:1729  DB_USER=admin  DB_PASS=password  DB=reactome
#   BATCH_ROWS=1000  PARALLEL=1  TYPEDB=~/.typedb/typedb
# DB_USER/DB_PASS rather than USER/PASS: USER is a standard shell variable.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ADDRESS="${ADDRESS:-localhost:1729}"
DB_USER="${DB_USER:-admin}"
DB_PASS="${DB_PASS:-password}"
DB="${DB:-reactome}"
BATCH_ROWS="${BATCH_ROWS:-1000}"
PARALLEL="${PARALLEL:-1}"
TYPEDB="${TYPEDB:-$HOME/.typedb/typedb}"
WORK="${WORK:-$HERE/work}"
PASSES="${PASSES:-$HERE/passes}"
OUT="${OUT:-$HERE/loader-output}"
SCHEMA="$HERE/schema.tql"

# The database is rebuilt from scratch every run, so a checkpoint from a
# previous run is never resumable — the loader refuses to start if one is
# present, so clear them.
rm -rf "$OUT"
mkdir -p "$OUT"

if [ ! -d "$WORK" ] || [ -z "$(ls -A "$WORK"/*.csv 2>/dev/null)" ]; then
    echo "no exported CSVs in $WORK — run export.py first" >&2
    exit 1
fi

common=(--address "$ADDRESS" --username "$DB_USER" --password "$DB_PASS"
        --database "$DB"
        --header true --batch-rows "$BATCH_ROWS" --parallel-batches "$PARALLEL")

echo "== dropping $DB if present =="
printf 'database delete %s\n' "$DB" > "$OUT/reset.tqls"
"$TYPEDB" console --address "$ADDRESS" --username "$DB_USER" --password "$DB_PASS" \
    --script="$OUT/reset.tqls" >/dev/null 2>&1 || true

first=1
failed=0
run_pass() {                                  # run_pass <csv path>
    local csv="$1" stem query
    local -a extra=()
    stem="$(basename "$csv" .csv)"
    query="$PASSES/$stem.tql"
    [ -f "$query" ] || { echo "  !! no pass for $stem" >&2; failed=1; return; }
    if [ "$first" = 1 ]; then                 # first pass creates the db + schema
        extra=(--create-db true --schema-file "$SCHEMA")
        first=0
    fi
    local rejected
    # ${arr[@]+...} guards the empty-array case: bash 3 with `set -u`
    # treats a bare "${extra[@]}" on an empty array as unbound.
    rejected=$("$TYPEDB" loader "${common[@]}" ${extra[@]+"${extra[@]}"} \
        --query "$query" --data "$csv" --output-dir "$OUT/$stem" 2>&1 \
        | tee "$OUT/$stem.log" | awk '/Rows rejected:/ {print $3}')
    rejected="${rejected:-?}"
    if [ "$rejected" != "0" ]; then
        echo "  !! $stem: $rejected rejected (see $OUT/$stem/rejects.log)" >&2
        failed=1
    fi
    printf '  %-52s rejected=%s\n' "$stem" "$rejected"
}

echo "== entities =="
for csv in "$WORK"/entity__*.csv; do run_pass "$csv"; done

# Multi-valued attributes come after their owners and before relations, for the
# same reason relations come last: the pass matches its owner by db-id, and an
# unmatched owner is a rejected row rather than a retry. The existence guard
# keeps a dataset with no multi-valued attributes from tripping over an
# unexpanded glob, which would read as "no pass for attr__*".
echo "== multi-valued attributes =="
for csv in "$WORK"/attr__*.csv; do [ -e "$csv" ] || continue; run_pass "$csv"; done

echo "== relations =="
for csv in "$WORK"/rel__*.csv; do run_pass "$csv"; done

# Extra players for the multi-valued roles of the reified relations. These come
# last because each one matches a relation by db-id that a rel__ pass inserted:
# one reified node is one relation, and a role it points at several times gets
# several players rather than several copies of the relation.
echo "== relation role links =="
for csv in "$WORK"/link__*.csv; do [ -e "$csv" ] || continue; run_pass "$csv"; done

# Relations whose role player is another relation — the literature evidence for
# a catalysis or regulation. Last, because the relation they point at has to
# exist first, and it is only complete once its link passes have run.
echo "== relations referencing relations =="
for csv in "$WORK"/post__*.csv; do [ -e "$csv" ] || continue; run_pass "$csv"; done

if [ "$failed" = 0 ]; then
    echo "== all passes committed with zero rejects =="
else
    echo "== finished WITH REJECTS — see $OUT ==" >&2
    exit 1
fi
