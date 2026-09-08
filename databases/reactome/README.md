# Reactome DBs

`docker compose` boots the three Reactome benchmark DBs, restores the MySQL and
Neo4j dumps from [`../../data/reactome`](../../data/reactome), and exports the
graph from Neo4j into TypeDB:

```sh
cd databases/reactome
docker compose up -d --wait neo4j mysql   # restore the dumps
docker compose up -d --wait               # then export Neo4j -> TypeDB
```

`--wait` blocks until the loads have finished — no service reports healthy
before then. Without it, `up -d` returns while the data is still loading. Neo4j
must be up before `typedb-load` starts (see [TypeDB](#typedb) for why it is not
a `depends_on`).

## Getting the dumps

Both dumps are far over GitHub's file-size limit and are gitignored, so a fresh
clone has to fetch them before the stack will start:

```sh
mkdir -p data/reactome/sql data/reactome/neo4j
curl -L https://reactome.org/download/current/databases/gk_current.sql.gz \
  | gunzip > data/reactome/sql/gk_current.sql
curl -Lo data/reactome/neo4j/reactome.graphdb.dump \
  https://reactome.org/download/current/reactome.graphdb.dump
```

The derived `data/reactome/sql/schema.sql` (the 242-table DDL used in the
prompt) *is* committed, so it does not need regenerating on every clone — only
when moving to a new Reactome release:

```sh
python3 data/reactome/sql/extract_schema.py
```

That script fails loudly if the release no longer has exactly 242 tables, which
is the signal that the questions and their expected values need rechecking.

| DB    | Endpoint                       | Credentials                                                                | Seeding                                                              |
| ----- | ------------------------------ | -------------------------------------------------------------------------- | -------------------------------------------------------------------- |
| MySQL | `localhost:3306` (`reactome`)  | benchmark: `bench_ro` / `bench_ro` (SELECT-only); admin: `root` / `password` | self-seeding via `/docker-entrypoint-initdb.d` on first init          |
| Neo4j | `bolt://localhost:7687`        | `neo4j` / `password`                                                        | one-shot `neo4j-load` → `neo4j-migrate`, then the server starts       |
| TypeDB | `localhost:1729` (`reactome`) | `admin` / `password`                                                       | one-shot `typedb-load`: export from Neo4j → generate passes → bulk load (~25 min) |

## Why the loads are independent

Reactome publishes each release as two native dumps — a MySQL dump of the
curation database and a Neo4j dump generated from it — so each DB restores its
own, with no shared source file to prep and no per-DB seed step.

That also means the two DBs are not loaded from a common source here. They are
both derived from the same Reactome release upstream, but the graph is produced
by Reactome's own `graph-importer`, so cross-DB count parity is not something
this stack guarantees — and the loaded data confirms it does not hold:

| | MySQL | Neo4j |
| --- | --- | --- |
| `DatabaseObject` | 1,871,599 | 2,958,129 |
| `PhysicalEntity` | 410,334 | 410,271 |

The `graph-importer` is a transformation, not a mirror. **Any question authored
against this dataset needs its expected answer established per DB rather than
assumed shared**, and the divergence should be characterised before questions
are written — identical counts across DBs cannot be taken for granted here.

## MySQL

The dump is MySQL, not Postgres — backtick quoting, `ENGINE=MyISAM`,
`int unsigned`, `utf8mb3` collations. It will not load into Postgres, which is
why this stack runs `mysql:8.0` (the SQL package speaks both engines and picks
one from the URL scheme). Pinned to 8.0 deliberately: all 242 tables are
MyISAM, and MySQL 9.x drops MyISAM support.

The dump carries no `CREATE DATABASE` or `USE` statement, so what places it in
the `reactome` schema is the entrypoint running it with
`--database=$MYSQL_DATABASE`. It contains no views, stored routines, triggers,
or `DEFINER` clauses, so no extra privileges are needed at load time.

Verified after a from-scratch `up`: all **242 tables** loaded, `DatabaseObject`
1,871,599 rows, `PhysicalEntity` 410,334, `ReactionlikeEvent` 95,780, `Pathway`
23,604. `bench_ro` reads and is refused `CREATE`.

### Querying it from the benchmark

`src/dbs/sql` speaks both engines and picks one from the URL scheme, so a
dataset config selects MySQL with nothing but its `url`:

```yaml
- sql:
    url: mysql://bench_ro:bench_ro@localhost/reactome
```

Type coverage was checked against all 242 tables. Every column type Reactome
uses is coerced except `longblob`, which appears exactly twice —
`Ontology.ontology` and `PathwayDiagram.storedATXML`, both serialized payloads
rather than queryable data. Selecting either returns a `WrongShape` error naming
the column, which is the intended behaviour: there is no canonical benchmark
value for a binary blob.

One modelling quirk worth knowing before authoring questions: Reactome stores
booleans as `enum('TRUE','FALSE')` in 9 columns, so they come back as the
**strings** `"TRUE"`/`"FALSE"`, not booleans. That is what MySQL actually holds —
the package does not second-guess it — but an expected answer written as a
boolean will not match.

## Neo4j

Restoring takes **two** offline steps before the server can start, so both run
as one-shot containers sharing the `neo4j_data` volume:

1. `neo4j-load` — `neo4j-admin database load`.
2. `neo4j-migrate` — `neo4j-admin database migrate --force-btree-indexes-to-range`.

The migrate step is not optional. Reactome's dump is written in the **AF4.3.0**
store format (introduced in Neo4j 4.3); `database load` restores it happily, but
the server refuses to start on it. Migration in turn aborts by default because
Neo4j 5 removed BTREE indexes and the dump carries 13 BTREE indexes plus 23
uniqueness constraints backed by BTREE indexes — hence
`--force-btree-indexes-to-range`, which rebuilds them as RANGE. Reactome's are
all single-property exact-match lookups (`dbId`, `stId`, `identifier`), which
RANGE serves well, but they are repopulated from scratch, so the first queries
after a fresh load are slower until that settles.

Two details worth knowing if you change the file layout:

- `neo4j-admin` resolves the dump as `<database>.dump` inside `--from-path`, so
  the bind mount renames `reactome.graphdb.dump` to `neo4j.dump`.
- Community edition serves exactly one user database, always named `neo4j`, so
  that is the load target.

Verified after a from-scratch `up`: **2,958,129 nodes** and **11,535,888
relationships**, with 36 RANGE indexes and 23 uniqueness constraints. Every node
carries the `DatabaseObject` label (count equals the total), which is the
polymorphic supertype — labels encode the class hierarchy
(`DatabaseObject` → `PhysicalEntity` → `GenomeEncodedEntity` →
`EntityWithAccessionedSequence`).

## TypeDB

Reactome publishes no TypeDB dump, so `typedb-load` builds one from the graph:
[`data/reactome/typedb/seed.sh`](../../data/reactome/typedb/seed.sh) runs
`export.py` (Neo4j → one CSV per concrete type, over the HTTP API),
`generate_passes.py` (one loader `.tql` per CSV, its `given` block derived from
the CSV header; a column that is ever blank becomes a nullable variable whose
player or attribute sits in a `try` block, so optional roles need no extra
passes) and
`load.sh` (drops `reactome`, installs `schema.tql`, then bulk-loads every pass
with `typedb loader`). The server image ships no loader, so `loader/Dockerfile`
pulls the `typedb-all` distribution plus Python and curl. Its `TYPEDB_VERSION`
must match the `typedb` service's image tag (**3.12.3**): loader and server
share a protocol, and a mismatch panics the server on the handshake.

Two things about the wiring:

- `typedb-load` deliberately does **not** `depends_on` Neo4j, although the
  export reads from it: that would re-trigger `neo4j-load`, whose
  `neo4j-admin database load` cannot overwrite a store while the server is
  running. Bring Neo4j up first; `seed.sh` checks it is reachable and stops
  with a clear message if not.
- The store lives on the `typedb_data` volume, mounted at the image's declared
  `VOLUME` path, so the ~25 minute load survives container recreation. Only
  `typedb-load` running again reloads it — so once loaded, restart the server
  alone with `docker compose up -d typedb` rather than a bare `up`, which
  re-runs every one-shot service.

The same `seed.sh` runs host-side against any TypeDB server (defaults:
`localhost:1729`, `admin` / `password`, `~/.typedb/typedb` for the loader).

Because the store is built from `schema.tql` and the export scripts rather than
restored from a dump, a change to any of them means a rebuild, not a migration:
`docker compose up -d typedb-load` re-exports and reloads from scratch. Neo4j
must be up, and nothing should be querying TypeDB while it runs.

Verified after a from-scratch `up`: **205 passes committed with zero rejects**,
and every reference query in `data/reactome/questions.json` reproduces its
expected value (`cargo run -p bench-cli --bin verify -- src/reactome.yml`).

## Notes

- The compose project is named `reactome`, so its containers and volumes are
  namespaced by dataset rather than by the `databases/reactome` folder name.
- Re-running `docker compose up` does **not** reload MySQL: the init hook fires
  only when the data directory is empty. Neo4j does reload, because
  `neo4j-load` passes `--overwrite-destination=true`, and so does TypeDB,
  because `load.sh` drops the database first. To reset everything:
  `docker compose down -v`, then the two-step `up` from the top of this file.
