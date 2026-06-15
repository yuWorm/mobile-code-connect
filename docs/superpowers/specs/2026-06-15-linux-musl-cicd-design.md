# Linux Musl CI/CD Design

## Scope

Publish Linux-only artifacts for:

- `control-server`
- `relayd`

The release must include:

- GitHub Release tarballs for Linux musl targets.
- Multi-platform Docker images for `linux/amd64` and `linux/arm64`.
- A `control-server` binary that embeds the compiled web UI.

Windows is out of scope for this release path.

## Targets

Release binary targets:

- `x86_64-unknown-linux-musl`
- `aarch64-unknown-linux-musl`

Docker platforms:

- `linux/amd64`
- `linux/arm64`

## Artifact Names

GitHub Release assets:

- `control-server-x86_64-unknown-linux-musl.tar.gz`
- `control-server-aarch64-unknown-linux-musl.tar.gz`
- `relayd-x86_64-unknown-linux-musl.tar.gz`
- `relayd-aarch64-unknown-linux-musl.tar.gz`
- `sha256sums.txt`

Docker images:

- `ghcr.io/yuworm/mobile-code-connect/control-server`
- `ghcr.io/yuworm/mobile-code-connect/relayd`

## Web Embedding

`web/dist` is produced before compiling `control-server`.

The control crate build script embeds files from `web/dist` into generated Rust source under `OUT_DIR`. If `web/dist` is absent, local development still compiles and `/admin` falls back to the existing HTML page.

Embedded routing serves:

- `/`
- `/admin`
- `/admin/*`
- `/center`
- `/center/*`
- `/login`
- `/login/*`
- `/assets/*`

API routes keep their existing explicit paths and are not changed.

## Release Trigger

- Pull requests and `master` pushes run CI.
- Git tags matching `v*` publish release binaries and Docker images.
- `master` pushes publish Docker images tagged from the branch and `latest`.

