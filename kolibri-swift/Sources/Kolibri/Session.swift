import CKolibri
import Foundation

/// Retains the wire-tap callback and carries it across the C boundary via an
/// opaque `user` pointer.
private final class WireBox {
    let callback: @Sendable (WireEvent) -> Void
    init(_ callback: @escaping @Sendable (WireEvent) -> Void) { self.callback = callback }
}

/// C-ABI trampoline: unpacks the `user` token back into a `WireBox` and forwards
/// the packet. Non-capturing, so it converts to a `@convention(c)` pointer.
private func wireTrampoline(
    _ user: UnsafeMutableRawPointer?,
    _ direction: UnsafePointer<CChar>?,
    _ command: UnsafePointer<CChar>?,
    _ opcode: UInt16,
    _ seq: UInt16,
    _ json: UnsafePointer<CChar>?
) {
    guard let user = user else { return }
    let box = Unmanaged<WireBox>.fromOpaque(user).takeUnretainedValue()
    box.callback(WireEvent(
        direction: direction.map { String(cString: $0) } ?? "",
        command: command.map { String(cString: $0) } ?? "",
        opcode: opcode,
        seq: seq,
        json: json.map { String(cString: $0) } ?? ""
    ))
}

/// The result of a single-request media upload.
public struct UploadResult: Sendable {
    public let status: UInt16
    public let body: Data
}

/// A live protocol session backed by the Rust core. A session owns a tokio
/// runtime; each network call blocks until it completes. The `async` methods run
/// those blocking calls off the Swift cooperative pool; if you want the raw
/// blocking behaviour, use `session.blocking`.
public final class Session: @unchecked Sendable {
    private let handle: OpaquePointer
    private let wireBox: WireBox?
    private let ffiQueue = DispatchQueue(label: "ru.kolibri.session.ffi")
    private let pushQueue = DispatchQueue(label: "ru.kolibri.session.push")

    /// Opens a session from `config`, running its device fields through the
    /// sessionInit handshake shape. Does not connect yet; call `connect()`.
    public init(config: Config) throws {
        let bag = CStringBag()
        var cconfig = KConfig(
            host: bag.dup(config.host),
            port: config.port,
            device_id: bag.dup(config.deviceID),
            instance_id: bag.dup(config.instanceID),
            app_version: bag.dup(config.appVersion),
            build_number: config.buildNumber,
            device_type: bag.dup(config.deviceType),
            os_version: bag.dup(config.osVersion),
            timezone: bag.dup(config.timezone),
            screen: bag.dup(config.screen),
            push_device_type: bag.dup(config.pushDeviceType),
            arch: bag.dup(config.arch),
            locale: bag.dup(config.locale),
            device_name: bag.dup(config.deviceName),
            device_locale: bag.dup(config.deviceLocale),
            client_session_id: config.clientSessionID,
            ping_interval_secs: config.pingIntervalSeconds,
            ping_interactive: config.pingInteractive,
            auto_reconnect: config.autoReconnect,
            insecure_tls: config.insecureTLS,
            proxy: bag.dup(config.proxy)
        )

        let box = config.onWire.map { WireBox($0) }
        self.wireBox = box
        let user = box.map { Unmanaged.passUnretained($0).toOpaque() }
        let callback: KWireCb? = box == nil ? nil : wireTrampoline

        var out: OpaquePointer?
        try check(kolibri_session_new(&cconfig, callback, user, &out))
        guard let out = out else { throw KolibriError("kolibri_session_new returned no handle") }
        self.handle = out
    }

    deinit {
        kolibri_session_free(handle)
    }

    // MARK: - Async facade

    /// Runs the sessionInit handshake and returns the decoded handshake payload.
    public func connect() async throws -> [String: Any] {
        try await offload { try self.connectDictBlocking() }
    }

    /// `connect()` returning the handshake payload as a raw JSON string.
    public func connectJSON() async throws -> String {
        try await offload { try self.connectJSONBlocking() }
    }

    /// `connect()` returning the raw msgpack payload.
    public func connectRaw() async throws -> Data {
        try await offload { try self.connectRawBlocking() }
    }

    /// Sends `opcode` with a msgpack payload and returns the response payload.
    public func request(_ opcode: UInt16, _ payload: Data = Data()) async throws -> Data {
        try await offload { try self.requestBlocking(opcode, payload) }
    }

