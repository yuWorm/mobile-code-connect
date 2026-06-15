# syntax=docker/dockerfile:1.7

FROM oven/bun:1.2-alpine AS web
WORKDIR /src/web
COPY web/package.json web/bun.lock ./
RUN bun install --frozen-lockfile
COPY web/ ./
RUN bun run build

FROM rust:1.95-alpine AS builder
RUN apk add --no-cache build-base
WORKDIR /src
COPY . .
COPY --from=web /src/web/dist /src/web/dist
RUN cargo build --release -p control-server

FROM alpine:3.22
RUN apk add --no-cache ca-certificates curl
COPY --from=builder /src/target/release/control-server /usr/local/bin/control-server
ENTRYPOINT ["/usr/local/bin/control-server"]

