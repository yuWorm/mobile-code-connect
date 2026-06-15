use std::path::Path;
use std::process::Command;

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
}

fn read_workspace_file(relative: &str) -> String {
    let path = workspace_root().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("{} should be readable: {err}", path.display()))
}

fn assert_contains_all(content: &str, expected: &[&str]) {
    for needle in expected {
        assert!(content.contains(needle), "missing {needle}");
    }
}

fn assert_executable(relative: &str) {
    let path = workspace_root().join(relative);
    let metadata = std::fs::metadata(&path)
        .unwrap_or_else(|err| panic!("{} should exist: {err}", path.display()));
    assert!(metadata.is_file(), "{} should be a file", path.display());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        assert_ne!(
            metadata.permissions().mode() & 0o111,
            0,
            "{} should be executable",
            path.display()
        );
    }
}

fn run_workspace_command(args: &[&str]) -> String {
    let mut command = Command::new(args[0]);
    command.args(&args[1..]).current_dir(workspace_root());

    let output = command
        .output()
        .unwrap_or_else(|err| panic!("{} should run: {err}", args.join(" ")));

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "{} failed with status {:?}\nstdout:\n{}\nstderr:\n{}",
        args.join(" "),
        output.status.code(),
        stdout,
        stderr
    );

    format!("{stdout}\n{stderr}")
}

#[test]
fn ios_browser_proxy_wrapper_documents_webview_lifecycle() {
    let browser_proxy = read_workspace_file(
        "mobile/ios/Sources/MobileCodeConnectMobileSdk/MobileCodeConnectBrowserProxyController.swift",
    );
    let pairing = read_workspace_file(
        "mobile/ios/Sources/MobileCodeConnectMobileSdk/MobileCodeConnectMobileGrantPairingController.swift",
    );
    let secure_store = read_workspace_file(
        "mobile/ios/Sources/MobileCodeConnectMobileSdk/MobileCodeConnectMobileGrantSecureStore.swift",
    );
    let source = format!("{browser_proxy}\n{pairing}\n{secure_store}");

    assert_contains_all(
        &source,
        &[
            "import WebKit",
            "import Network",
            "FfiMobileTunnel",
            "FfiBrowserProxy",
            "FfiBrowserProxyDirectFallbackPolicy",
            "browserProxyConfigWithDefaults",
            "makeBrowserProxyConfig",
            "bindHost",
            "localPort",
            "domainSuffix",
            "maxConnections",
            "directFallbackPolicy",
            ".localNetworkAndDomain",
            "requestHeadTimeoutMs",
            "directConnectTimeoutMs",
            "tunnelOpenTimeoutMs",
            "idleTimeoutMs",
            "MobileCodeConnectMobileGrantPairingController",
            "MobileCodeConnectMobileGrantSecureStore",
            "SecItemAdd",
            "SecItemCopyMatching",
            "SecItemDelete",
            "mobileGrantCredentialToJson",
            "mobileGrantCredentialFromJson",
            "startMobileGrantPairing",
            "pollMobileGrantPairingOnce",
            "mobileGrantPairingOptionsWithDefaults",
            "browserProxyDeviceServiceRoute",
            "browserProxyRouteHttpUrl",
            "browserProxyClassifyUrlWithDefaults",
            "startBrowserProxyWithConfig",
            "stats",
            "applyProxy",
            "WKWebViewConfiguration",
            "ProxyConfiguration",
            "closeBrowserProxy",
            "try proxy.shutdown()",
            "shutdown",
        ],
    );
}

#[test]
fn android_browser_proxy_wrapper_documents_webview_lifecycle() {
    let browser_proxy = read_workspace_file(
        "mobile/android/src/main/java/dev/mobilecode/connect/mobile/MobileCodeConnectBrowserProxyController.kt",
    );
    let pairing = read_workspace_file(
        "mobile/android/src/main/java/dev/mobilecode/connect/mobile/MobileCodeConnectMobileGrantPairingController.kt",
    );
    let secure_store = read_workspace_file(
        "mobile/android/src/main/java/dev/mobilecode/connect/mobile/MobileCodeConnectMobileGrantSecureStore.kt",
    );
    let source = format!("{browser_proxy}\n{pairing}\n{secure_store}");

    assert_contains_all(
        &source,
        &[
            "androidx.webkit.ProxyConfig",
            "androidx.webkit.ProxyController",
            "androidx.webkit.WebViewFeature",
            "FfiMobileTunnel",
            "FfiBrowserProxy",
            "FfiBrowserProxyDirectFallbackPolicy",
            "browserProxyConfigWithDefaults",
            "makeBrowserProxyConfig",
            "bindHost",
            "localPort",
            "domainSuffix",
            "maxConnections",
            "directFallbackPolicy",
            "LOCAL_NETWORK_AND_DOMAIN",
            "requestHeadTimeoutMs",
            "directConnectTimeoutMs",
            "tunnelOpenTimeoutMs",
            "idleTimeoutMs",
            "MobileCodeConnectMobileGrantPairingController",
            "MobileCodeConnectMobileGrantSecureStore",
            "AndroidKeyStore",
            "KeyGenParameterSpec",
            "AES/GCM/NoPadding",
            "mobileGrantCredentialToJson",
            "mobileGrantCredentialFromJson",
            "startMobileGrantPairing",
            "pollMobileGrantPairingOnce",
            "mobileGrantPairingOptionsWithDefaults",
            "browserProxyDeviceServiceRoute",
            "browserProxyRouteHttpUrl",
            "browserProxyClassifyUrlWithDefaults",
            "startBrowserProxyWithConfig",
            "stats",
            "applyProxy",
            "ProxyController.getInstance().setProxyOverride",
            "clearProxyOverride",
            "closeBrowserProxy",
            "proxy.shutdown()",
            "shutdown",
        ],
    );
}

