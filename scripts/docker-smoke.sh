#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
image_name="${SIDECAR_SMOKE_IMAGE:-agent-sidecar:smoke}"

cd "${repo_root}"

SIDECAR_PGPASSWORD="${SIDECAR_PGPASSWORD:-smoke-password}" \
  docker compose -f deploy/docker-compose.example.yml config >/tmp/agent-sidecar-compose.yml

if grep -qE '^[[:space:]]+ports:' /tmp/agent-sidecar-compose.yml; then
  echo "sidecar compose config must not publish ports" >&2
  exit 1
fi

require_compose_line() {
  local pattern="$1"
  if ! grep -q "${pattern}" /tmp/agent-sidecar-compose.yml; then
    echo "compose config missing expected line: ${pattern}" >&2
    exit 1
  fi
}

require_compose_line 'read_only: true'
require_compose_line 'target: /opt/agent-sidecar/policy'
require_compose_line 'target: /codex-auth'
require_compose_line 'no-new-privileges:true'

docker build -t "${image_name}" .

docker run --rm \
  --read-only \
  --user 10001:10001 \
  --entrypoint /bin/sh \
  "${image_name}" \
  -c 'test "$(id -u)" = "10001" \
    && test ! -w / \
    && test -r /opt/agent-sidecar/policy/AGENTS.md \
    && command -v psql >/dev/null \
    && command -v codex >/dev/null'

echo "docker smoke checks passed"
