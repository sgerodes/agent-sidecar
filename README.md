# Agent Sidecar

Rust AI Sidecar service for running Codex subscription CLI inside a restricted Docker container.

V1 is intentionally narrow: private REST API in, supervised `codex exec` out, direct read-only PostgreSQL access through `psql`, structured JSON responses, and fail-closed egress secret filtering.

## API

`GET /healthz`

Returns process health.

`GET /readyz`

Runs `SELECT 1` with the configured PostgreSQL credentials.

`POST /v1/chat`

```json
{
  "conversation_id": "telegram-chat-123",
  "conversation": [
    { "role": "user", "content": "Previous user message" },
    { "role": "assistant", "content": "Previous assistant answer" }
  ],
  "user_message": "What should I know today?",
  "metadata": {
    "telegram_user_id": "12345"
  }
}
```

Successful response:

```json
{
  "request_id": "cf9f64cb-6a39-4483-b008-3531ef4f87f3",
  "status": "completed",
  "answer": "Answer text",
  "provider": "codex",
  "diagnostics": {
    "duration_ms": 2500,
    "secret_filter_checked": true,
    "provider_stdout_bytes": 42,
    "provider_stderr_bytes": 0
  }
}
```

## Configuration

Required:

- `PGHOST`
- `PGDATABASE`
- `PGUSER`
- `PGPASSWORD`

Common optional settings:

- `SIDECAR_BIND_ADDR`, default `0.0.0.0:8080`
- `SIDECAR_POLICY_WORKSPACE`, default `/opt/agent-sidecar/policy`
- `SIDECAR_RESPONSE_SCHEMA`, default `<policy workspace>/response.schema.json`
- `SIDECAR_CODEX_COMMAND`, default `codex`
- `SIDECAR_CODEX_HOME`, provider auth directory
- `SIDECAR_CODEX_MODEL`, optional Codex model override
- `SIDECAR_PROVIDER_TIMEOUT_SECONDS`, default `120`
- `SIDECAR_SECRET_FILE_PATHS`, comma-separated protected secret files to inventory
- `SIDECAR_CANARY_SECRETS`, comma-separated canary secrets for tests

The service refuses to start if API-key style model credentials such as `OPENAI_API_KEY` are present. This project is subscription-CLI only.

## Docker

See [deploy/docker-compose.example.yml](deploy/docker-compose.example.yml). The sidecar service uses `expose`, not public `ports`.

The runtime image installs Codex CLI using the current official npm package path, `@openai/codex`. You can replace that with a pinned release binary or an internal base image if you want a smaller or more controlled NAS image.

Normal operation mounts Codex auth read-only. Use [deploy/docker-compose.auth-maintenance.example.yml](deploy/docker-compose.auth-maintenance.example.yml) for a separate writable login/refresh flow.

## Local Development

```sh
cargo fmt
cargo test
```

Run locally with a configured Codex subscription login and PostgreSQL environment:

```sh
SIDECAR_POLICY_WORKSPACE="$PWD/config/policy-workspace" \
SIDECAR_RESPONSE_SCHEMA="$PWD/config/policy-workspace/response.schema.json" \
SIDECAR_CODEX_HOME="$HOME/.codex" \
PGHOST=localhost \
PGPORT=5432 \
PGDATABASE=public_data \
PGUSER=ai_readonly \
PGPASSWORD=replace-with-read-only-password \
cargo run
```

Do not put these values in a `.env` file for this project.

## Real Codex Smoke Tests

Default tests use fake providers. Ignored integration tests exercise the real Codex CLI:

```sh
SIDECAR_RUN_REAL_CODEX_TESTS=1 \
SIDECAR_POLICY_WORKSPACE="$PWD/config/policy-workspace" \
SIDECAR_RESPONSE_SCHEMA="$PWD/config/policy-workspace/response.schema.json" \
SIDECAR_CODEX_HOME="$HOME/.codex" \
PGHOST=localhost \
PGPORT=5432 \
PGDATABASE=public_data \
PGUSER=ai_readonly \
PGPASSWORD=replace-with-read-only-password \
cargo test --test real_codex -- --ignored
```

Use canary secrets for leakage tests.

## Docker Smoke Test

This opt-in check builds the image and verifies the hardening shape from the Compose example:

```sh
bash scripts/docker-smoke.sh
```
