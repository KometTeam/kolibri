# Проблема: асимметрия JSON-моста по бинарю

**Статус:** fixed (`kolibri-dart` + `kolibri-go`) · **Компонент:** `kolibri-net/src/protocol/json.rs`, биндинги `kolibri-dart`/`kolibri-go`
**Блокировало:** миграцию Komet на JSON-мост без внешнего `msgpack_dart`

## Сделано (2026-07-11)

Реализован фикс из раздела ниже. `value_to_json` (логи) остался плоским;
добавлен `value_to_json_tagged` (`Binary` → `{"$bin":...}`) + `Packet::json_tagged()`.
`kolibri-dart` data-плоскость (`connect().payload_json`, `pushes()`, выход
`request_json`) переведена на тегированную версию; `wire_json` (лог) — плоский.
В `kolibri.dart` добавлен `_unescapeBinary`, применён в `requestMap`/`_asMap`
(за ним `payloadMap`/`pushesMap`). Round-trip-тест в ядре зелёный
(`Binary` → tagged → `json_to_value` → тот же `Binary`; плоский по-прежнему base64).

**Go тоже симметризован:** `kolibri-go/rust` (`connect_json`, `request_json`,
`next_push_json`) на `value_to_json_tagged`/`json_tagged` (wire-tap лог —
плоский); в `kolibri.go` добавлен `unescapeBinary`, применён в `jsonToMap` (за
ним `Connect`/`NextPush`/`RequestMap`). Go round-trip-тест зелёный
(`[]byte` → escape → JSON → `jsonToMap` → тот же `[]byte`).

**Python** не затронут: там msgpack→dict через `value_to_py`, бинарь и так
нативные `bytes` (JSON-моста в data-плоскости нет).

## Суть

JSON-мост ядра лосслесс в одну сторону и лоссовый в другую по типу `Binary`.

- **Вход** (`json_to_value`, host → wire): `{"$bin":"<b64>"}` → `Value::Binary`. Тег сохраняется, тип восстанавливается.
- **Выход** (`value_to_json`, wire → host): `Value::Binary` → **нетегированная** base64-`String`. Тип теряется.

То есть по проводу `bin` → в Dart приходит обычная строка, а не бинарь. Обратно к `Uint8List` это уже не поднять — в JSON нет признака, что строка была бинарём.

Для сравнения `Ext` симметричен: выход даёт `{"$ext":tag,"data":"<b64>"}`, вход это же и восстанавливает. Асимметрия только у `Binary`.

## Где в коде

`kolibri-net/src/protocol/json.rs`:

```rust
// выход — теряет тип:
Value::Binary(bytes) => Json::String(base64_encode(bytes)),

// вход — восстанавливает тип:
fn tagged_binary(obj: &Map<String, Json>) -> Option<Value> {
    if obj.len() == 1 {
        if let Some(Json::String(b64)) = obj.get("$bin") {
            return base64_decode(b64).map(Value::Binary);
        }
    }
    ...
}
```

Комментарий в шапке модуля это фиксирует как осознанное решение — «lossy: Binary/Ext turn into base64 strings … won't round-trip». Для логов это ок, для data-плоскости — нет.

## Влияние

Потребители, которые читают payload как JSON (`request_json`/`requestMap`, `HandshakeInfo.payload_json`, `PushEvent.payload_json` в `kolibri-dart`), получают ответный бинарь как `String`.

Конкретно ломается Komet: скачивание фото/видео/файлов читает `response.payload['content']` и проверяет `is Uint8List` / `is List<int>` — с JSON-мостом там оказывается base64-`String`, и проверки не проходят. Ровно из-за этого Komet не может отказаться от `msgpack_dart` и использовать JSON-мост как единственный кодек.

## Почему выход сделан лоссовым намеренно

Выход `value_to_json` обслуживает **логи** (wire-tap, `request_json`-для-отладки в Python/Dart/Go), где плоский base64 читается глазом и `{"$bin"}` только мешал бы. Ломать это поведение нельзя — надо добавить отдельный лосслесс-режим для data-плоскости, оставив плоский для логов.

## Фикс

Симметризовать выход, не трогая логовый путь.

1. **`json.rs`** — вынести тело в приватную `to_json(value, tag_binary: bool)`; `value_to_json` = `to_json(_, false)` (логи, как было), добавить `value_to_json_tagged` = `to_json(_, true)`, где `Binary` → `{"$bin":"<b64>"}`. `tag_binary` протянуть рекурсивно через `Array`/`Map`.
2. **`protocol/mod.rs`** — до-экспортировать `value_to_json_tagged`.
3. **`protocol/packet.rs`** — добавить `Packet::json_tagged()` рядом с `json()` (последнюю оставить плоской).
4. **`kolibri-dart/rust/src/api/session.rs`** — перевести `connect().payload_json`, `pushes()` и выход `request_json` на тегированную версию; `wire_json` (WireLogEvent) оставить плоским.
5. **`kolibri-dart/lib/kolibri.dart`** — добавить `_unescapeBinary` (обратное к `_escapeBinary`): `{"$bin":<b64>}` → `Uint8List`; применить в `requestMap` и `_asMap` (за ним `payloadMap`/`pushesMap`).

После этого JSON-мост лосслесс в обе стороны, и внешний msgpack-декодер на стороне хоста не нужен.

## Тест

Round-trip: `Value::Binary(x)` → `value_to_json_tagged` → `json_to_value` → снова `Value::Binary(x)`. Плюс проверить, что `value_to_json` (логовый) по-прежнему даёт плоскую base64-строку.
