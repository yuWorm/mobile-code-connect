// swift-tools-version: 5.9

import PackageDescription

let package = Package(
    name: "QuicTunnelMobileSdk",
    platforms: [
        .iOS(.v17),
        .macOS(.v14),
    ],
    products: [
        .library(
            name: "QuicTunnelMobileSdk",
            targets: ["QuicTunnelMobileSdk"]
        ),
    ],
    targets: [
        .binaryTarget(
            name: "quic_tunnel_mobile_coreFFI",
            path: "Artifacts/quic_tunnel_mobile_coreFFI.xcframework"
        ),
        .target(
            name: "QuicTunnelMobileSdk",
            dependencies: ["quic_tunnel_mobile_coreFFI"],
            path: "Sources/QuicTunnelMobileSdk"
        ),
    ]
)
