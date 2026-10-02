// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "FluentWriter",
    platforms: [.macOS(.v14)],
    products: [
        .executable(name: "FluentWriter", targets: ["FluentWriter"]),
        .library(name: "WriterCore", targets: ["WriterCore"]),
        .library(name: "WriterAgents", targets: ["WriterAgents"]),
    ],
    targets: [
        .target(name: "WriterCore"),
        .target(name: "WriterAgents", dependencies: ["WriterCore"]),
        .executableTarget(
            name: "FluentWriter",
            dependencies: ["WriterCore", "WriterAgents"],
            resources: [.copy("Resources/Fonts")]
        ),
        .testTarget(name: "WriterCoreTests", dependencies: ["WriterCore"]),
        .testTarget(name: "WriterAgentsTests", dependencies: ["WriterAgents", "WriterCore"]),
    ],
    swiftLanguageModes: [.v5]
)
