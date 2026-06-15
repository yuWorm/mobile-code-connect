# Mobile Package CI/CD Design

## Scope

Add GitHub Actions release packaging for the mobile SDK packages:

- Android AAR
- iOS XCFramework

The mobile packages are published as GitHub Release assets on `v*` tags. Maven
Central, GitHub Packages Maven, CocoaPods, and remote SwiftPM binary publishing
are out of scope for this step.

## Existing Packaging Scripts

The workflow reuses the checked-in scripts:

- `scripts/package-mobile-android.sh`
- `scripts/package-mobile-ios.sh`

The scripts remain the source of truth for staging UniFFI bindings, native
libraries, manifests, and final platform package inputs.

## Android Release Job

The Android job runs on `ubuntu-latest` and installs:

- stable Rust
- Android SDK platform/build tools
- Android NDK
- Gradle
- `uniffi-bindgen` 0.31.1

It builds all default Android Rust targets:

- `aarch64-linux-android`
- `armv7-linux-androideabi`
- `x86_64-linux-android`
- `i686-linux-android`

The job runs `scripts/package-mobile-android.sh --gradle-task assembleRelease`
and uploads the release AAR plus the generated package manifest.

## iOS Release Job

The iOS job runs on `macos-latest` and installs:

- stable Rust
- `uniffi-bindgen` 0.31.1

It builds:

- `aarch64-apple-ios`
- `aarch64-apple-ios-sim`
- `x86_64-apple-ios`

The job runs `scripts/package-mobile-ios.sh` and zips the generated
`mobilecode_connect_mobile_coreFFI.xcframework` as a release asset, alongside
the generated package manifest.

## Validation

PRs and `master` pushes run dry-run validation for both mobile packaging scripts
without requiring Android or Xcode toolchains.

Release tags run the full mobile jobs and publish their artifacts into the same
GitHub Release as the server binaries.

