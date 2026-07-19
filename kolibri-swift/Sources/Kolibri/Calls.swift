import CKolibri
import Foundation

/// A STUN/TURN server in the shape a WebRTC stack expects.
public struct IceServer: Codable, Sendable {
    public let urls: [String]
    public let username: String?
    public let credential: String?
}

/// Decoded call params (vcp): endpoints, ICE servers, and, when `decodeVCP` is
/// given a conversation id, the ws2 connect url.
public struct CallParams: Codable, Sendable {
    public let token: String
    public let wsEndpoint: String
    public let stun: String?
    public let turn: [String]
    public let turnUser: String?
    public let turnPassword: String?
    public let isVideo: Bool
    public let expiresAt: Int64?
    public let userId: Int64
    public let iceServers: [IceServer]
    public let ws2Url: String?
}

/// A parsed ws2 `connection` notification.
public struct ConnectionInfo: Codable, Sendable {
    public let topology: String?
    public let isSfu: Bool
    public let participants: [Int64]
    public let peer: Int64?
    public let iceServers: [IceServer]
}

/// A parsed ws2 `transmitted-data` notification: an SDP or an ICE candidate.
public enum TransmittedData: Sendable {
    case sdp(type: String, sdp: String)
    case candidate(candidate: String, sdpMid: String, sdpMLineIndex: Int64)
}

private func callDecoder() -> JSONDecoder {
    let decoder = JSONDecoder()
    decoder.keyDecodingStrategy = .convertFromSnakeCase
    return decoder
}

/// A ws2 signaling client. It connects on `Call.connect` and each call blocks on
/// its own runtime. Signaling only; the WebRTC media stack stays in your app.
public final class Call: @unchecked Sendable {
    private let handle: OpaquePointer
    private let closed = AtomicFlag()
    private let ffiQueue = DispatchQueue(label: "ru.kolibri.call.ffi")
    private let notifQueue = DispatchQueue(label: "ru.kolibri.call.notif")

    private init(handle: OpaquePointer) { self.handle = handle }

    deinit { closeHandle() }

    // MARK: - Connect

    /// Opens a ws2 signaling connection. `userAgent` and `proxy` may be empty.
    public static func connect(url: String, userAgent: String = "", proxy: String = "") async throws -> Call {
        try await withCheckedThrowingContinuation { continuation in
            DispatchQueue.global().async {
                do { continuation.resume(returning: try connectBlocking(url: url, userAgent: userAgent, proxy: proxy)) }
                catch { continuation.resume(throwing: error) }
            }
        }
    }

    /// Blocking variant of `connect`.
    public static func connectBlocking(url: String, userAgent: String = "", proxy: String = "") throws -> Call {
        var out: OpaquePointer?
        let err = url.withCString { curl in
            userAgent.withCString { cua in
                proxy.withCString { cpx in
                    kolibri_call_connect(curl, cua, cpx, &out)
                }
            }
        }
        try check(err)
        guard let out = out else { throw KolibriError("kolibri_call_connect returned no handle") }
        return Call(handle: out)
    }

    // MARK: - Async facade

    /// Accepts the incoming call; returns the response JSON.
    public func accept() async throws -> String {
        try await offload { try self.acceptBlocking() }
    }

    /// Ends the call with `reason`; returns the response JSON.
    public func hangup(reason: String) async throws -> String {
        try await offload { try self.hangupBlocking(reason: reason) }
    }

    /// Sends an SDP offer/answer to a participant; returns the response JSON.
    public func transmitSDP(participantID: Int64, type: String, sdp: String) async throws -> String {
        try await offload { try self.transmitSDPBlocking(participantID: participantID, type: type, sdp: sdp) }
    }

    /// Sends an ICE candidate to a participant; returns the response JSON.
    public func transmitCandidate(participantID: Int64, candidate: String, sdpMid: String, sdpMLineIndex: Int64) async throws -> String {
        try await offload {
            try self.transmitCandidateBlocking(participantID: participantID, candidate: candidate, sdpMid: sdpMid, sdpMLineIndex: sdpMLineIndex)
        }
    }

    /// Updates the audio/video/screen flags; returns the response JSON.
    public func changeMedia(audio: Bool, video: Bool, screen: Bool) async throws -> String {
        try await offload { try self.changeMediaBlocking(audio: audio, video: video, screen: screen) }
    }

    /// Sends a raw command with a JSON object of extra fields (empty for none);
    /// returns the response JSON.
    public func sendCommand(_ command: String, extraJSON: String = "") async throws -> String {
        try await offload { try self.sendCommandBlocking(command, extraJSON: extraJSON) }
    }

    /// Waits up to `timeout` seconds for the next ws2 notification (raw JSON). A
    /// negative timeout blocks forever; `nil` means the wait timed out.
    public func nextNotification(timeout: TimeInterval = -1) async throws -> String? {
        try await offload { try self.nextNotificationBlocking(timeout: timeout) }
    }

    /// An async stream of ws2 notifications (raw JSON), polled on a background
    /// queue with `pollInterval`-second waits so it observes cancellation.
    public func notifications(pollInterval: TimeInterval = 1.0) -> AsyncStream<String> {
        AsyncStream { continuation in
            let cancelled = AtomicFlag()
            notifQueue.async { [weak self] in
                while !cancelled.value {
                    guard let self, self.isConnected else { break }
                    do {
                        if let n = try self.nextNotificationBlocking(timeout: pollInterval) {
                            continuation.yield(n)
                        }
                    } catch {
                        break
                    }
                }
                continuation.finish()
            }
            continuation.onTermination = { _ in cancelled.set() }
        }
    }

