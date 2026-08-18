// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "WFObserverConsole",
    platforms: [
        .macOS(.v13),
    ],
    dependencies: [
        .package(name: "WFObserver", path: "../../../dist/apple"),
    ],
    targets: [
        .executableTarget(
            name: "WFObserverConsole",
            dependencies: [
                .product(name: "WFObserver", package: "WFObserver"),
            ]
        ),
    ]
)
