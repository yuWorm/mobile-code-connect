# Linux Musl CI/CD Plan

## Steps

1. Add a control crate build script that turns `web/dist` into compile-time embedded assets.
2. Update control routes to serve the embedded SPA while keeping the old admin HTML fallback.
3. Add Dockerfiles for `control-server` and `relayd`.
4. Add a GitHub Actions workflow for CI, musl release binaries, checksums, and GHCR multi-platform images.
5. Verify locally with web build, Rust tests, and a control-server build.

## Verification

Run:

```sh
bun run build
cargo test -p mobilecode_connect_control
cargo build -p control-server
```

