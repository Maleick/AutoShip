# syntax=docker/dockerfile:1

FROM node:20-bookworm AS frontend-builder
WORKDIR /build

COPY textquest-web/frontend/package.json textquest-web/frontend/package-lock.json ./textquest-web/frontend/
RUN cd textquest-web/frontend && npm ci

COPY textquest-web/frontend ./textquest-web/frontend
RUN cd textquest-web/frontend && npm run build

FROM rust:1-bookworm AS backend-builder
WORKDIR /workspace

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
    cmake \
    clang \
    libclang-dev \
    libasound2-dev \
    libssl-dev \
    pkg-config \
 && rm -rf /var/lib/apt/lists/*

COPY . .
RUN cargo build -p textquest-web --release

FROM debian:bookworm-slim AS runtime
WORKDIR /app

RUN useradd --create-home --uid 10001 --shell /bin/false textquest \
 && mkdir -p /app/data /app/web /app/config \
 && chown -R textquest:textquest /app

RUN apt-get update \
 && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
 && rm -rf /var/lib/apt/lists/*

COPY --from=backend-builder --chown=textquest:textquest /workspace/target/release/textquest-web /usr/local/bin/textquest-web
COPY --from=backend-builder --chown=textquest:textquest /workspace/textquest-web/config ./config
COPY --from=frontend-builder --chown=textquest:textquest /build/textquest-web/frontend/dist ./web/dist

USER textquest
ENV TEXTQUEST_DATA_DIR=/app
EXPOSE 3001
HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
  CMD curl -fsS http://127.0.0.1:3001/api/health > /dev/null
ENTRYPOINT ["/usr/local/bin/textquest-web"]
