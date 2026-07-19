package ru.kolibri

/**
 * An error surfaced by the kolibri core (a failed handshake, request, upload, or
 * call operation). [message] is the string the Rust side returned.
 */
class KolibriException(message: String) : RuntimeException(message)

/** Process-wide TLS trust policy for the kolibri core. */
object Kolibri {
    /** Trust the bundled Минцифры CA (socket, media, calls); off by default, set at startup. */
    fun setTrustMincifryCa(enabled: Boolean) {
        Native.ensureLoaded()
        Native.setTrustMincifryCa(enabled)
    }

    fun trustMincifryCa(): Boolean {
        Native.ensureLoaded()
        return Native.trustMincifryCa()
    }
}

/**
 * A negative timeout blocks forever (−1); otherwise seconds become milliseconds.
 * Used for the push / notification wait APIs.
 */
internal fun timeoutMillis(seconds: Double): Long =
    if (seconds < 0) -1L else (seconds * 1000).toLong()
