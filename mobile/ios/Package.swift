// swift-tools-version: 5.9

import PackageDescription

let package = Package(
    name: "MobileCodeConnectMobileSdk",
    platforms: [
        .iOS(.v17),
        .macOS(.v14),
    ],
    products: [
        .library(
            name: "MobileCodeConnectMobileSdk",
            targets: ["MobileCodeConnectMobileSdk"]
        ),
    ],
    targets: [
        .binaryTarget(
            name: "mobilecode_connect_mobile_coreFFI",
            path: "Artifacts/mobilecode_connect_mobile_coreFFI.xcframework"
        ),
        .target(
            name: "MobileCodeConnectMobileSdk",
            dependencies: ["mobilecode_connect_mobile_coreFFI"],
            path: "Sources/MobileCodeConnectMobileSdk"
        ),
    ]
)
