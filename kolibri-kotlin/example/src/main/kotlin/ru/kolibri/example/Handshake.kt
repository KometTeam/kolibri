package ru.kolibri.example

import kotlinx.coroutines.flow.take
import kotlinx.coroutines.runBlocking
import ru.kolibri.Config
import ru.kolibri.Session
import ru.kolibri.WireEvent

/**
 * Connects, prints the handshake, sends one request, and drains a few pushes.
 *
 * Run with:  gradle :example:run --args "api.oneme.ru"
 * (build the native lib first: ./build-rust.sh)
 */
fun main(args: Array<String>) = runBlocking {
    val host = args.firstOrNull() ?: "api.oneme.ru"

    val config = Config(
        host = host,
        onWire = { e: WireEvent ->
            println("${e.direction} ${e.command} op=${e.opcode} ${e.json}")
        },
    )

    Session.open(config).use { session ->
        val info = session.connect()
        println("handshake -> $info")
        println("state=${session.state} ua=${session.userAgent}")

        // JSON in, JSON out; no msgpack library needed.
        val resp = session.requestJson(opcode = 64, jsonIn = "{}")
        println("response -> $resp")

        session.pushes().take(3).collect { push ->
            println("push op=${push.opcode} ${push.json}")
        }
    }
}
