// swift-tools-version:6.0
// ABOUTME: SwiftPM manifest for the Glitch Spot iOS SDK; its sources live under packages/ios.
import PackageDescription

let package = Package(
    name: "GlitchSpotKit",
    platforms: [.iOS("18.6"), .macOS(.v14)],
    products: [
        .library(name: "GlitchSpotKit", targets: ["GlitchSpotKit"]),
    ],
    targets: [
        .target(name: "GlitchSpotCore", path: "packages/ios/Sources/GlitchSpotCore"),
        .target(
            name: "GlitchSpotKit",
            dependencies: ["GlitchSpotCore"],
            path: "packages/ios/Sources/GlitchSpotKit"
        ),
        .testTarget(
            name: "GlitchSpotCoreTests",
            dependencies: ["GlitchSpotCore"],
            path: "packages/ios/Tests/GlitchSpotCoreTests"
        ),
    ]
)
