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
 * A ws2 signaling client. It connects on [connect] and each call blocks on its
 * own runtime. Signaling only; the WebRTC media stack stays in your app. All
 * responses and notifications cross the boundary as raw JSON strings; decode
 * them with whatever JSON library you use.
 *
 * Holds native resources; [close] it when done, or use it inside [use] `{ }`.
 */
class Call private constructor(private val handle: Long) : AutoCloseable {

    private val closed = AtomicBoolean(false)

    companion object {
        /** Opens a ws2 signaling connection. [userAgent] and [proxy] may be "". */
        suspend fun connect(url: String, userAgent: String = "", proxy: String = ""): Call =
            withContext(Dispatchers.IO) { connectBlocking(url, userAgent, proxy) }

        /** Blocking variant of [connect]. */
        fun connectBlocking(url: String, userAgent: String = "", proxy: String = ""): Call {
            Native.ensureLoaded()
            return Call(Native.callConnect(url, userAgent, proxy))
        }

        // ---- notification parsing (pure, synchronous) ----

        /**
         * Decodes a vcp call-params string into JSON (token/endpoints/ice_servers/
         * user_id). Pass a conversation id to also get `ws2_url`; pass "" to skip
         * it. Returns null if the vcp can't be decoded.
         */
        fun decodeVcp(vcp: String, conversationId: String = ""): String? {
            Native.ensureLoaded()
            return Native.decodeVcp(vcp, conversationId)
        }

        /**
         * Parses a ws2 `connection` notification (raw JSON) into
         * `{topology,is_sfu,participants,ice_servers[,peer]}`. Pass your calls
         * user id to get `peer` filled; pass null to skip it.
         */
        fun parseConnection(notificationJson: String, myUserId: Long? = null): String {
            Native.ensureLoaded()
            return Native.parseConnection(notificationJson, myUserId ?: 0, myUserId != null)
        }

        /**
         * Parses a ws2 `transmitted-data` notification (raw JSON) into
         * `{kind:"sdp",...}` or `{kind:"candidate",...}`. Returns null when it
         * carries neither.
         */
        fun parseTransmittedData(notificationJson: String): String? {
            Native.ensureLoaded()
            return Native.parseTransmittedData(notificationJson)
        }
    }

    // ---- suspending facade (all return response JSON) ----

    /** Accepts the incoming call. */
    suspend fun accept(): String = io { Native.callAccept(handle) }

    /** Ends the call with [reason]. */
    suspend fun hangup(reason: String): String = io { Native.callHangup(handle, reason) }

    /** Sends an SDP offer/answer to a participant. */
    suspend fun transmitSdp(participantId: Long, type: String, sdp: String): String =
        io { Native.callTransmitSdp(handle, participantId, type, sdp) }

    /** Sends an ICE candidate to a participant. */
    suspend fun transmitCandidate(
        participantId: Long,
        candidate: String,
        sdpMid: String,
        sdpMlineIndex: Long,
    ): String = io { Native.callTransmitCandidate(handle, participantId, candidate, sdpMid, sdpMlineIndex) }

    /** Updates the audio/video/screen flags. */
    suspend fun changeMedia(audio: Boolean, video: Boolean, screen: Boolean): String =
        io { Native.callChangeMedia(handle, audio, video, screen) }

    /** Sends a raw command with a JSON object of extra fields (empty for none). */
    suspend fun sendCommand(command: String, extraJson: String = ""): String =
        io { Native.callSendCommand(handle, command, extraJson) }

    /**
     * Waits up to [timeoutSeconds] for the next ws2 notification (raw JSON). A
     * negative timeout blocks forever; `null` means the wait timed out.
     */
    suspend fun nextNotification(timeoutSeconds: Double = -1.0): String? =
        io { Native.callNextNotification(handle, timeoutMillis(timeoutSeconds)) }

    /**
     * A cold [Flow] of ws2 notifications (raw JSON), polled on [Dispatchers.IO]
     * with [pollIntervalSeconds] waits so it observes cancellation. Ends when the
     * socket drops or the collector's scope is cancelled.
     */
    fun notifications(pollIntervalSeconds: Double = 1.0): Flow<String> = flow {
        while (currentCoroutineContext().isActive && isConnected) {
            val n = Native.callNextNotification(handle, timeoutMillis(pollIntervalSeconds)) ?: continue
            emit(n)
        }
    }.flowOn(Dispatchers.IO)

    /** Whether the ws2 socket is still up. */
    val isConnected: Boolean
        get() = Native.callIsConnected(handle)

    /** Hangs up the ws2 socket and frees the client. Idempotent. */
    override fun close() {
        if (closed.compareAndSet(false, true)) {
            Native.callClose(handle)
        }
    }

    @Suppress("removal")
    protected fun finalize() = close()

    private suspend inline fun <T> io(crossinline body: () -> T): T =
        withContext(Dispatchers.IO) { body() }
}
