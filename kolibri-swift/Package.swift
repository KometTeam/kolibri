// swift-tools-version: 5.9
import PackageDescription

// Kolibri links CKolibri.xcframework, the kolibri-swift-ffi static library (a C
// ABI over kolibri-net, in ./rust) packaged for Apple platforms. Build it first;
// the same script drives both the quick and the full build:
//
//     ./build-rust.sh          # macOS slice only — fast local dev/test loop
//     ./build-xcframework.sh   # iOS device + iOS simulator + macOS — for shipping
//     swift build
//     swift test
//
// Both produce CKolibri.xcframework at the package root, which this manifest
// links unconditionally. Run everything from the kolibri-swift directory.
let package = Package(
    name: "Kolibri",
    platforms: [
        .macOS(.v12),
        .iOS(.v15),
    ],
    products: [
        .library(name: "Kolibri", targets: ["Kolibri"]),
    ],
    targets: [
        .binaryTarget(name: "CKolibri", path: "CKolibri.xcframework"),
        .target(
            name: "Kolibri",
            dependencies: ["CKolibri"],
            linkerSettings: [
                .linkedFramework("CoreFoundation", .when(platforms: [.macOS, .iOS])),
                .linkedFramework("Security", .when(platforms: [.macOS, .iOS])),
            ]
        ),
        .testTarget(name: "KolibriTests", dependencies: ["Kolibri"]),
    ]
)
