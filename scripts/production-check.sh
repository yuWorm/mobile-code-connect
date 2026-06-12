#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

log() {
  printf '[production-check] %s\n' "$*"
}

die() {
  log "$*"
  exit 1
}

run() {
  log "running: $*"
  "$@"
}

enabled() {
  local value="${1:-0}"
  [[ "$value" == "1" || "$value" == "true" || "$value" == "yes" ]]
}

check_runtime_env() {
  if ! enabled "${QUIC_PROD_CHECK_REQUIRE_RUNTIME_ENV:-0}"; then
    log "runtime env gate skipped; set QUIC_PROD_CHECK_REQUIRE_RUNTIME_ENV=1 to enforce production env checks"
    return 0
  fi

  [[ "${QUIC_TUNNEL_STRICT_AUTH:-}" == "true" ]] || \
    die "QUIC_TUNNEL_STRICT_AUTH=true is required for production"

  [[ -n "${QUIC_TEST_TOKEN_SECRET:-}" ]] || \
    die "QUIC_TEST_TOKEN_SECRET must be set to the production token secret"
  [[ "$QUIC_TEST_TOKEN_SECRET" != "dev-secret" ]] || \
    die "QUIC_TEST_TOKEN_SECRET must not be dev-secret in production"

  log "runtime env gate passed"
}

log "release checklist: docs/production-readiness.md"
check_runtime_env

run bash -n scripts/dev-stack.sh
run bash -n scripts/e2e-smoke.sh
run bash -n scripts/package-mobile-ios.sh
run bash -n scripts/package-mobile-android.sh
run bash -n scripts/install-relayd.sh
run bash -n scripts/production-check.sh
run scripts/gen-mobile-bindings.sh --language all
run scripts/package-mobile-ios.sh --dry-run --ios-min-version 17.0 --targets aarch64-apple-ios,aarch64-apple-ios-sim,x86_64-apple-ios --xcframework-output target/mobile-package-dry-run/ios/quic_tunnel_mobile_coreFFI.xcframework
run scripts/package-mobile-android.sh --dry-run --gradle-task assembleRelease --aar-output-dir target/mobile-package-dry-run/android/aar

run cargo fmt --check
run cargo test -p quic_tunnel_mobile_core --lib mobile_grant_
run cargo test -p quic_tunnel_mobile_core --test mobile_platform_wrappers
run cargo test -p quic_tunnel_sdk --test live_workflow
run cargo test -p quic_tunnel_mobile_core --test smoke_script
run cargo test -p mobile-cli
run cargo test --workspace --no-run

if enabled "${QUIC_PROD_CHECK_MOBILE_PACKAGE:-0}" || enabled "${QUIC_PROD_CHECK_IOS_PACKAGE:-0}"; then
  run scripts/package-mobile-ios.sh --ios-min-version 17.0 --targets aarch64-apple-ios,aarch64-apple-ios-sim,x86_64-apple-ios
else
  log "real iOS packaging skipped; set QUIC_PROD_CHECK_MOBILE_PACKAGE=1 or QUIC_PROD_CHECK_IOS_PACKAGE=1 to run scripts/package-mobile-ios.sh"
fi

if enabled "${QUIC_PROD_CHECK_MOBILE_PACKAGE:-0}" || enabled "${QUIC_PROD_CHECK_ANDROID_PACKAGE:-0}"; then
  run scripts/package-mobile-android.sh --gradle-task assembleRelease
else
  log "real Android packaging skipped; set QUIC_PROD_CHECK_MOBILE_PACKAGE=1 or QUIC_PROD_CHECK_ANDROID_PACKAGE=1 to run scripts/package-mobile-android.sh"
fi

if enabled "${QUIC_PROD_CHECK_DEVICE_SIGNOFF:-0}"; then
  signoff_file="${QUIC_PROD_CHECK_DEVICE_SIGNOFF_FILE:-docs/mobile-device-acceptance-signoff.md}"
  [[ -s "$signoff_file" ]] || die "mobile device signoff file is required: $signoff_file; use docs/mobile-device-acceptance.md"
  for required in iOS Android WebView P2P Relay revoke LocalNetworkAndDomain; do
    grep -q "$required" "$signoff_file" || die "mobile device signoff missing required evidence marker: $required"
  done
  log "mobile device signoff gate passed: $signoff_file"
else
  log "mobile device signoff skipped; complete docs/mobile-device-acceptance.md and set QUIC_PROD_CHECK_DEVICE_SIGNOFF=1 to enforce it"
fi

if enabled "${QUIC_PROD_CHECK_FULL:-0}"; then
  run cargo test --workspace
else
  log "full workspace runtime tests skipped; set QUIC_PROD_CHECK_FULL=1 to run cargo test --workspace"
fi

if enabled "${QUIC_PROD_CHECK_E2E:-0}"; then
  run ./scripts/e2e-smoke.sh
else
  log "socket-binding E2E smoke skipped; set QUIC_PROD_CHECK_E2E=1 to run ./scripts/e2e-smoke.sh"
fi

log "production readiness gate completed; review docs/production-readiness.md before release"
