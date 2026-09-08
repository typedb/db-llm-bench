# Databases

One `docker compose` stack per dataset. There is currently one dataset,
Reactome, so [`reactome/`](reactome/README.md) is the only stack:

```sh
cd databases/reactome
docker compose up -d --wait
```

It boots MySQL, Neo4j and TypeDB, restores Reactome's dumps into the first two,
and exports from Neo4j into TypeDB. `--wait` blocks until the loads finish;
the README there covers fetching the dumps, credentials, and load timings.

`mysql/roles.sql` holds the SELECT-only benchmark role and is shared by any
MySQL-backed stack rather than living inside one dataset's folder.

Credentials and endpoints match the defaults in `src/reactome.yml` and the DB
packages' `#[ignore]`d live tests (`cargo test -- --ignored`).
