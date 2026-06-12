# MobileCode Connect Crate Rename Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rename the Rust crate namespace from `quic_tunnel_*` to `mobilecode_connect_*` while preserving a controlled migration path for binaries, generated mobile bindings, scripts, and docs.

**Architecture:** Treat crate renaming as a compatibility-sensitive API migration, not a global string replace. First rename library package names and Rust imports while keeping binary names and environment variables stable, then update mobile FFI/package artifacts, then add or document compatibility aliases for operational surfaces.

**Tech Stack:** Rust 2021 workspace, Cargo package/dependency aliases, Clap binaries, UniFFI 0.31.1, SwiftPM, Android Gradle/Kotlin, Bash scripts, Vue admin console docs/tests.

---

## File Map

- Modify: `Cargo.toml` workspace members and package references only if paths change.
- Modify: `crates/*/Cargo.toml` package names and dependency keys.
- Modify: all Rust imports under `apps/`, `crates/`, and tests from `quic_tunnel_*` to `mobilecode_connect_*`.
- Modify: `scripts/gen-mobile-bindings.sh`, `scripts/package-mobile-ios.sh`, `scripts/package-mobile-android.sh`, `scripts/production-check.sh` for crate/package artifact names.
- Modify: `mobile/ios/Package.swift`, `mobile/ios/README.md`, `mobile/ios/Artifacts/README.md`, and generated-artifact tests for the renamed UniFFI library.
- Modify: `mobile/android/build.gradle.kts`, `mobile/android/settings.gradle.kts`, `mobile/android/README.md`, `mobile/android/consumer-rules.pro`, and Kotlin imports when the UniFFI package changes.
- Modify: docs and static contract tests that assert crate names, especially `crates/mobile-core/tests/mobile_platform_wrappers.rs`, `crates/mobile-core/tests/smoke_script.rs`, and `crates/sdk/tests/examples_docs.rs`.
- Do not modify in the first crate rename task: binary names (`mobile-cli`, `agentd`, `relayd`, `control-server`, `punch-server`), environment variables (`QUIC_TUNNEL_*`, `QUIC_TEST_*`), or `.qtunnel.local`. Plan those as separate compatibility migrations after crate rename lands.

## Target Names

- `quic_tunnel_protocol` -> `mobilecode_connect_protocol`
- `quic_tunnel_auth` -> `mobilecode_connect_auth`
- `quic_tunnel_tunnel` -> `mobilecode_connect_tunnel`
- `quic_tunnel_agent` -> `mobilecode_connect_agent`
- `quic_tunnel_mobile_core` -> `mobilecode_connect_mobile_core`
- `quic_tunnel_relay` -> `mobilecode_connect_relay`
- `quic_tunnel_control` -> `mobilecode_connect_control`
- `quic_tunnel_control_client` -> `mobilecode_connect_control_client`
- `quic_tunnel_sdk` -> `mobilecode_connect_sdk`
- `quic_tunnel_punch` -> `mobilecode_connect_punch`

## Task 1: Rename Cargo Package Names

**Files:**
- Modify: `crates/protocol/Cargo.toml`
- Modify: `crates/auth/Cargo.toml`
- Modify: `crates/tunnel/Cargo.toml`
- Modify: `crates/agent/Cargo.toml`
- Modify: `crates/mobile-core/Cargo.toml`
- Modify: `crates/relay/Cargo.toml`
- Modify: `crates/control/Cargo.toml`
- Modify: `crates/control-client/Cargo.toml`
- Modify: `crates/sdk/Cargo.toml`
- Modify: `crates/punch/Cargo.toml`
- Modify: `apps/*/Cargo.toml`
- Modify: `Cargo.lock`

- [ ] **Step 1: Capture current package graph**

Run:

```bash
cargo metadata --format-version 1 > /tmp/mobilecode-connect-before-metadata.json
```

Expected: command exits 0.

- [ ] **Step 2: Change package names in library crate manifests**

Update only `[package] name = ...` values using the target name table. Keep crate directory paths unchanged.

- [ ] **Step 3: Update dependency keys in all manifests**

Replace manifest dependency keys and path dependency references, for example:

```toml
mobilecode_connect_protocol = { path = "../protocol" }
mobilecode_connect_mobile_core = { path = "../mobile-core" }
```

For app manifests under `apps/*`, use `../../crates/...` paths as they do now.

- [ ] **Step 4: Verify metadata resolves**

Run:

```bash
cargo metadata --format-version 1
```

Expected: exits 0 and lists `mobilecode_connect_*` package names.

- [ ] **Step 5: Commit package manifest rename**

```bash
git add Cargo.toml Cargo.lock crates apps
git commit -m "chore: rename Cargo packages to MobileCode Connect namespace"
```

## Task 2: Rename Rust Imports and Package References

**Files:**
- Modify: `apps/**/*.rs`
- Modify: `crates/**/*.rs`
- Modify: `crates/**/tests/**/*.rs`
- Modify: `crates/**/examples/**/*.rs`

- [ ] **Step 1: Replace Rust import paths**

Use a mechanical rename from old crate identifiers to new identifiers across Rust files only. Do not alter strings or docs in this step.

- [ ] **Step 2: Run formatter**

Run:

```bash
cargo fmt --check
```

Expected: pass. If it fails, run `cargo fmt`, inspect the diff, and rerun `cargo fmt --check`.

