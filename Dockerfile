# syntax=docker/dockerfile:1

FROM node:22-alpine AS web
WORKDIR /web
COPY web/package.json web/package-lock.json ./
RUN npm ci
COPY web/ ./
RUN npm run build

FROM rust:1-slim-bookworm AS server
WORKDIR /src/server
COPY core/ /src/core/
COPY server/Cargo.toml server/Cargo.lock ./
# Build dependencies once so source edits don't recompile the world.
RUN mkdir src && echo 'fn main() {}' > src/main.rs && cargo build --release && rm -rf src
COPY server/ ./
RUN touch src/main.rs && cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --uid 10001 --home /app fxbet \
    && mkdir -p /data && chown fxbet /data
WORKDIR /app
COPY --from=server /src/server/target/release/fxbet /app/fxbet
COPY --from=web /web/dist /app/web
ENV FXBET_BIND=0.0.0.0:8080 \
    FXBET_DATABASE_URL=sqlite:///data/fxbet.db?mode=rwc \
    FXBET_STATIC_DIR=/app/web \
    RUST_LOG=info
USER fxbet
VOLUME /data
EXPOSE 8080
HEALTHCHECK --interval=30s --timeout=3s CMD curl -fsS http://127.0.0.1:8080/api/health || exit 1
CMD ["/app/fxbet"]
