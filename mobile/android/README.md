# QuicTunnelMobileSdk for Android

This Gradle project builds the Android library wrapper around the generated
UniFFI Kotlin bindings and native Rust shared libraries.

Expected release layout:

- `src/main/java/dev/quictunnel/mobile/QuicTunnelBrowserProxyController.kt`
- `src/main/java/dev/quictunnel/mobile/QuicTunnelMobileGrantPairingController.kt`
- `src/main/java/dev/quictunnel/mobile/QuicTunnelMobileGrantSecureStore.kt`
- `src/main/uniffi/kotlin/uniffi/quic_tunnel_mobile_core/...`
- `src/main/jniLibs/<abi>/libquic_tunnel_mobile_core.so`

Run `scripts/package-mobile-android.sh` from the repository root to build the
Rust shared libraries, generate UniFFI Kotlin bindings, copy Android native
libraries into `src/main/jniLibs`, and run:

```bash
./gradlew :assembleRelease
```
