# MobileCodeConnectMobileSdk for iOS

This Swift package wraps the generated UniFFI module and the native Rust mobile
core artifact.

Expected release layout:

- `Package.swift`
- `Sources/MobileCodeConnectMobileSdk/MobileCodeConnectBrowserProxyController.swift`
- `Sources/MobileCodeConnectMobileSdk/MobileCodeConnectMobileGrantPairingController.swift`
- `Sources/MobileCodeConnectMobileSdk/MobileCodeConnectMobileGrantSecureStore.swift`
- `Sources/MobileCodeConnectMobileSdk/Generated/mobilecode_connect_mobile_core.swift`
- `Artifacts/mobilecode_connect_mobile_coreFFI.xcframework`

Run `scripts/package-mobile-ios.sh` from the repository root to build the Rust
static libraries, generate UniFFI Swift bindings, stage the generated Swift file,
and create the `mobilecode_connect_mobile_coreFFI` XCFramework.
