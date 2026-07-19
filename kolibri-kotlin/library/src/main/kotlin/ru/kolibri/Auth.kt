package ru.kolibri

/** The anti-spoof fingerprint (authRequest `mode` / login `chatCacheFingerprint`). */
object Auth {
    /** Known reference-client digests. Override the [fingerprint] arguments if they change. */
    val defaultSignatureDigest: ByteArray =
        hexToBytes("1684414033eb263e2c615f8b7df5ed8793850a07656304997fbf07e9e21e1e93")
    val defaultDexDigest: ByteArray =
        hexToBytes("0a6265f6e5d8231b9cba641f8c40475e6f3baeb06ed41b804b9bf7307aa4214e")
    val defaultSoDigest: ByteArray =
        hexToBytes("90e2fb8745b17b42a10182f8d8ac590e3fca5b311e2ce2d5144fa2c18cb3090d")

    /**
     * Builds the 96-byte anti-spoof fingerprint. [signature], [dex], and [so]
     * default to the reference-client digests above.
     */
    fun fingerprint(
        callsSeed: Long,
        deviceId: String,
        signature: ByteArray = defaultSignatureDigest,
        dex: ByteArray = defaultDexDigest,
        so: ByteArray = defaultSoDigest,
    ): ByteArray {
        Native.ensureLoaded()
        return Native.authMode(signature, dex, so, callsSeed, deviceId)
    }
}

/** Decodes a hex string into bytes, ignoring any non-hex characters. */
internal fun hexToBytes(hex: String): ByteArray {
    val out = ArrayList<Byte>(hex.length / 2)
    var value = 0
    var haveHigh = false
    for (c in hex) {
        val nibble = when (c) {
            in '0'..'9' -> c - '0'
            in 'a'..'f' -> c - 'a' + 10
            in 'A'..'F' -> c - 'A' + 10
            else -> continue
        }
        if (haveHigh) {
            out.add(((value shl 4) or nibble).toByte())
            haveHigh = false
        } else {
            value = nibble
            haveHigh = true
        }
    }
    return out.toByteArray()
}
