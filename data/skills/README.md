# Skills

Vendored LLM query-writing skills, one folder per DB. Each is loaded (all `.md`
files in the folder) into the prompt's `{{skills}}` slot when a DB's config sets
`skills: data/skills/<db>`, so the benchmark can measure skill-on vs skill-off.

These are third-party skills, downloaded and included under their upstream
licenses. Each source link below is a permalink to the upstream commit the
vendored copy was taken from.

| DB | Skill | Source | License |
| --- | --- | --- | --- |
| `typedb` | TypeQL (TypeDB 3.8+) | [typedb/typedb-skills `typeql.md`](https://github.com/typedb/typedb-skills/blob/ad68f806aa91dfd2ad0c67ffd6101215e6794eb2/typeql.md) (official) | first-party (no explicit license file) |
| `neo4j` | Cypher 25 | [neo4j-contrib/neo4j-skills `neo4j-cypher-skill/SKILL.md`](https://github.com/neo4j-contrib/neo4j-skills/blob/bdbce1aadd5827d1acf69634f2bdc004b7f3692f/neo4j-cypher-skill/SKILL.md) | MIT |
| `sql` | SQL queries (multi-dialect) | [anthropics/knowledge-work-plugins `data/skills/sql-queries/SKILL.md`](https://github.com/anthropics/knowledge-work-plugins/blob/2d6f7e22dd25593f0f748010430ef86f19659735/data/skills/sql-queries/SKILL.md) | Apache-2.0 |

Notes:

- Only the top-level skill `.md` of each upstream is vendored (`load_skills`
  reads a skill folder non-recursively). The Neo4j upstream also ships a
  `references/` directory of deeper material not included here.
- The SQL skill teaches analytical warehouse dialects (PostgreSQL, Snowflake,
  BigQuery, Redshift, Databricks) and does not cover MySQL, the engine this
  dataset runs on; no database-specific SQL skill comparable to the TypeQL and
  Cypher ones exists.
- The Neo4j skill targets Cypher 25 (Neo4j 2025.x+) and tells the model to open
  every query with `CYPHER 25`; the benchmark server is Neo4j 5.26, which
  rejects that preamble. No comparably sourced, licensed Cypher-5 skill exists
  (the closest, tomasonjo/blogs `neo4j-cypher-guide`, has no license), so the
  skill is kept as-is and `data/prompts/neo4j.txt` names the server version
  instead, as the TypeDB prompt does for TypeQL 3.x.
