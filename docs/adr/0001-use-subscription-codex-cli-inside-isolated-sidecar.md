# Use Subscription Codex CLI Inside an Isolated Sidecar

The AI Sidecar uses the Codex subscription CLI as its v1 provider runtime and deliberately excludes API-key or API-billed model access. This keeps the product aligned with the deployment requirement while making the container boundary, provider auth mount, and egress filtering responsible for containing the subscription CLI.
