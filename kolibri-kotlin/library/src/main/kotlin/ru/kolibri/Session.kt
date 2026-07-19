package ru.kolibri

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.isActive
import kotlinx.coroutines.withContext
import java.util.concurrent.atomic.AtomicBoolean

/**
 * A live protocol session backed by the Rust core. A session owns a tokio
 * runtime; each network call blocks until it completes. The `suspend` methods
 * run those blocking calls on [Dispatchers.IO]; for the raw blocking behaviour
 * (CLI/server code outside coroutines) use [blocking].
 *
 * A session holds native resources; [close] it when done, or use it inside
 * [use] `{ }`. A finalizer frees the handle as a last resort.
 */
class Session private constructor(
    private val handle: Long,
    // Retained so the tap object outlives the session on the Kotlin heap.
    @Suppress("unused") private val wireTap: WireTap?,
) : AutoCloseable {

    private val closed = AtomicBoolean(false)

    companion object {
        /**
         * Opens a session from [config], running its device fields through the
         * sessionInit handshake shape. Does not connect yet; call [connect].
         */
        fun open(config: Config): Session {
            Native.ensureLoaded()
            val onWire = config.onWire
            val tap: WireTap? = onWire?.let { cb ->
                WireTap { direction, command, opcode, seq, json ->
                    cb(WireEvent(direction, command, opcode, seq, json))
                }
            }
            val handle = Native.sessionNew(
                config.host,
                config.port,
                config.deviceId,
                config.instanceId,
                config.appVersion,
                config.buildNumber,
                config.deviceType,
                config.osVersion,
                config.timezone,
                config.screen,
                config.pushDeviceType,
                config.arch,
                config.locale,
                config.deviceName,
                config.deviceLocale,
                config.clientSessionId,
                config.pingIntervalSeconds,
                config.pingInteractive,
                config.autoReconnect,
                config.insecureTls,
                config.proxy,
                tap,
            )
            return Session(handle, tap)
        }
    }

    // ---- suspending facade ----

    /** Runs the sessionInit handshake and returns the handshake payload as JSON. */
    suspend fun connect(): String = io { blocking.connect() }

    /** [connect] returning the raw msgpack handshake payload. */
    suspend fun connectRaw(): ByteArray = io { blocking.connectRaw() }

    /** Sends `opcode` with a msgpack payload and returns the response payload. */
    suspend fun request(opcode: Int, payload: ByteArray = EMPTY): ByteArray =
        io { blocking.request(opcode, payload) }

    /**
     * Sends a JSON payload and gets the response as JSON; no msgpack library
     * needed. `{"$bin":"<base64>"}` in the request marks a binary field; binary
     * in the response comes back as base64.
     */
    suspend fun requestJson(opcode: Int, jsonIn: String): String =
        io { blocking.requestJson(opcode, jsonIn) }

    /** Fire-and-forget send; returns the assigned seq. */
    suspend fun send(opcode: Int, payload: ByteArray = EMPTY): Int =
        io { blocking.send(opcode, payload) }

    /**
     * Waits up to [timeoutSeconds] for the next server push. A negative timeout
     * blocks forever; `null` means the wait timed out.
     */
    suspend fun nextPush(timeoutSeconds: Double = -1.0): Push? =
        io { blocking.nextPush(timeoutSeconds) }

    /** POSTs [data] to a CDN url in a single request. */
    suspend fun uploadFile(url: String, data: ByteArray, filename: String): UploadResult =
        io { blocking.uploadFile(url, data, filename) }

    /** Uploads [data] as multipart/form-data. */
    suspend fun uploadPhoto(url: String, data: ByteArray, filename: String): UploadResult =
        io { blocking.uploadPhoto(url, data, filename) }

    /** Uploads [data] in parallel resumable chunks; returns true on success. */
    suspend fun uploadVideo(url: String, data: ByteArray, chunkSize: Int, concurrency: Int): Boolean =
        io { blocking.uploadVideo(url, data, chunkSize, concurrency) }

    /**
     * A cold [Flow] of server pushes. It polls the core on [Dispatchers.IO] with
     * [pollIntervalSeconds] waits so the flow observes cancellation; collection
     * stops when the collector's scope is cancelled.
     */
    fun pushes(pollIntervalSeconds: Double = 1.0): Flow<Push> = flow {
        while (currentCoroutineContext().isActive) {
            val push = blocking.nextPush(pollIntervalSeconds) ?: continue
            emit(push)
        }
    }.flowOn(Dispatchers.IO)

    // ---- instant (non-blocking) accessors ----

    /** The current session state. */
    val state: SessionState
        get() = SessionState.fromCode(Native.sessionState(handle))

    /** The keepalive interactive flag (foreground/background hint), settable live. */
    var pingInteractive: Boolean
        get() = Native.sessionPingInteractive(handle)
        set(value) = Native.sessionSetPingInteractive(handle, value)

    /** The media HTTP User-Agent derived from the handshake device. */
    val userAgent: String
        get() = Native.sessionUserAgent(handle)

    /** Stops the session and disables auto-reconnect. */
    fun disconnect() = Native.sessionDisconnect(handle)

    /** The synchronous, blocking view of the network calls. */
    val blocking: BlockingSession = BlockingSession(handle)

    /** Frees the native session. Idempotent; safe to call more than once. */
    override fun close() {
        if (closed.compareAndSet(false, true)) {
            Native.sessionFree(handle)
        }
    }

    @Suppress("removal")
    protected fun finalize() = close()

    private suspend inline fun <T> io(crossinline body: () -> T): T =
        withContext(Dispatchers.IO) { body() }
}

/**
 * The synchronous, blocking view of a [Session] ([Session.blocking]). Each call
 * blocks the calling thread until the core completes it.
 */
class BlockingSession internal constructor(private val handle: Long) {

    /** Runs the sessionInit handshake; returns the handshake payload as JSON. */
    fun connect(): String = Native.sessionConnectJson(handle)

    /** [connect] returning the raw msgpack handshake payload. */
    fun connectRaw(): ByteArray = Native.sessionConnect(handle)

    fun request(opcode: Int, payload: ByteArray = EMPTY): ByteArray =
        Native.sessionRequest(handle, opcode, payload)

    fun requestJson(opcode: Int, jsonIn: String): String =
        Native.sessionRequestJson(handle, opcode, jsonIn)

    fun send(opcode: Int, payload: ByteArray = EMPTY): Int =
        Native.sessionSend(handle, opcode, payload)

    fun nextPush(timeoutSeconds: Double = -1.0): Push? {
        val opcode = IntArray(1)
        val json = Native.sessionNextPushJson(handle, timeoutMillis(timeoutSeconds), opcode)
            ?: return null
        return Push(opcode[0], json)
    }

    fun uploadFile(url: String, data: ByteArray, filename: String): UploadResult =
        upload(url, data, filename, photo = false)

    fun uploadPhoto(url: String, data: ByteArray, filename: String): UploadResult =
        upload(url, data, filename, photo = true)

    fun uploadVideo(url: String, data: ByteArray, chunkSize: Int, concurrency: Int): Boolean =
        Native.uploadVideo(handle, url, data, chunkSize, concurrency)

    private fun upload(url: String, data: ByteArray, filename: String, photo: Boolean): UploadResult {
        val status = IntArray(1)
        val body = if (photo) {
            Native.uploadPhoto(handle, url, data, filename, status)
        } else {
            Native.uploadFile(handle, url, data, filename, status)
        }
        return UploadResult(status[0], body)
    }
}

private val EMPTY = ByteArray(0)
