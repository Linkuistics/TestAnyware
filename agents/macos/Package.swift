// swift-tools-version: 6.0
import PackageDescription

// Self-contained Swift package. Intentionally does NOT path-depend on
// cli/ for the protocol types — host and agent share a wire-level contract
// (JSON-RPC 2.0, port 8648, schema in docs/architecture/agent-protocol.md),
// not Swift types. The host CLI will migrate to Rust; at that point each
// language side independently models the wire protocol. See LLM_STATE
// for the decision rationale.
let package = Package(
    name: "TestAnywareAgent",
    platforms: [.macOS(.v14)],
    products: [
        .executable(name: "testanyware-agent", targets: ["testanyware-agent"]),
    ],
    dependencies: [
        .package(url: "https://github.com/hummingbird-project/hummingbird.git", from: "2.21.0"),
    ],
    targets: [
        .target(
            name: "TestAnywareAgentProtocol",
            dependencies: [],
            path: "Sources/TestAnywareAgentProtocol"
        ),
        .target(
            name: "TestAnywareAgent",
            dependencies: [
                "TestAnywareAgentProtocol",
            ],
            path: "Sources/TestAnywareAgent",
            linkerSettings: [
                .linkedFramework("ApplicationServices"),
                .linkedFramework("CoreGraphics"),
            ]
        ),
        .executableTarget(
            name: "testanyware-agent",
            dependencies: [
                "TestAnywareAgent",
                .product(name: "Hummingbird", package: "hummingbird"),
            ],
            path: "Sources/testanyware-agent"
        ),
        .testTarget(
            name: "TestAnywareAgentTests",
            dependencies: [
                "TestAnywareAgent",
                "TestAnywareAgentProtocol",
            ],
            path: "Tests/TestAnywareAgentTests"
        ),
    ]
)
