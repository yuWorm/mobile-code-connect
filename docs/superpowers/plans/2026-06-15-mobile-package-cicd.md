# Mobile Package CI/CD Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish Android AAR and iOS XCFramework assets from GitHub Actions on `v*` tags.

**Architecture:** Reuse the existing mobile packaging scripts and add two release jobs to the current CI/CD workflow. The Android and iOS jobs upload artifacts that the existing release publishing job collects into one GitHub Release.

**Tech Stack:** GitHub Actions, Rust, UniFFI, Android SDK/NDK, Gradle, Xcode.

---

## Chunk 1: Workflow

### Task 1: Add Mobile Validation

**Files:**
- Modify: `.github/workflows/ci-cd.yml`

- [ ] Add a CI step that runs `bash -n` for both mobile packaging scripts.
- [ ] Add dry-run invocations for Android and iOS packaging.

### Task 2: Add Android Release Job

**Files:**
- Modify: `.github/workflows/ci-cd.yml`

- [ ] Add a tag-only `mobile-android` job.
- [ ] Install stable Rust, Android SDK/NDK, Gradle, Rust Android targets, and `uniffi-bindgen`.
- [ ] Run `scripts/package-mobile-android.sh --gradle-task assembleRelease`.
- [ ] Upload the AAR, Android manifest, and Android checksum file.

### Task 3: Add iOS Release Job

**Files:**
- Modify: `.github/workflows/ci-cd.yml`

- [ ] Add a tag-only `mobile-ios` job.
- [ ] Install stable Rust, Rust iOS targets, and `uniffi-bindgen`.
- [ ] Run `scripts/package-mobile-ios.sh` with device and simulator targets.
- [ ] Zip the XCFramework and upload it with the iOS manifest and checksum file.

### Task 4: Publish Release

**Files:**
- Modify: `.github/workflows/ci-cd.yml`

- [ ] Make `publish-release` depend on server and mobile release jobs.
- [ ] Download all `release-*` artifacts and merge all checksum files.

## Chunk 2: Documentation

### Task 5: Update Release Docs

**Files:**
- Modify: `docs/release.md`

- [ ] Document Android and iOS release assets.
- [ ] Document local dry-run and full package commands.

## Chunk 3: Verification

### Task 6: Verify

**Files:**
- Read: `.github/workflows/ci-cd.yml`
- Read: `docs/release.md`

- [ ] Run mobile packaging dry-runs.
- [ ] Parse workflow YAML.
- [ ] Run `git diff --check`.

