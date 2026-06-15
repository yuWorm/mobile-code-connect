# Release and CI/CD

This repository publishes Linux server artifacts for `control-server` and
`relayd`, plus mobile SDK package artifacts for Android and iOS.

## GitHub Actions

`.github/workflows/ci-cd.yml` runs on:

- pull requests targeting `master`
- pushes to `master`
- tags matching `v*`
- manual `workflow_dispatch`

PRs and `master` pushes run the web build, web tests, Rust workspace tests, and
mobile packaging dry-runs. Tags matching `v*` additionally publish GitHub
Release binaries and mobile package assets. Pushes to `master` and release tags
publish Docker images to GHCR.

## Release Binaries

Release tags publish musl tarballs for:

- `x86_64-unknown-linux-musl`
- `aarch64-unknown-linux-musl`

Expected assets:

- `control-server-x86_64-unknown-linux-musl.tar.gz`
- `control-server-aarch64-unknown-linux-musl.tar.gz`
- `relayd-x86_64-unknown-linux-musl.tar.gz`
- `relayd-aarch64-unknown-linux-musl.tar.gz`
- `mobilecode-connect-mobile-android.aar`
- `mobilecode-connect-mobile-android-manifest.json`
- `mobilecode-connect-mobile-ios-xcframework.zip`
- `mobilecode-connect-mobile-ios-manifest.json`
- `sha256sums.txt`

Create a release by pushing a tag:

```sh
git tag v0.1.0
git push origin v0.1.0
```

## Docker Images

Images are published as multi-platform `linux/amd64` and `linux/arm64` images:

```text
ghcr.io/yuworm/mobile-code-connect/control-server
ghcr.io/yuworm/mobile-code-connect/relayd
```

Branch pushes produce branch tags and `latest` on the default branch. Release
tags produce matching version tags.

## Mobile Packages

Release tags build Android and iOS SDK package assets with the existing scripts:

```sh
scripts/package-mobile-android.sh --gradle-task assembleRelease
scripts/package-mobile-ios.sh
```

Android release builds run on Ubuntu and package the default ABI set:

- `arm64-v8a`
- `armeabi-v7a`
- `x86_64`
- `x86`

The Android GitHub Release asset is:

```text
mobilecode-connect-mobile-android.aar
```

iOS release builds run on macOS and package:

- `aarch64-apple-ios`
- `aarch64-apple-ios-sim`
- `x86_64-apple-ios`

The iOS GitHub Release asset is:

```text
mobilecode-connect-mobile-ios-xcframework.zip
```

Each mobile package also uploads its generated `mobile-package-manifest.json`
under a stable release name. `sha256sums.txt` includes server and mobile release
assets.

## Embedded Web UI

`control-server` embeds `web/dist` at compile time. CI runs:

```sh
cd web
bun install --frozen-lockfile
bun run build
```

before compiling the Rust binary. The embedded app is served from `/`, `/admin`,
`/center`, `/login`, and `/assets/*`. If `web/dist` is absent during local
development, the control crate still compiles and `/admin` falls back to the
legacy `docs/control-admin.html` page.

## Local Build

Build the same app shape locally:

```sh
cd web
bun install --frozen-lockfile
bun run build
cd ..
cargo build --release -p control-server -p relayd
```

For a local x86_64 musl binary:

```sh
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl -p control-server -p relayd
```

The bare `control-server` binary uses the system `curl` executable for GitHub
OAuth. The Docker image includes `curl`; non-container deployments should install
it on the host.

Validate mobile packaging scripts without local Android or Xcode toolchains:

```sh
scripts/package-mobile-ios.sh \
  --dry-run \
  --targets aarch64-apple-ios,aarch64-apple-ios-sim,x86_64-apple-ios \
  --xcframework-output target/mobile-package-dry-run/ios/mobilecode_connect_mobile_coreFFI.xcframework

scripts/package-mobile-android.sh \
  --dry-run \
  --gradle-task assembleRelease \
  --aar-output-dir target/mobile-package-dry-run/android/aar
```

