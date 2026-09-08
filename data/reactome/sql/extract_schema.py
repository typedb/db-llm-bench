#!/usr/bin/env python3
"""Derive the prompt's schema.sql from Reactome's MySQL dump.

The dump interleaves DDL with ~841MB of INSERTs and with MySQL's versioned
/*!...*/ directives; the prompt wants the DDL alone. Storage and charset
clauses are dropped because they repeat identically on all 242 tables and say
nothing about the data model, but index declarations are KEPT: the dump has no
FOREIGN KEY constraints at all (every table is MyISAM), so the KEY lines are
the only structural signal about which columns join to which.

Usage: extract_schema.py [gk_current.sql] [schema.sql]
"""

import pathlib
import re
import sys

EXPECTED_TABLES = 242


def extract(dump: str) -> str:
    blocks = re.findall(r"^CREATE TABLE .*?^\) [^;]*;", dump, re.M | re.S)
    if len(blocks) != EXPECTED_TABLES:
        raise SystemExit(
            f"expected {EXPECTED_TABLES} tables, found {len(blocks)} — "
            "has the release changed?"
        )
    cleaned = []
    for block in blocks:
        block = re.sub(r"\) ENGINE=[^;]*;", ");", block)
        block = re.sub(r" CHARACTER SET \w+ COLLATE \w+", "", block)
        block = re.sub(r" COLLATE \w+", "", block)
        cleaned.append(block.rstrip())
    return "\n\n".join(cleaned) + "\n"


def main() -> None:
    here = pathlib.Path(__file__).parent
    src = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else here / "gk_current.sql"
    dst = pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else here / "schema.sql"
    out = extract(src.read_text(encoding="utf8", errors="replace"))
    dst.write_text(out, encoding="utf8")
    print(f"wrote {dst} — {len(out)} chars, ~{len(out) // 4} est. tokens")


if __name__ == "__main__":
    main()
