import CKolibri
import Foundation

/// An error surfaced by the kolibri core (a failed handshake, request, upload,
/// or call operation). `message` is the string the Rust side returned.
public struct KolibriError: Error, CustomStringConvertible {
    public let message: String
    public init(_ message: String) { self.message = message }
    public var description: String { message }
}

// MARK: - C string / byte bridging

/// Turns a fallible-call return value into a thrown error: the FFI returns NULL
/// on success or an owned error string, which we free here.
@inline(__always)
func check(_ err: UnsafeMutablePointer<CChar>?) throws {
    if let err = err {
        let message = String(cString: err)
        kolibri_string_free(err)
        throw KolibriError(message)
    }
}

/// Takes ownership of a `char *` out-param: copies it to a Swift string and
/// frees the C allocation. NULL becomes "".
@inline(__always)
func takeString(_ p: UnsafeMutablePointer<CChar>?) -> String {
    guard let p = p else { return "" }
    let s = String(cString: p)
    kolibri_string_free(p)
    return s
}

/// Takes ownership of a `KBytes` out-param: copies it to `Data` and frees the C
/// buffer.
@inline(__always)
func takeBytes(_ b: KBytes) -> Data {
    guard let ptr = b.ptr, b.len > 0 else {
        kolibri_bytes_free(b)
        return Data()
    }
    let data = Data(bytes: ptr, count: b.len)
    kolibri_bytes_free(b)
    return data
}

extension Data {
    /// Exposes the bytes as a `(ptr, len)` pair, passing `(nil, 0)` when empty,
    /// as the FFI's `slice` helper expects.
    @inline(__always)
    func withU8<R>(_ body: (UnsafePointer<UInt8>?, Int) -> R) -> R {
        if isEmpty { return body(nil, 0) }
        return withUnsafeBytes { raw in
            body(raw.bindMemory(to: UInt8.self).baseAddress, count)
        }
    }
}

/// Holds C strings alive for the duration of a single FFI call, freeing them
/// when the scope ends. Use for structs (like KConfig) whose many `const char *`
/// fields must all outlive the call.
final class CStringBag {
    private var pointers: [UnsafeMutablePointer<CChar>] = []

    /// Duplicates `s` into a C string owned by this bag.
    func dup(_ s: String) -> UnsafePointer<CChar> {
        let p = strdup(s)!
        pointers.append(p)
        return UnsafePointer(p)
    }

    deinit {
        for p in pointers { free(p) }
    }
}

// MARK: - JSON helpers

/// Decodes a JSON object string into a dictionary; "" and "null" (a request
/// that returned no body) yield an empty dictionary.
func jsonToDictionary(_ s: String) throws -> [String: Any] {
    if s.isEmpty || s == "null" { return [:] }
    guard let data = s.data(using: .utf8) else { return [:] }
    let value = try JSONSerialization.jsonObject(with: data, options: [])
    return value as? [String: Any] ?? [:]
}

/// Decodes an FFI JSON string into a `Codable` model.
func decodeJSON<T: Decodable>(_ type: T.Type, from s: String) throws -> T {
    let data = Data(s.utf8)
    return try JSONDecoder().decode(type, from: data)
}
