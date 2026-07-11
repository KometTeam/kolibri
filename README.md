# kolibri

*[English version](README.en.md)*

Переносимое ядро клиента бинарного мессенджер-протокола на Rust: собственный
фрейминг поверх постоянного TLS-сокета, полезная нагрузка в MessagePack, сжатие
LZ4/Zstd, полная машина сессии (handshake, keepalive, реконнект), загрузка медиа
на CDN и сигнализация звонков (ws2). Протокол пишется **один раз на Rust**, а
тонкие обёртки дают его Python, Dart и любому хосту через C ABI.

Ядро не зависит от UI и платформы — на входе байты, на выходе байты.

## Структура

| Крейт | Что внутри |
|-------|------------|
| [`kolibri-net`](kolibri-net) | Rust-ядро: packet-кодек, фрейминг, сжатие, async TLS-транспорт, машина сессии, anti-spoof fingerprint, загрузка медиа, звонки (vcp + ws2-сигналинг). |
| [`kolibri-py`](kolibri-py) | Python-биндинги (pyo3/maturin): синхронный `Session`, звонки, загрузки, нативные dict'ы вместо байтов. |
| [`kolibri-dart`](kolibri-dart) | Dart/Flutter-биндинги (`flutter_rust_bridge`): async `Future` + пуши/прогресс через `Stream`. |

Идея: session-машина (handshake, ping, реконнект) и разбор протокола живут в
ядре, поэтому каждая обёртка получает их бесплатно.

## Возможности

- **Транспорт** — постоянный TLS-сокет (tokio + rustls), мультиплекс запрос/ответ
  по seq, broadcast-поток серверных пушей, авто-реконнект с экспоненциальным
  backoff.
- **Протокол** — 10-байтный бинарный заголовок, MessagePack, сжатие LZ4-block
  (исходящее) со сниффингом Zstd/LZ4-frame/LZ4-block на входящем.
- **Сессия** — handshake (opcode 6) из device-полей, keepalive-ping, состояния
  `disconnected/connecting/connected/online`.
- **Авторизация** — запрос кода → verify → login; anti-spoof fingerprint
  (3×SHA-256), дайджесты передаёт хост (не зашиты).
- **Спуф устройства** — все поля `userAgent` handshake задаются хостом; готовые
  пресеты реальных устройств. Медиа-UA автоматически выводится из этих же полей.
- **Медиа** — заливка на CDN: файл (одиночный POST), фото (multipart),
  видео (параллельные докачиваемые чанки), с колбэками прогресса.
- **Звонки** — разбор `vcp`, ws2-сигналинг (SDP/ICE/accept/hangup),
  типизированный разбор входящих нотификаций. Сам WebRTC-медиастек — на хосте.

## Быстрый старт

**Rust:**
```rust
use kolibri_net::{Session, SessionConfig, ClientConfig, HandshakeConfig};

let session = Session::new(SessionConfig::new(
    ClientConfig::new("host.example", 443),
    handshake, // device-поля
));
let info = session.connect().await?;                 // handshake
let resp = session.request(opcode, &msgpack).await?; // запрос/ответ
```

**Python:**
```python
import kolibri

s = kolibri.Session("host.example", 443)  # + device-kwargs при желании
info = s.connect()                        # handshake -> online
resp = s.request(opcode, {"key": "value"})  # dict внутрь -> dict наружу
```

**Dart:**
```dart
import 'package:kolibri/kolibri.dart';

await initKolibri(libraryPath: '.../libkolibri_dart.dylib'); // на Flutter не нужно
final s = openSession(host: 'host.example');
final info = await s.connect();
final resp = await s.request(opcode: 64, payload: msgpackBytes);
```

## Сборка и тесты

**Rust-ядро:**
```bash
cargo test              # протокол + транспорт + сессия + медиа + звонки
cargo clippy
cargo build --no-default-features   # чистый кодек без tokio/rustls
```

**Python:**
```bash
cd kolibri-py
python -m venv ../.venv && source ../.venv/bin/activate && pip install maturin
maturin develop         # на свежем Python + abi3: PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1
python examples/handshake.py
```

**Dart:**
```bash
cd kolibri-dart
dart pub get
flutter_rust_bridge_codegen generate
dart run build_runner build --delete-conflicting-outputs
cargo build --manifest-path rust/Cargo.toml
dart run example/handshake.dart
```

## Флаги сборки (feature-гейты)

- `transport` (по умолчанию вкл) — async-транспорт (tokio + rustls). Выключаешь —
  остаётся чистый протокол-кодек без сети.
- `calls` (по умолчанию вкл) — звонки (ws2-сигналинг, WebSocket). Кому звонки не
  нужны, отключает и не тянет WebSocket в бинарь.

## Пример: музыкальный автоответчик

`kolibri-py/examples/call_bot.py` — бот логинится, ждёт входящий звонок,
авто-принимает и играет в него аудио-трек (`.opus`/`.mp3`/`.wav`/`.webm`).
Сигнализация через kolibri, WebRTC-медиа через [aiortc](https://github.com/aiortc/aiortc).

```bash
cd kolibri-py
KOLIBRI_TRACK=track.opus KOLIBRI_PHONE=+7XXXXXXXXXX \
  ../.venv/bin/python examples/call_bot.py
```

> **Звонки:** держи VPN выключенным — он ломает UDP-путь медиа (ICE проходит, а
> DTLS через NAT/VPN не устанавливается). Для машины за NAT надёжнее публичный IP.

## Статус

Протокол-ядро, транспорт, машина сессии, медиа-загрузки и звонки готовы и
проверены вживую против боевого сервера (полный флоу авторизации, логин,
загрузка, звонок с медиа). Python- и Dart-обёртки работают и гоняют одно ядро.

## Лицензия

Двойная, на выбор:

- Apache License 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT ([LICENSE-MIT](LICENSE-MIT))

Любой вклад, отправленный в проект, лицензируется так же, без дополнительных
условий.
