# QuicTunnelMobileSdk for iOS

This Swift package wraps the generated UniFFI module and the native Rust mobile
core artifact.

Expected release layout:

- `Package.swift`
- `Sources/QuicTunnelMobileSdk/QuicTunnelBrowserProxyController.swift`
- `Sources/QuicTunnelMobileSdk/QuicTunnelMobileGrantPairingController.swift`
- `Sources/QuicTunnelMobileSdk/QuicTunnelMobileGrantSecureStore.swift`
- `Sources/QuicTunnelMobileSdk/Generated/quic_tunnel_mobile_core.swift`
- `Artifacts/quic_tunnel_mobile_coreFFI.xcframework`

Run `scripts/package-mobile-ios.sh` from the repository root to build the Rust
static libraries, generate UniFFI Swift bindings, stage the generated Swift file,
and create the `quic_tunnel_mobile_coreFFI` XCFramework.
