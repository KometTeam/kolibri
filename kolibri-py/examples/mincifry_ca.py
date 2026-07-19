"""Handshake against api2.oneme.ru (Минцифры-signed chain).

    python examples/mincifry_ca.py
"""

import kolibri

# set once at startup, before any Session/upload/call
kolibri.set_trust_mincifry_ca(True)
print("trust Минцифры CA:", kolibri.trust_mincifry_ca())

s = kolibri.Session("api2.oneme.ru", 443)
print("state:", s.state())

info = s.connect()
print("state:", s.state())
print("calls_seed :", info["calls_seed"])
print("location   :", info["payload"].get("location"))

s.disconnect()
print("state:", s.state())
