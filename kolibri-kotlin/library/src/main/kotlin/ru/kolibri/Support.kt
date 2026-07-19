package ru.kolibri

/**
 * An error surfaced by the kolibri core (a failed handshake, request, upload, or
 * call operation). [message] is the string the Rust side returned.
 */
class KolibriException(message: String) : RuntimeException(message)

/**
 * A negative timeout blocks forever (−1); otherwise seconds become milliseconds.
 * Used for the push / notification wait APIs.
 */
internal fun timeoutMillis(seconds: Double): Long =
    if (seconds < 0) -1L else (seconds * 1000).toLong()