- [ ] **Step 3: Compile the renamed workspace**

Run:

```bash
cargo check --workspace
```

Expected: pass.

- [ ] **Step 4: Commit Rust import rename**

```bash
git add apps crates
git commit -m "chore: update Rust imports for MobileCode Connect crates"
```

## Task 3: Update SDK Examples and Documentation Contracts

**Files:**
- Modify: `README.md`
- Modify: `docs/production-readiness.md`
- Modify: `docs/mobile-device-acceptance.md`
- Modify: `crates/sdk/tests/examples_docs.rs`
- Modify: `crates/mobile-core/tests/smoke_script.rs`

- [ ] **Step 1: Update documented cargo commands**

Change example commands such as:

```bash
cargo run -p quic_tunnel_sdk --example sdk_mock_workflow
```

to:

```bash
cargo run -p mobilecode_connect_sdk --example sdk_mock_workflow
```

- [ ] **Step 2: Update static contract tests**

Update tests that assert documented crate names and production check commands.

- [ ] **Step 3: Run documentation contract tests**

Run:

```bash
cargo test -p mobilecode_connect_sdk --test examples_docs
cargo test -p mobilecode_connect_mobile_core --test smoke_script
```

Expected: both pass.

- [ ] **Step 4: Commit docs and contract tests**

```bash
git add README.md docs crates/sdk/tests/examples_docs.rs crates/mobile-core/tests/smoke_script.rs
git commit -m "docs: update crate references for MobileCode Connect"
```

## Task 4: Rename UniFFI Mobile Core Artifacts

**Files:**
- Modify: `scripts/gen-mobile-bindings.sh`
- Modify: `scripts/package-mobile-ios.sh`
- Modify: `scripts/package-mobile-android.sh`
- Modify: `scripts/production-check.sh`
- Modify: `mobile/ios/Package.swift`
- Modify: `mobile/ios/README.md`
- Modify: `mobile/ios/Artifacts/README.md`
- Modify: `mobile/android/README.md`
- Modify: `mobile/android/consumer-rules.pro`
- Modify: `crates/mobile-core/tests/mobile_platform_wrappers.rs`

- [ ] **Step 1: Update generated artifact names**

Rename generated library and binding references:

```text
quic_tunnel_mobile_core -> mobilecode_connect_mobile_core
quic_tunnel_mobile_coreFFI -> mobilecode_connect_mobile_coreFFI
uniffi.quic_tunnel_mobile_core -> uniffi.mobilecode_connect_mobile_core
```

- [ ] **Step 2: Update iOS package binary target**

Update `mobile/ios/Package.swift` binary target and expected XCFramework path to `mobilecode_connect_mobile_coreFFI.xcframework`.

- [ ] **Step 3: Update Android consumer rules and README**

Change keep rules and docs to the new UniFFI package namespace.

- [ ] **Step 4: Run mobile packaging dry runs**

Run:

```bash
scripts/gen-mobile-bindings.sh --language all
scripts/package-mobile-ios.sh --dry-run --ios-min-version 17.0 --targets aarch64-apple-ios,aarch64-apple-ios-sim,x86_64-apple-ios --xcframework-output target/mobile-package-dry-run/ios/mobilecode_connect_mobile_coreFFI.xcframework
scripts/package-mobile-android.sh --dry-run --gradle-task assembleRelease --aar-output-dir target/mobile-package-dry-run/android/aar
cargo test -p mobilecode_connect_mobile_core --test mobile_platform_wrappers
```

Expected: all pass.

- [ ] **Step 5: Commit mobile artifact rename**

```bash
git add scripts mobile crates/mobile-core/tests/mobile_platform_wrappers.rs
git commit -m "chore: rename MobileCode Connect mobile FFI artifacts"
```

## Task 5: Full Verification Gate

**Files:**
- No new edits unless verification exposes missed references.

- [ ] **Step 1: Search for stale crate names**

Run:

```bash
rg -n "quic_tunnel_(protocol|auth|tunnel|agent|mobile_core|relay|control|control_client|sdk|punch)" apps crates mobile scripts README.md docs
```

Expected: no matches except migration notes intentionally kept in this plan or compatibility docs.

- [ ] **Step 2: Run workspace checks**

Run:

```bash
cargo fmt --check
cargo test --workspace --no-run
```

Expected: both pass.

- [ ] **Step 3: Run production check without optional runtime gates**

Run:

```bash
scripts/production-check.sh
```

Expected: pass, with optional real packaging/e2e gates skipped unless their env flags are enabled.

- [ ] **Step 4: Final commit for missed cleanup only**

If verification required follow-up edits:

```bash
git add <changed-files>
git commit -m "chore: finish MobileCode Connect crate rename"
```

## Follow-Up Plan After Crates

Create a separate migration plan for:

- binary/package names: `mobile-cli`, `agentd`, `relayd`, `control-server`, `punch-server`
- environment variable aliases: `MOBILECODE_CONNECT_*` while preserving `QUIC_TUNNEL_*`
- dev/test env aliases: `MOBILECODE_CONNECT_TEST_*` while preserving `QUIC_TEST_*`
- synthetic browser proxy domain: `.mobilecode-connect.local` while preserving `.qtunnel.local`
- public SDK names: `QuicTunnelMobileSdk` to a MobileCode Connect SDK name with platform deprecation notes
