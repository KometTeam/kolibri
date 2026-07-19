import CKolibri
import Foundation

/// The anti-spoof fingerprint (authRequest `mode` / login `chatCacheFingerprint`).
public enum Auth {
    /// Known reference-client digests. Override the `Auth.fingerprint` arguments
    /// if they change.
    public static let defaultSignatureDigest =
        hexToData("1684414033eb263e2c615f8b7df5ed8793850a07656304997fbf07e9e21e1e93")
    public static let defaultDexDigest =
        hexToData("0a6265f6e5d8231b9cba641f8c40475e6f3baeb06ed41b804b9bf7307aa4214e")
    public static let defaultSoDigest =
        hexToData("90e2fb8745b17b42a10182f8d8ac590e3fca5b311e2ce2d5144fa2c18cb3090d")

    /// Builds the 96-byte anti-spoof fingerprint. `signature`, `dex`, and `so`
    /// default to the reference-client digests above.
    public static func fingerprint(
        callsSeed: Int64,
        deviceID: String,
        signature: Data? = nil,
        dex: Data? = nil,
        so: Data? = nil
    ) -> Data {
        let sig = signature ?? defaultSignatureDigest
        let dexDigest = dex ?? defaultDexDigest
        let soDigest = so ?? defaultSoDigest

        var out = KBytes()
        deviceID.withCString { cdev in
            sig.withU8 { sp, sl in
                dexDigest.withU8 { dp, dl in
                    soDigest.withU8 { op, ol in
                        _ = kolibri_auth_mode(sp, sl, dp, dl, op, ol, callsSeed, cdev, &out)
                    }
                }
            }
        }
        return takeBytes(out)
    }
}

/// Decodes a hex string into bytes (ignoring any non-hex characters).
func hexToData(_ hex: String) -> Data {
    var data = Data(capacity: hex.count / 2)
    var byte: UInt8 = 0
    var haveHigh = false
    for c in hex.utf8 {
        let nibble: UInt8
        switch c {
        case 0x30...0x39: nibble = c - 0x30
        case 0x61...0x66: nibble = c - 0x61 + 10
        case 0x41...0x46: nibble = c - 0x41 + 10
        default: continue
        }
        if haveHigh {
            data.append(byte << 4 | nibble)
            haveHigh = false
        } else {
            byte = nibble
            haveHigh = true
        }
    }
    return data
}
