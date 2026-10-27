// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "SwitcherUI",
    platforms: [.macOS(.v13)],
    products: [
        .library(name: "SwitcherUI", targets: ["SwitcherUI"]),
        .executable(name: "SwitcherPreview", targets: ["SwitcherPreview"]),
    ],
    targets: [
        .target(name: "SwitcherUI"),
        .executableTarget(name: "SwitcherPreview", dependencies: ["SwitcherUI"]),
        .testTarget(name: "SwitcherUITests", dependencies: ["SwitcherUI"]),
    ]
)
