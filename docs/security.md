# Security Notes

The AI Sidecar is built around containment rather than trust in model behavior.

## V1 Boundaries

- The sidecar exposes no public ports; the backend calls it over a private Docker network.
- Codex runs in a read-only sandbox rooted at the Policy Workspace.
- The Policy Workspace must not contain app source, host data, or broad NAS mounts.
- Provider auth is read-only during normal operation.
- The Security AI is stateless and receives only the stored security prompt plus current user message.
- Executor PostgreSQL access is optional; when enabled, it uses a read-only DB user.
- The Egress Secret Filter fails closed on protected secret detection.

## Explicit Residual Risks

- Codex can use normal tools inside the restricted container.
- DB credentials are intentionally visible to the executor Codex subprocess when executor DB access is enabled.
- There is no v1 concurrency limit.
- There is no v1 SQL allowlist, SQL timeout, row cap, or result-size cap in the sidecar.
- Exact and partial secret filtering is defense in depth; it cannot prove every transformed secret is impossible to leak.

## Operational Rules

- Keep the Docker network membership minimal.
- Keep mounted files read-only except the separate auth maintenance flow.
- Do not place API keys in the deployment environment.
- Use canary secrets in tests instead of trying to leak real credentials.
- Add DB-level row security or separate scoped credentials before exposing private user data.
- Keep `SIDECAR_SECURITY_AI_ENABLED=true` unless you are doing explicit local troubleshooting.
