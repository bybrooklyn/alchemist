// swift-tools-version: 6.2

import PackageDescription

let package = Package(
    name: "AlchemistMac",
    platforms: [
        .macOS(.v26)
    ],
    products: [
        .executable(name: "AlchemistMac", targets: ["AlchemistMac"])
    ],
    targets: [
        .target(
            name: "AlchemistMacCore",
            path: "Sources/AlchemistMacCore"
        ),
        .executableTarget(
            name: "AlchemistMac",
            dependencies: ["AlchemistMacCore"],
            path: "Sources/AlchemistMac"
        ),
        .executableTarget(
            name: "AlchemistMacChecks",
            dependencies: ["AlchemistMacCore"],
            path: "Sources/AlchemistMacChecks"
        )
    ]
)
