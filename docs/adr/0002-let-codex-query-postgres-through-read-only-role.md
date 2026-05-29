# Let Codex Query Postgres Through a Read-Only Role

Codex is allowed to invoke `psql` directly inside the restricted container using PostgreSQL environment variables. The sidecar does not allowlist SQL, cap result output, or hide DB credentials from Codex in v1; the database read-only role is the access boundary, and the egress filter treats DB credentials as protected secrets.
