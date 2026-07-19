import asyncio
import sys

import kolibri


async def main() -> None:
    dev = "spoof-0001"
    phone = sys.argv[1] if len(sys.argv) > 1 else "+70000000000"
    s = kolibri.AsyncSession(
        "api.oneme.ru", 443, device_id=dev, device_name="Google Pixel 8 Pro"
    )
    info = await s.connect()
    seed = info["calls_seed"]
    mode = kolibri.auth_mode(seed, dev)
    r = await s.request(
        17, {"phone": phone, "type": "START_AUTH", "language": "ru", "mode": mode}
    )
    print("token:", r["token"])
    s.disconnect()


if __name__ == "__main__":
    asyncio.run(main())
