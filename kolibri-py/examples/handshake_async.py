"""sessionInit handshake against the real server, no SMS (asyncio).

    python examples/handshake_async.py
"""

import asyncio

import kolibri


async def main() -> None:
    s = kolibri.AsyncSession("api.oneme.ru", 443)
    print("state:", s.state())

    info = await s.connect()
    print("state:", s.state())
    print("calls_seed :", info["calls_seed"])
    print("location   :", info["payload"].get("location"))
    print("countries  :", len(info["payload"].get("reg-country-code", [])))

    s.disconnect()
    print("state:", s.state())


if __name__ == "__main__":
    asyncio.run(main())
