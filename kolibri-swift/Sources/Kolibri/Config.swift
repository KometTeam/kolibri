import Foundation

/// The state of a `Session`, as reported by `Session.state`.
public enum SessionState: Int, Sendable {
    case disconnected = 0
    case connecting = 1
    case connected = 2
    case online = 3
}

/// One tapped packet, delivered to `Config.onWire`.
public struct WireEvent: Sendable {
    /// "out" or "in".
    public let direction: String
    /// "request", "ok", "not_found", "error", or "push".
    public let command: String
    public let opcode: UInt16
    public let seq: UInt16
    /// The payload rendered as JSON (lossy; binary fields become base64).
    public let json: String
}

/// A server push: an opcode plus its decoded payload.
public struct Push {
    public let opcode: UInt16
    public let payload: [String: Any]

    public init(opcode: UInt16, payload: [String: Any]) {
        self.opcode = opcode
        self.payload = payload
    }
}

/// Device fields (which feed the sessionInit handshake) and connection options.
/// Start from `Config(host:)` for defaults matching the reference client and
/// override any field before opening a `Session`.
public struct Config: Sendable {
    public var host: String
    public var port: UInt16 = 443
    public var deviceID: String = "kolibri-swift"
    public var instanceID: String = "kolibri-swift"
    public var appVersion: String = "26.20.2"
    public var buildNumber: Int64 = 6758
    public var deviceType: String = "ANDROID"
    public var osVersion: String = "Android 14"
    public var timezone: String = "Europe/Moscow"
    public var screen: String = "420dpi 420dpi 1080x2340"
    public var pushDeviceType: String = "GCM"
    public var arch: String = "arm64-v8a"
    public var locale: String = "ru"
    public var deviceName: String = "Swift"
    public var deviceLocale: String = "ru"
    public var clientSessionID: Int64 = 1_700_000_000
    public var pingIntervalSeconds: UInt64 = 30
    public var pingInteractive: Bool = true
    public var autoReconnect: Bool = true
    public var insecureTLS: Bool = false
    /// "scheme://[user:pass@]host:port", or "" for a direct connection.
    public var proxy: String = ""
    /// Optional traffic tap: every packet in both directions is reported.
    public var onWire: (@Sendable (WireEvent) -> Void)?

    /// A config for `host` with defaults matching the reference client.
    public init(host: String) {
        self.host = host
    }
}
