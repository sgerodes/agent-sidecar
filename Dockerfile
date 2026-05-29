FROM rust:1.92-trixie AS builder

WORKDIR /build
COPY Cargo.toml Cargo.lock* ./
COPY src ./src
RUN cargo build --release

FROM node:22-trixie-slim AS runtime

ARG APP_UID=10001
ARG APP_GID=10001

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates postgresql-client \
    && npm install -g @openai/codex \
    && npm cache clean --force \
    && rm -rf /var/lib/apt/lists/*

RUN groupadd --system --gid "${APP_GID}" sidecar \
    && useradd --system --uid "${APP_UID}" --gid sidecar --home-dir /nonexistent --shell /usr/sbin/nologin sidecar

COPY --from=builder /build/target/release/agent-sidecar /usr/local/bin/agent-sidecar
COPY config/policy-workspace /opt/agent-sidecar/policy

RUN chown -R root:root /opt/agent-sidecar \
    && chmod -R a-w /opt/agent-sidecar \
    && chmod 0755 /usr/local/bin/agent-sidecar

USER sidecar:sidecar
EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/agent-sidecar"]
