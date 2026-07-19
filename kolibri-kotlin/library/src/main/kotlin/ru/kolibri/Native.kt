package ru.kolibri

/**
 * The raw JNI surface over the kolibri core. Each function maps 1:1 to a
 * `Java_ru_kolibri_Native_*` entry point in the Rust `kolibri-kotlin-jni` crate.
 *
 * Fallible calls throw [KolibriException]; "no value" (a timed-out push, an
 * un-decodable vcp) comes back as `null`. Handles are opaque `long` pointers
 * owned by the core; free them with [sessionFree] / [callClose].
 *
 * This is internal plumbing; use [Session], [Call], and [Auth] instead.
 */
internal object Native {
    init {
        // libkolibri_kotlin.so (Android jniLibs) / .dylib / .dll on the JVM's
        // java.library.path. Callers may preload it themselves before touching
        // this object; the redundant load is a no-op.
        System.loadLibrary("kolibri_kotlin")
    }

    /** Forces the containing class (and the `loadLibrary` above) to load. */
    fun ensureLoaded() = Unit

    // ---- session ----

    external fun sessionNew(
        host: String,
        port: Int,
        deviceId: String,
        instanceId: String,
        appVersion: String,
        buildNumber: Long,
        deviceType: String,
        osVersion: String,
        timezone: String,
        screen: String,
        pushDeviceType: String,
        arch: String,
        locale: String,
        deviceName: String,
        deviceLocale: String,
        clientSessionId: Long,
        pingIntervalSecs: Long,
        pingInteractive: Boolean,
        autoReconnect: Boolean,
        insecureTls: Boolean,
        proxy: String,
        wire: WireTap?,
    ): Long

    external fun sessionConnect(handle: Long): ByteArray
    external fun sessionConnectJson(handle: Long): String
    external fun sessionRequest(handle: Long, opcode: Int, payload: ByteArray): ByteArray
    external fun sessionRequestJson(handle: Long, opcode: Int, jsonIn: String): String
    external fun sessionSend(handle: Long, opcode: Int, payload: ByteArray): Int
    external fun sessionNextPush(handle: Long, timeoutMs: Long, outOpcode: IntArray): ByteArray?
    external fun sessionNextPushJson(handle: Long, timeoutMs: Long, outOpcode: IntArray): String?
    external fun setTrustMincifryCa(enabled: Boolean)
    external fun trustMincifryCa(): Boolean

    external fun sessionState(handle: Long): Int
    external fun sessionPingInteractive(handle: Long): Boolean
    external fun sessionSetPingInteractive(handle: Long, interactive: Boolean)
    external fun sessionUserAgent(handle: Long): String
    external fun sessionDisconnect(handle: Long)
    external fun sessionFree(handle: Long)

    // ---- media uploads ----

    external fun uploadFile(handle: Long, url: String, data: ByteArray, filename: String, outStatus: IntArray): ByteArray
    external fun uploadPhoto(handle: Long, url: String, data: ByteArray, filename: String, outStatus: IntArray): ByteArray
    external fun uploadVideo(handle: Long, url: String, data: ByteArray, chunkSize: Int, concurrency: Int): Boolean

    // ---- auth ----

    external fun authMode(
        signature: ByteArray,
        dex: ByteArray,
        so: ByteArray,
        callsSeed: Long,
        deviceId: String,
    ): ByteArray

    // ---- calls: notification parsing ----

    external fun decodeVcp(vcp: String, conversationId: String): String?
    external fun parseConnection(notification: String, myUserId: Long, hasUserId: Boolean): String
    external fun parseTransmittedData(notification: String): String?

    // ---- calls: ws2 signaling ----

    external fun callConnect(url: String, userAgent: String, proxy: String): Long
    external fun callAccept(handle: Long): String
    external fun callHangup(handle: Long, reason: String): String
    external fun callTransmitSdp(handle: Long, participantId: Long, sdpType: String, sdp: String): String
    external fun callTransmitCandidate(
        handle: Long,
        participantId: Long,
        candidate: String,
        sdpMid: String,
        sdpMlineIndex: Long,
    ): String
    external fun callChangeMedia(handle: Long, audio: Boolean, video: Boolean, screen: Boolean): String
    external fun callSendCommand(handle: Long, command: String, extraJson: String): String
    external fun callNextNotification(handle: Long, timeoutMs: Long): String?
    external fun callIsConnected(handle: Long): Boolean
    external fun callClose(handle: Long)
}

/**
 * A traffic tap invoked once per packet in each direction. Called from the
 * core's tokio threads; keep it fast and non-blocking, and don't throw.
 */
fun interface WireTap {
    fun onPacket(direction: String, command: String, opcode: Int, seq: Int, json: String)
}
