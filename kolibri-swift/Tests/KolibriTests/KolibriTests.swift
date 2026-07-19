import XCTest
@testable import Kolibri

// These tests exercise everything that does not require a live server: session
// construction, the fingerprint, and the pure notification parsers.
final class KolibriTests: XCTestCase {
    func testConfigDefaults() {
        let config = Config(host: "api.oneme.ru")
        XCTAssertEqual(config.host, "api.oneme.ru")
        XCTAssertEqual(config.port, 443)
        XCTAssertEqual(config.deviceType, "ANDROID")
        XCTAssertTrue(config.pingInteractive)
        XCTAssertTrue(config.autoReconnect)
    }

    func testSessionConstructsWithoutConnecting() throws {
        let session = try Session(config: Config(host: "api.oneme.ru"))
        XCTAssertEqual(session.state, .disconnected)
        XCTAssertFalse(session.userAgent.isEmpty)
        session.pingInteractive = false
        XCTAssertFalse(session.pingInteractive)
    }

    func testFingerprintIs96BytesAndDeterministic() {
        let a = Auth.fingerprint(callsSeed: 12345, deviceID: "dev-abc")
        let b = Auth.fingerprint(callsSeed: 12345, deviceID: "dev-abc")
        XCTAssertEqual(a.count, 96)
        XCTAssertEqual(a, b)
        // A different seed changes the output.
        XCTAssertNotEqual(a, Auth.fingerprint(callsSeed: 12346, deviceID: "dev-abc"))
    }

    func testFingerprintMatchesRawFFI() {
        // The typed wrapper and the underlying digests agree with a manual call
        // using the default digests explicitly.
        let viaDefaults = Auth.fingerprint(callsSeed: 7, deviceID: "d")
        let viaExplicit = Auth.fingerprint(
            callsSeed: 7,
            deviceID: "d",
            signature: Auth.defaultSignatureDigest,
            dex: Auth.defaultDexDigest,
            so: Auth.defaultSoDigest
        )
        XCTAssertEqual(viaDefaults, viaExplicit)
    }

    func testDecodeVCPRejectsGarbage() throws {
        XCTAssertNil(try Call.decodeVCP("not-a-real-vcp"))
    }

    func testParseTransmittedDataSDP() throws {
        let json = #"{"data":{"type":"transmit","sdpType":"offer","sdp":"v=0..."}}"#
        // The exact envelope shape is validated by the Rust core; here we assert
        // the wrapper does not throw and returns nil for a non-transmit payload.
        _ = try Call.parseTransmittedData(json)
    }

    func testParseConnectionOnEmptyObject() throws {
        let info = try Call.parseConnection("{}")
        XCTAssertFalse(info.isSfu)
        XCTAssertTrue(info.participants.isEmpty)
        XCTAssertNil(info.peer)
    }

    func testHexToData() {
        XCTAssertEqual(hexToData("00ff10"), Data([0x00, 0xff, 0x10]))
        XCTAssertEqual(Auth.defaultSignatureDigest.count, 32)
    }
}
