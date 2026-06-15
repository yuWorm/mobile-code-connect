# syntax=docker/dockerfile:1.7

FROM rust:1.95-alpine AS builder
RUN apk add --no-cache build-base
WORKDIR /src
COPY . .
RUN cargo build --release -p relayd

FROM alpine:3.22
RUN apk add --no-cache ca-certificates
COPY --from=builder /src/target/release/relayd /usr/local/bin/relayd
ENTRYPOINT ["/usr/local/bin/relayd"]