    /// Sends a JSON payload and gets the response as JSON, no msgpack library
    /// needed. `{"$bin":"<base64>"}` in the request marks a binary field; binary
    /// in the response comes back as base64.
    public func requestJSON(_ opcode: UInt16, _ jsonIn: String) async throws -> String {
        try await offload { try self.requestJSONBlocking(opcode, jsonIn) }
    }

    /// Fire-and-forget send; returns the assigned seq.
    public func send(_ opcode: UInt16, _ payload: Data = Data()) async throws -> UInt16 {
        try await offload { try self.sendBlocking(opcode, payload) }
    }

    /// Waits up to `timeout` seconds for the next server push. A negative
    /// timeout blocks forever; `nil` means the wait timed out.
    public func nextPush(timeout: TimeInterval = -1) async throws -> Push? {
        try await offload { try self.nextPushBlocking(timeout: timeout) }
    }

    /// POSTs `data` to a CDN url in a single request.
    public func uploadFile(url: String, data: Data, filename: String) async throws -> UploadResult {
        try await offload { try self.uploadFileBlocking(url: url, data: data, filename: filename) }
    }

    /// Uploads `data` as multipart/form-data.
    public func uploadPhoto(url: String, data: Data, filename: String) async throws -> UploadResult {
        try await offload { try self.uploadPhotoBlocking(url: url, data: data, filename: filename) }
    }

    /// Uploads `data` in parallel resumable chunks; returns true on success.
    public func uploadVideo(url: String, data: Data, chunkSize: Int, concurrency: Int) async throws -> Bool {
        try await offload {
            try self.uploadVideoBlocking(url: url, data: data, chunkSize: chunkSize, concurrency: concurrency)
        }
    }