#[test]
fn readme_points_to_platform_browser_proxy_wrappers() {
    let readme = read_workspace_file("README.md");

    assert_contains_all(
        &readme,
        &[
            "mobile/ios/Sources/MobileCodeConnectMobileSdk/MobileCodeConnectBrowserProxyController.swift",
            "mobile/ios/Sources/MobileCodeConnectMobileSdk/MobileCodeConnectMobileGrantPairingController.swift",
            "mobile/ios/Sources/MobileCodeConnectMobileSdk/MobileCodeConnectMobileGrantSecureStore.swift",
            "mobile/android/src/main/java/dev/mobilecode/connect/mobile/MobileCodeConnectBrowserProxyController.kt",
            "mobile/android/src/main/java/dev/mobilecode/connect/mobile/MobileCodeConnectMobileGrantPairingController.kt",
            "mobile/android/src/main/java/dev/mobilecode/connect/mobile/MobileCodeConnectMobileGrantSecureStore.kt",
        ],
    );
}

#[test]
fn ios_swift_package_declares_sdk_and_native_core_artifact() {
    let manifest = read_workspace_file("mobile/ios/Package.swift");

    assert_contains_all(
        &manifest,
        &[
            "swift-tools-version: 5.9",
            "MobileCodeConnectMobileSdk",
            ".iOS(.v17)",
            ".macOS(.v14)",
            ".library(",
            ".binaryTarget(",
            "mobilecode_connect_mobile_coreFFI",
            "Artifacts/mobilecode_connect_mobile_coreFFI.xcframework",
            "Sources/MobileCodeConnectMobileSdk",
        ],
    );
}

#[test]
fn android_gradle_library_declares_webview_proxy_sdk_inputs() {
    let settings = read_workspace_file("mobile/android/settings.gradle.kts");
    let build = read_workspace_file("mobile/android/build.gradle.kts");
    let manifest = read_workspace_file("mobile/android/src/main/AndroidManifest.xml");
    let consumer_rules = read_workspace_file("mobile/android/consumer-rules.pro");
    let combined = format!("{settings}\n{build}\n{manifest}\n{consumer_rules}");

    assert_contains_all(
        &combined,
        &[
            "rootProject.name = \"MobileCodeConnectMobileSdk\"",
            "com.android.library",
            "namespace = \"dev.mobilecode.connect.mobile\"",
            "compileSdk = 36",
            "minSdk = 23",
            "java.srcDirs(\"src/main/java\")",
            "src/main/jniLibs",
            "consumer-rules.pro",
            "androidx.webkit:webkit:1.14.0",
            "net.java.dev.jna:jna:5.17.0@aar",
            "uniffi.mobilecode_connect_mobile_core",
            "dev.mobilecode.connect.mobile",
            "<manifest",
        ],
    );
    assert!(
        !combined.contains("org.jetbrains.kotlin.android"),
        "AGP 9 provides Kotlin support without the deprecated Kotlin Android plugin"
    );
}

#[test]
fn readme_points_to_platform_package_manifests() {
    let readme = read_workspace_file("README.md");

    assert_contains_all(
        &readme,
        &[
            "mobile/ios/Package.swift",
            "mobile/android/build.gradle.kts",
            "mobile/android/settings.gradle.kts",
            "Artifacts/mobilecode_connect_mobile_coreFFI.xcframework",
            "src/main/java/uniffi",
            "src/main/jniLibs",
        ],
    );
}

