# db-llm-bench

Benchmarking LLM query generation across database models

The benchmark asks the same natural-language questions of the
[Reactome](https://reactome.org) pathway database loaded into MySQL, Neo4j and
TypeDB, and scores each model's SQL, Cypher and TypeQL against a known answer.

Individual sub-component READMEs are available in
- [data](data/README.md) - the dataset, prompts, questions and skills
- [databases](databases/README.md) - the `docker compose` stack that loads it
- [src](src/README.md) - the benchmark runner
- [analysis](analysis/README.md) - scripts for summarising a results file