    /// An async stream of server pushes. It polls the core on a background queue
    /// with `pollInterval`-second waits so the stream observes cancellation; the
    /// loop stops when the task is cancelled or the stream's consumer stops.
    public func pushes(pollInterval: TimeInterval = 1.0) -> AsyncStream<Push> {
        AsyncStream { continuation in
            let cancelled = AtomicFlag()
            pushQueue.async { [weak self] in
                while !cancelled.value {
                    guard let self else { break }
                    do {
                        if let push = try self.nextPushBlocking(timeout: pollInterval) {
                            continuation.yield(push)
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

    // MARK: - Instant (non-blocking) accessors

    /// The current session state.
    public var state: SessionState {
        SessionState(rawValue: Int(kolibri_session_state(handle))) ?? .disconnected
    }

    /// The keepalive interactive flag (foreground/background hint).
    public var pingInteractive: Bool {
        get { kolibri_session_ping_interactive(handle) }
        set { kolibri_session_set_ping_interactive(handle, newValue) }
    }

    /// The media HTTP User-Agent derived from the handshake device.
    public var userAgent: String {
        takeString(kolibri_session_user_agent(handle))
    }

    /// Stops the session and disables auto-reconnect.
    public func disconnect() {
        kolibri_session_disconnect(handle)
    }

    /// A view exposing the synchronous, blocking versions of the network calls.
    public var blocking: BlockingSession { BlockingSession(self) }

    // MARK: - Blocking implementations

    func connectDictBlocking() throws -> [String: Any] {
        try jsonToDictionary(try connectJSONBlocking())
    }

    func connectJSONBlocking() throws -> String {
        var out: UnsafeMutablePointer<CChar>?
        try check(kolibri_session_connect_json(handle, &out))
        return takeString(out)
    }

    func connectRawBlocking() throws -> Data {
        var out = KBytes()
        try check(kolibri_session_connect(handle, &out))
        return takeBytes(out)
    }

    func requestBlocking(_ opcode: UInt16, _ payload: Data) throws -> Data {
        var out = KBytes()
        let err = payload.withU8 { ptr, len in
            kolibri_session_request(handle, opcode, ptr, len, &out)
        }
        try check(err)
        return takeBytes(out)
    }

    func requestJSONBlocking(_ opcode: UInt16, _ jsonIn: String) throws -> String {
        var out: UnsafeMutablePointer<CChar>?
        let err = jsonIn.withCString { kolibri_session_request_json(handle, opcode, $0, &out) }
        try check(err)
        return takeString(out)
    }

    func sendBlocking(_ opcode: UInt16, _ payload: Data) throws -> UInt16 {
        var seq: UInt16 = 0
        let err = payload.withU8 { ptr, len in
            kolibri_session_send(handle, opcode, ptr, len, &seq)
        }
        try check(err)
        return seq
    }

    func nextPushBlocking(timeout: TimeInterval) throws -> Push? {
        var opcode: UInt16 = 0
        var out: UnsafeMutablePointer<CChar>?
        var got = false
        try check(kolibri_session_next_push_json(handle, pushTimeoutMillis(timeout), &opcode, &out, &got))
        guard got else { return nil }
        return Push(opcode: opcode, payload: try jsonToDictionary(takeString(out)))
    }

    func uploadFileBlocking(url: String, data: Data, filename: String) throws -> UploadResult {
        try upload(url: url, data: data, filename: filename, kolibri_upload_file)
    }

    func uploadPhotoBlocking(url: String, data: Data, filename: String) throws -> UploadResult {
        try upload(url: url, data: data, filename: filename, kolibri_upload_photo)
    }

    func uploadVideoBlocking(url: String, data: Data, chunkSize: Int, concurrency: Int) throws -> Bool {
        var ok = false
        let err = url.withCString { curl in
            data.withU8 { ptr, len in
                kolibri_upload_video(handle, curl, ptr, len, chunkSize, concurrency, &ok)
            }
        }
        try check(err)
        return ok
    }

    private func upload(
        url: String,
        data: Data,
        filename: String,
        _ fn: (OpaquePointer?, UnsafePointer<CChar>?, UnsafePointer<UInt8>?, Int, UnsafePointer<CChar>?, UnsafeMutablePointer<UInt16>?, UnsafeMutablePointer<KBytes>?) -> UnsafeMutablePointer<CChar>?
    ) throws -> UploadResult {
        var status: UInt16 = 0
        var body = KBytes()
        let err = url.withCString { curl in
            filename.withCString { cname in
                data.withU8 { ptr, len in
                    fn(handle, curl, ptr, len, cname, &status, &body)
                }
            }
        }
        try check(err)
        return UploadResult(status: status, body: takeBytes(body))
    }

    private func offload<T>(_ body: @escaping () throws -> T) async throws -> T {
        try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<T, Error>) in
            ffiQueue.async {
                do { continuation.resume(returning: try body()) }
                catch { continuation.resume(throwing: error) }
            }
        }
    }
}

/// The synchronous, blocking view of a `Session` (`session.blocking`). Each call
/// blocks the calling thread until the core completes it. For CLI and server code
/// outside Swift concurrency.
public struct BlockingSession {
    private let session: Session
    init(_ session: Session) { self.session = session }

    public func connect() throws -> [String: Any] { try session.connectDictBlocking() }
    public func connectJSON() throws -> String { try session.connectJSONBlocking() }
    public func connectRaw() throws -> Data { try session.connectRawBlocking() }
    public func request(_ opcode: UInt16, _ payload: Data = Data()) throws -> Data {
        try session.requestBlocking(opcode, payload)
    }
    public func requestJSON(_ opcode: UInt16, _ jsonIn: String) throws -> String {
        try session.requestJSONBlocking(opcode, jsonIn)
    }
    public func send(_ opcode: UInt16, _ payload: Data = Data()) throws -> UInt16 {
        try session.sendBlocking(opcode, payload)
    }
    public func nextPush(timeout: TimeInterval = -1) throws -> Push? {
        try session.nextPushBlocking(timeout: timeout)
    }
    public func uploadFile(url: String, data: Data, filename: String) throws -> UploadResult {
        try session.uploadFileBlocking(url: url, data: data, filename: filename)
    }
    public func uploadPhoto(url: String, data: Data, filename: String) throws -> UploadResult {
        try session.uploadPhotoBlocking(url: url, data: data, filename: filename)
    }
    public func uploadVideo(url: String, data: Data, chunkSize: Int, concurrency: Int) throws -> Bool {
        try session.uploadVideoBlocking(url: url, data: data, chunkSize: chunkSize, concurrency: concurrency)
    }
}

/// A negative timeout blocks forever (−1); otherwise seconds become milliseconds.
func pushTimeoutMillis(_ timeout: TimeInterval) -> Int64 {
    timeout < 0 ? -1 : Int64(timeout * 1000)
}

/// A minimal thread-safe boolean flag for cross-queue cancellation.
final class AtomicFlag: @unchecked Sendable {
    private let lock = NSLock()
    private var flag = false
    var value: Bool { lock.lock(); defer { lock.unlock() }; return flag }
    func set() { lock.lock(); flag = true; lock.unlock() }

    /// Sets the flag and reports whether this call is the one that flipped it
    /// (false if it was already set). Guards a one-shot free.
    func trySet() -> Bool {
        lock.lock(); defer { lock.unlock() }
        if flag { return false }
        flag = true
        return true
    }
}