#[test]
fn mobile_packaging_scripts_stage_native_artifacts_and_bindings() {
    assert_executable("scripts/package-mobile-ios.sh");
    assert_executable("scripts/package-mobile-android.sh");

    let ios = read_workspace_file("scripts/package-mobile-ios.sh");
    assert_contains_all(
        &ios,
        &[
            "aarch64-apple-ios",
            "aarch64-apple-ios-sim",
            "x86_64-apple-ios",
            "--ios-min-version",
            "--xcframework-output",
            "IPHONEOS_DEPLOYMENT_TARGET",
            "lipo -create",
            "libmobilecode_connect_mobile_core_simulator.a",
            "mobile-package-manifest.json",
            "sha256_file",
            "cargo build -p mobilecode_connect_mobile_core --release --target",
            "scripts/gen-mobile-bindings.sh --language swift",
            "mobilecode_connect_mobile_core.swift",
            "mobilecode_connect_mobile_coreFFI.h",
            "module.modulemap",
            "xcodebuild -create-xcframework",
            "Artifacts/mobilecode_connect_mobile_coreFFI.xcframework",
            "Sources/MobileCodeConnectMobileSdk/Generated",
        ],
    );

    let android = read_workspace_file("scripts/package-mobile-android.sh");
    assert_contains_all(
        &android,
        &[
            "aarch64-linux-android",
            "armv7-linux-androideabi",
            "x86_64-linux-android",
            "i686-linux-android",
            "arm64-v8a",
            "armeabi-v7a",
            "cargo build -p mobilecode_connect_mobile_core --release --target",
            "scripts/gen-mobile-bindings.sh --language kotlin",
            "KOTLIN_DEST=\"$ANDROID_DIR/src/main/java\"",
            "src/main/jniLibs",
            "libmobilecode_connect_mobile_core.so",
            "--gradle-task",
            "--aar-output-dir",
            "--no-strip",
            "GRADLE_TASK",
            "llvm-strip",
            "mobile-package-manifest.json",
            "sha256_file",
            "assembleRelease",
        ],
    );
}

#[test]
fn android_packaging_script_configures_ndk_linkers() {
    let android = read_workspace_file("scripts/package-mobile-android.sh");

    assert_contains_all(
        &android,
        &[
            "--ndk-home",
            "--ndk-host-tag",
            "--android-api",
            "ANDROID_NDK_HOME",
            "ANDROID_NDK_ROOT",
            "ANDROID_NDK_HOST_TAG",
            "CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER",
            "CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_LINKER",
            "CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER",
            "CARGO_TARGET_I686_LINUX_ANDROID_LINKER",
            "aarch64-linux-android${ANDROID_API}-clang",
            "armv7a-linux-androideabi${ANDROID_API}-clang",
            "x86_64-linux-android${ANDROID_API}-clang",
            "i686-linux-android${ANDROID_API}-clang",
            "llvm-ar",
            "llvm-ranlib",
        ],
    );
}

#[test]
fn ios_packaging_script_dry_run_plans_deployment_target_and_simulator_fat_library() {
    let ios = run_workspace_command(&[
        "scripts/package-mobile-ios.sh",
        "--dry-run",
        "--ios-min-version",
        "17.0",
        "--targets",
        "aarch64-apple-ios,aarch64-apple-ios-sim,x86_64-apple-ios",
        "--staging-dir",
        "target/mobile-package-dry-run/ios",
    ]);

    assert_contains_all(
        &ios,
        &[
            "IPHONEOS_DEPLOYMENT_TARGET=17.0",
            "cargo build -p mobilecode_connect_mobile_core --release --target aarch64-apple-ios",
            "cargo build -p mobilecode_connect_mobile_core --release --target aarch64-apple-ios-sim",
            "cargo build -p mobilecode_connect_mobile_core --release --target x86_64-apple-ios",
            "lipo -create",
            "target/aarch64-apple-ios-sim/release/libmobilecode_connect_mobile_core.a",
            "target/x86_64-apple-ios/release/libmobilecode_connect_mobile_core.a",
            "libmobilecode_connect_mobile_core_simulator.a",
            "xcodebuild -create-xcframework",
            "target/aarch64-apple-ios/release/libmobilecode_connect_mobile_core.a",
            "target/mobile-package-dry-run/ios/libmobilecode_connect_mobile_core_simulator.a",
        ],
    );
}

#[test]
fn android_packaging_script_dry_run_uses_configurable_gradle_task() {
    let android = run_workspace_command(&[
        "scripts/package-mobile-android.sh",
        "--dry-run",
        "--ndk-home",
        "/opt/android-ndk",
        "--ndk-host-tag",
        "linux-x86_64",
        "--android-api",
        "24",
        "--gradle-task",
        "assembleDebug",
        "--targets",
        "aarch64-linux-android",
        "--staging-dir",
        "target/mobile-package-dry-run/android",
    ]);

    assert_contains_all(
        &android,
        &["GRADLE_TASK=assembleDebug", "dry-run: assembleDebug from"],
    );
}

