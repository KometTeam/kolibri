"""Full phone-auth flow over asyncio: sends a REAL SMS, then verifies the code.

    python examples/auth_async.py +7XXXXXXXXXX
"""

import asyncio
import sys

import kolibri

# opcodes, see kolibri-net protocol::opcodes
AUTH_REQUEST = 17
AUTH = 18

# device_id must match between the handshake and the anti-spoof fingerprint
DEVICE_ID = "kolibri-rs-device"


async def main() -> None:
    if len(sys.argv) < 2:
        sys.exit("usage: python examples/auth_async.py +7XXXXXXXXXX")

    digits = "".join(c for c in sys.argv[1] if c.isdigit())
    phone = "+" + digits

    s = kolibri.AsyncSession("api.oneme.ru", 443, device_id=DEVICE_ID)
    info = await s.connect()
    print("online, calls_seed:", info["calls_seed"])

    mode = kolibri.auth_mode(info["calls_seed"], DEVICE_ID)
    resp = await s.request(
        AUTH_REQUEST,
        {"phone": phone, "type": "START_AUTH", "language": "ru", "mode": mode},
    )
    token = resp["token"]
    print("code sent. temp token:", token[:24], "…")

    # input() is blocking; run it off the event loop so the session keeps ticking
    code = (await asyncio.to_thread(input, "Enter SMS code: ")).strip()
    result = await s.request(
        AUTH,
        {"token": token, "verifyCode": code, "authTokenType": "CHECK_CODE"},
    )
    login = result.get("tokenAttrs", {}).get("LOGIN", {}).get("token")
    profile = result.get("profile", {}).get("contact", {})
    print("✓ logged in")
    print("  login token:", (login or "")[:24], "…")
    print("  profile    :", profile.get("names"), profile.get("phone"))

    s.disconnect()


if __name__ == "__main__":
    asyncio.run(main())
