# TypeDB bulk loader + Python, for the Reactome multi-pass load. The server
# image ships no loader, so we pull the `typedb-all` distribution (which
# bundles `typedb loader`) and add Python for the export/generate steps and
# curl for the Neo4j HTTP export. Keep TYPEDB_VERSION aligned with the typedb
# service's image tag: loader and server share a protocol, and a mismatch
# panics the server on the handshake.
FROM debian:bookworm-slim

ARG TYPEDB_VERSION=3.12.3
ARG TARGETARCH

RUN apt-get update \
    && apt-get install -y --no-install-recommends curl ca-certificates python3 bash \
    && rm -rf /var/lib/apt/lists/*

RUN case "$TARGETARCH" in \
        amd64) ARCH=x86_64 ;; \
        arm64) ARCH=arm64 ;; \
        *) echo "unsupported architecture: $TARGETARCH" && exit 1 ;; \
    esac \
    && NAME="typedb-all-linux-${ARCH}" \
    && curl -sSL "https://repo.typedb.com/public/public-release/raw/names/${NAME}/versions/${TYPEDB_VERSION}/${NAME}-${TYPEDB_VERSION}.tar.gz" \
        | tar xz -C /opt \
    && ln -s "/opt/${NAME}-${TYPEDB_VERSION}/typedb" /usr/local/bin/typedb

# seed.sh (mounted at runtime) runs export -> generate passes -> load.
ENTRYPOINT ["bash", "/reactome/typedb/seed.sh"]
