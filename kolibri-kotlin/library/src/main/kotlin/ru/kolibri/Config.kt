package ru.kolibri

/** The state of a [Session], as reported by [Session.state]. */
enum class SessionState(val code: Int) {
    DISCONNECTED(0),
    CONNECTING(1),
    CONNECTED(2),
    ONLINE(3);

    companion object {
        fun fromCode(code: Int): SessionState =
            entries.firstOrNull { it.code == code } ?: DISCONNECTED
    }
}

/**
 * One tapped packet, delivered to [Config.onWire].
 *
 * @property direction "out" or "in".
 * @property command "request", "ok", "not_found", "error", or "push".
 * @property json the payload rendered as JSON (lossy; binary fields become base64).
 */
data class WireEvent(
    val direction: String,
    val command: String,
    val opcode: Int,
    val seq: Int,
    val json: String,
)

/** A server push: an opcode plus its decoded payload as raw JSON. */
data class Push(
    val opcode: Int,
    /** The payload as a JSON string (binary fields tagged as `{"$bin":"<base64>"}`). */
    val json: String,
)

/** The result of a single-request media upload. */
data class UploadResult(
    val status: Int,
    val body: ByteArray,
) {
    override fun equals(other: Any?): Boolean =
        other is UploadResult && status == other.status && body.contentEquals(other.body)

    override fun hashCode(): Int = 31 * status + body.contentHashCode()
}

/**
 * Device fields (which feed the sessionInit handshake) and connection options.
 * Start from `Config(host = ...)` for defaults matching the reference client and
 * override any field before opening a [Session].
 *
 * @property proxy "scheme://[user:pass@]host:port", or "" for a direct connection.
 * @property onWire optional traffic tap: every packet in both directions is reported.
 */
data class Config(
    val host: String,
    val port: Int = 443,
    val deviceId: String = "kolibri-kotlin",
    val instanceId: String = "kolibri-kotlin",
    val appVersion: String = "26.20.2",
    val buildNumber: Long = 6758,
    val deviceType: String = "ANDROID",
    val osVersion: String = "Android 14",
    val timezone: String = "Europe/Moscow",
    val screen: String = "420dpi 420dpi 1080x2340",
    val pushDeviceType: String = "GCM",
    val arch: String = "arm64-v8a",
    val locale: String = "ru",
    val deviceName: String = "Kotlin",
    val deviceLocale: String = "ru",
    val clientSessionId: Long = 1_700_000_000,
    val pingIntervalSeconds: Long = 30,
    val pingInteractive: Boolean = true,
    val autoReconnect: Boolean = true,
    val insecureTls: Boolean = false,
    val proxy: String = "",
    val onWire: ((WireEvent) -> Unit)? = null,
)
