package ru.kolibri

import kotlin.test.Test
import kotlin.test.assertEquals
import kotlin.test.assertNull
import kotlin.test.assertTrue

/**
 * Exercises the pure (non-networking) FFI paths, which is all we can drive
 * without a live server. Requires the native lib on java.library.path; build it
 * first with ../build-rust.sh (the Gradle test task points there).
 */
class PureTest {
    @Test
    fun fingerprintIs96Bytes() {
        val fp = Auth.fingerprint(callsSeed = 123, deviceId = "device-abc")
        assertEquals(96, fp.size)
    }

    @Test
    fun fingerprintIsDeterministic() {
        val a = Auth.fingerprint(callsSeed = 7, deviceId = "dev")
        val b = Auth.fingerprint(callsSeed = 7, deviceId = "dev")
        assertTrue(a.contentEquals(b))
    }

    @Test
    fun decodeGarbageVcpIsNull() {
        assertNull(Call.decodeVcp("not-a-real-vcp"))
    }

    @Test
    fun parseTransmittedDataOfEmptyIsNull() {
        assertNull(Call.parseTransmittedData("{}"))
    }

    @Test
    fun parseConnectionReturnsJson() {
        val json = Call.parseConnection("{}", myUserId = 42)
        assertTrue(json.contains("topology"))
        assertTrue(json.contains("participants"))
    }
}
