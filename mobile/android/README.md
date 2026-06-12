# MobileCodeConnectMobileSdk for Android

This Gradle project builds the Android library wrapper around the generated
UniFFI Kotlin bindings and native Rust shared libraries.

Expected release layout:

- `src/main/java/dev/mobilecode/connect/mobile/MobileCodeConnectBrowserProxyController.kt`
- `src/main/java/dev/mobilecode/connect/mobile/MobileCodeConnectMobileGrantPairingController.kt`
- `src/main/java/dev/mobilecode/connect/mobile/MobileCodeConnectMobileGrantSecureStore.kt`
- `src/main/uniffi/kotlin/uniffi/mobilecode_connect_mobile_core/...`
- `src/main/jniLibs/<abi>/libmobilecode_connect_mobile_core.so`

Run `scripts/package-mobile-android.sh` from the repository root to build the
Rust shared libraries, generate UniFFI Kotlin bindings, copy Android native
libraries into `src/main/jniLibs`, and run:

```bash
./gradlew :assembleRelease
```
