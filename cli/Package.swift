// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "TestAnywareCLI",
    platforms: [.macOS(.v14)],
    products: [
        .library(name: "TestAnywareDriver", targets: ["TestAnywareDriver"]),
        .library(name: "TestAnywareAgentProtocol", targets: ["TestAnywareAgentProtocol"]),
        .executable(name: "testanyware", targets: ["testanyware"]),
    ],
    dependencies: [
        .package(url: "https://github.com/apple/swift-argument-parser.git", from: "1.5.0"),
        .package(url: "https://github.com/hummingbird-project/hummingbird.git", from: "2.21.0"),
        .package(name: "royalvnc", path: "../vendored/royalvnc"),
    ],
    targets: [
        // MARK: - Libraries
        .target(
            name: "TestAnywareAgentProtocol",
            dependencies: [],
            path: "Sources/TestAnywareAgentProtocol"
        ),
        .target(
            name: "TestAnywareDriver",
            dependencies: [
                "TestAnywareAgentProtocol",
                .product(name: "Hummingbird", package: "hummingbird"),
                .product(name: "RoyalVNCKit", package: "royalvnc"),
            ],
            path: "Sources/TestAnywareDriver",
            linkerSettings: [
                .linkedFramework("CoreGraphics"),
                .linkedFramework("AVFoundation"),
                .linkedFramework("CoreMedia"),
                .linkedFramework("CoreVideo"),
                .linkedFramework("Vision"),
            ]
        ),

        // MARK: - Executables
        .executableTarget(
            name: "testanyware",
            dependencies: [
                "TestAnywareAgentProtocol",
                "TestAnywareDriver",
                .product(name: "ArgumentParser", package: "swift-argument-parser"),
            ],
            path: "Sources/testanyware"
        ),
        // MARK: - Unit tests
        .testTarget(
            name: "TestAnywareDriverTests",
            dependencies: [
                "TestAnywareDriver",
                "TestAnywareAgentProtocol",
                .product(name: "HummingbirdTesting", package: "hummingbird"),
                .product(name: "RoyalVNCKit", package: "royalvnc"),
            ],
            path: "Tests/TestAnywareDriverTests"
        ),
        .testTarget(
            name: "TestAnywareAgentProtocolTests",
            dependencies: [
                "TestAnywareAgentProtocol",
            ],
            path: "Tests/TestAnywareAgentProtocolTests"
        ),
        // MARK: - Integration tests (require a VNC endpoint)
        .testTarget(
            name: "IntegrationTests",
            dependencies: [
                "TestAnywareDriver",
                .product(name: "RoyalVNCKit", package: "royalvnc"),
            ],
            path: "Tests/IntegrationTests"
        ),
    ]
)