    /// Whether the ws2 socket is still up.
    public var isConnected: Bool { kolibri_call_is_connected(handle) }

    /// Hangs up the ws2 socket and frees the client. Safe to call more than once.
    public func close() { closeHandle() }

    // MARK: - Blocking implementations

    func acceptBlocking() throws -> String {
        var out: UnsafeMutablePointer<CChar>?
        try check(kolibri_call_accept(handle, &out))
        return takeString(out)
    }

    func hangupBlocking(reason: String) throws -> String {
        var out: UnsafeMutablePointer<CChar>?
        let err = reason.withCString { kolibri_call_hangup(handle, $0, &out) }
        try check(err)
        return takeString(out)
    }

    func transmitSDPBlocking(participantID: Int64, type: String, sdp: String) throws -> String {
        var out: UnsafeMutablePointer<CChar>?
        let err = type.withCString { ct in
            sdp.withCString { cs in
                kolibri_call_transmit_sdp(handle, participantID, ct, cs, &out)
            }
        }
        try check(err)
        return takeString(out)
    }

    func transmitCandidateBlocking(participantID: Int64, candidate: String, sdpMid: String, sdpMLineIndex: Int64) throws -> String {
        var out: UnsafeMutablePointer<CChar>?
        let err = candidate.withCString { cc in
            sdpMid.withCString { cm in
                kolibri_call_transmit_candidate(handle, participantID, cc, cm, sdpMLineIndex, &out)
            }
        }
        try check(err)
        return takeString(out)
    }

    func changeMediaBlocking(audio: Bool, video: Bool, screen: Bool) throws -> String {
        var out: UnsafeMutablePointer<CChar>?
        try check(kolibri_call_change_media(handle, audio, video, screen, &out))
        return takeString(out)
    }

    func sendCommandBlocking(_ command: String, extraJSON: String) throws -> String {
        var out: UnsafeMutablePointer<CChar>?
        let err = command.withCString { cc in
            extraJSON.withCString { ce in
                kolibri_call_send_command(handle, cc, ce, &out)
            }
        }
        try check(err)
        return takeString(out)
    }

    func nextNotificationBlocking(timeout: TimeInterval) throws -> String? {
        var out: UnsafeMutablePointer<CChar>?
        var got = false
        try check(kolibri_call_next_notification(handle, pushTimeoutMillis(timeout), &out, &got))
        guard got else { return nil }
        return takeString(out)
    }

    private func closeHandle() {
        if closed.trySet() {
            kolibri_call_close(handle)
        }
    }

    private func offload<T>(_ body: @escaping () throws -> T) async throws -> T {
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<T, Error>) in
            ffiQueue.async {
                do { continuation.resume(returning: try body()) }
                catch { continuation.resume(throwing: error) }
            }
        }
    }

    // MARK: - Notification parsing (pure, synchronous)

    /// Decodes a vcp call-params string. Pass a conversation id to also get
    /// `ws2Url`; pass "" to skip it. Returns nil if the vcp can't be decoded.
    public static func decodeVCP(_ vcp: String, conversationID: String = "") throws -> CallParams? {
        var got = false
        var out: UnsafeMutablePointer<CChar>?
        let err = vcp.withCString { cvcp in
            conversationID.withCString { ccid in
                kolibri_decode_vcp(cvcp, ccid, &got, &out)
            }
        }
        try check(err)
        guard got else { return nil }
        return try callDecoder().decode(CallParams.self, from: Data(takeString(out).utf8))
    }

    /// Parses a ws2 `connection` notification (raw JSON). Pass your own calls
    /// user id to get `peer` filled; pass nil to skip it.
    public static func parseConnection(_ notificationJSON: String, myUserID: Int64? = nil) throws -> ConnectionInfo {
        var out: UnsafeMutablePointer<CChar>?
        let err = notificationJSON.withCString {
            kolibri_parse_connection($0, myUserID ?? 0, myUserID != nil, &out)
        }
        try check(err)
        return try callDecoder().decode(ConnectionInfo.self, from: Data(takeString(out).utf8))
    }

    /// Parses a ws2 `transmitted-data` notification (raw JSON). Returns nil when
    /// it carries neither an SDP nor a candidate.
    public static func parseTransmittedData(_ notificationJSON: String) throws -> TransmittedData? {
        var got = false
        var out: UnsafeMutablePointer<CChar>?
        let err = notificationJSON.withCString {
            kolibri_parse_transmitted_data($0, &got, &out)
        }
        try check(err)
        guard got else { return nil }

        let object = try JSONSerialization.jsonObject(with: Data(takeString(out).utf8)) as? [String: Any] ?? [:]
        switch object["kind"] as? String {
        case "sdp":
            return .sdp(type: object["type"] as? String ?? "", sdp: object["sdp"] as? String ?? "")
        case "candidate":
            let index = (object["sdp_mline_index"] as? NSNumber)?.int64Value ?? 0
            return .candidate(
                candidate: object["candidate"] as? String ?? "",
                sdpMid: object["sdp_mid"] as? String ?? "",
                sdpMLineIndex: index
            )
        default:
            return nil
        }
    }
}