#[test]
fn ios_packaging_script_dry_run_plans_custom_xcframework_output_and_manifest() {
    let ios = run_workspace_command(&[
        "scripts/package-mobile-ios.sh",
        "--dry-run",
        "--xcframework-output",
        "target/mobile-package-dry-run/ios/MobileCodeConnectMobileCoreFFI.xcframework",
        "--targets",
        "aarch64-apple-ios",
        "--staging-dir",
        "target/mobile-package-dry-run/ios",
    ]);

    assert_contains_all(
        &ios,
        &[
            "XCFRAMEWORK_OUTPUT=target/mobile-package-dry-run/ios/MobileCodeConnectMobileCoreFFI.xcframework",
            "xcodebuild -create-xcframework",
            "MobileCodeConnectMobileCoreFFI.xcframework",
            "dry-run: write target/mobile-package-dry-run/ios/mobile-package-manifest.json with sha256 entries",
        ],
    );
}

#[test]
fn android_packaging_script_dry_run_plans_strip_aar_archive_and_manifest() {
    let android = run_workspace_command(&[
        "scripts/package-mobile-android.sh",
        "--dry-run",
        "--ndk-home",
        "/opt/android-ndk",
        "--ndk-host-tag",
        "linux-x86_64",
        "--android-api",
        "24",
        "--aar-output-dir",
        "target/mobile-package-dry-run/android/aar",
        "--targets",
        "aarch64-linux-android",
        "--staging-dir",
        "target/mobile-package-dry-run/android",
    ]);

    assert_contains_all(
        &android,
        &[
            "STRIP_NATIVE_LIBS=1",
            "llvm-strip",
            "dry-run: cp",
            "target/mobile-package-dry-run/android/aar",
            "dry-run: write target/mobile-package-dry-run/android/mobile-package-manifest.json with sha256 entries",
        ],
    );
}

#[test]
fn readme_documents_mobile_packaging_scripts() {
    let readme = read_workspace_file("README.md");

    assert_contains_all(
        &readme,
        &[
            "scripts/package-mobile-ios.sh",
            "scripts/package-mobile-android.sh",
            "--dry-run",
            "--ios-min-version",
            "--gradle-task",
            "--xcframework-output",
            "--aar-output-dir",
            "--no-strip",
            "lipo",
            "mobile-package-manifest.json",
            "directFallbackPolicy",
            "LocalNetworkAndDomain",
            "FfiBrowserProxy.stats()",
            "mobilecode_connect_mobile_coreFFI.xcframework",
            "assembleRelease",
        ],
    );
}

#[test]
fn mobile_packaging_scripts_support_toolchain_free_dry_run() {
    let ios = run_workspace_command(&[
        "scripts/package-mobile-ios.sh",
        "--dry-run",
        "--ios-min-version",
        "17.0",
        "--targets",
        "aarch64-apple-ios",
        "--staging-dir",
        "target/mobile-package-dry-run/ios",
    ]);
    assert_contains_all(
        &ios,
        &[
            "dry-run",
            "IPHONEOS_DEPLOYMENT_TARGET=17.0",
            "cargo build -p mobilecode_connect_mobile_core --release --target aarch64-apple-ios",
            "scripts/gen-mobile-bindings.sh --language swift",
            "mobilecode_connect_mobile_core.swift",
            "xcodebuild -create-xcframework",
            "Artifacts/mobilecode_connect_mobile_coreFFI.xcframework",
        ],
    );

    let android = run_workspace_command(&[
        "scripts/package-mobile-android.sh",
        "--dry-run",
        "--ndk-home",
        "/opt/android-ndk",
        "--ndk-host-tag",
        "linux-x86_64",
        "--android-api",
        "24",
        "--gradle-task",
        "assembleRelease",
        "--targets",
        "aarch64-linux-android,x86_64-linux-android",
        "--staging-dir",
        "target/mobile-package-dry-run/android",
    ]);
    assert_contains_all(
        &android,
        &[
            "dry-run",
            "cargo build -p mobilecode_connect_mobile_core --release --target aarch64-linux-android",
            "cargo build -p mobilecode_connect_mobile_core --release --target x86_64-linux-android",
            "scripts/gen-mobile-bindings.sh --language kotlin",
            "ANDROID_NDK_HOME=/opt/android-ndk",
            "CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER=/opt/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android24-clang",
            "CARGO_TARGET_X86_64_LINUX_ANDROID_LINKER=/opt/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/x86_64-linux-android24-clang",
            "CC_aarch64_linux_android=/opt/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android24-clang",
            "AR_aarch64_linux_android=/opt/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ar",
            "RANLIB_aarch64_linux_android=/opt/android-ndk/toolchains/llvm/prebuilt/linux-x86_64/bin/llvm-ranlib",
            "arm64-v8a",
            "x86_64",
            "libmobilecode_connect_mobile_core.so",
            "GRADLE_TASK=assembleRelease",
            "assembleRelease",
        ],
    );
}
