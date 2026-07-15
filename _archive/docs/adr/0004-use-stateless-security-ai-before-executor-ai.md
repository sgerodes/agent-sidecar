# Use Stateless Security AI Before Executor AI

The sidecar runs an optional Security AI before the executor. It receives only the stored security prompt and current user message, returns strict JSON with `allow` or `block`, and fails closed on timeout, process failure, invalid JSON, or any non-`allow` decision. This keeps prompt-driven gatekeeping separate from executor behavior and avoids giving the guard database access or sticky chat context.
