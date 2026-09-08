# Data

- `prompts/<db>.txt` - the prompt template for each DB. Templates are dataset-independent, so they
  are kept apart from the dataset. `sql.txt` is worded for PostgreSQL and `sql-mysql.txt` for MySQL;
  a dataset config picks whichever matches its engine.
- `skills/<db>/` - vendored third-party query-writing skills, loaded into the prompt's `{{skills}}`
  slot when a DB config opts in. See [`skills/README.md`](skills/README.md).
- `reactome/` - the benchmark dataset: the Reactome curation database, restored from the MySQL and
  Neo4j dumps Reactome publishes per release and exported from the graph into TypeDB. Carries
  `questions.json` and its `questions-review.md` rendering, and per DB the schema handed to the
  model, the `example-N.txt` few-shot examples, and the scripts that derive the schema or drive the
  load. The dumps themselves are gitignored;
  [`../databases/reactome/README.md`](../databases/reactome/README.md) says where to fetch them.

A dataset folder holds, per DB, the schema and data files needed to load it, and optionally the
`example-1.txt`, `example-2.txt`, ... examples fed into the prompt's examples slot.
