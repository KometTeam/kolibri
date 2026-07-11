//! MessagePack -> JSON, for logs. lossy: Binary/Ext turn into base64 strings,
//! non-string map keys get stringified (JSON holds neither) — so it reads but
//! won't round-trip back to the same msgpack.

use base64::Engine;
use rmpv::Value;
use serde_json::{Map, Number, Value as Json};

/// a decoded MessagePack value as JSON.
pub fn value_to_json(value: &Value) -> Json {
    match value {
        Value::Nil => Json::Null,
        Value::Boolean(b) => Json::Bool(*b),
        Value::Integer(i) => integer_to_json(i),
        Value::F32(f) => Number::from_f64(*f as f64).map_or(Json::Null, Json::Number),
        Value::F64(f) => Number::from_f64(*f).map_or(Json::Null, Json::Number),
        Value::String(s) => Json::String(utf8_lossy(s)),
        Value::Binary(bytes) => Json::String(base64_encode(bytes)),
        Value::Array(items) => Json::Array(items.iter().map(value_to_json).collect()),
        Value::Map(entries) => {
            let mut map = Map::with_capacity(entries.len());
            for (k, v) in entries {
                map.insert(map_key(k), value_to_json(v));
            }
            Json::Object(map)
        }
        Value::Ext(tag, data) => {
            let mut map = Map::with_capacity(2);
            map.insert("$ext".to_string(), Json::Number((*tag).into()));
            map.insert("data".to_string(), Json::String(base64_encode(data)));
            Json::Object(map)
        }
    }
}

fn integer_to_json(i: &rmpv::Integer) -> Json {
    if let Some(u) = i.as_u64() {
        Json::Number(u.into())
    } else if let Some(s) = i.as_i64() {
        Json::Number(s.into())
    } else {
        Json::Null
    }
}

fn map_key(value: &Value) -> String {
    match value {
        Value::String(s) => utf8_lossy(s),
        Value::Integer(i) => integer_to_json(i).to_string(),
        Value::Boolean(b) => b.to_string(),
        Value::Nil => "null".to_string(),
        other => value_to_json(other).to_string(),
    }
}

fn utf8_lossy(s: &rmpv::Utf8String) -> String {
    match s.as_str() {
        Some(text) => text.to_string(),
        None => String::from_utf8_lossy(s.as_bytes()).into_owned(),
    }
}

fn base64_encode(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::value_to_json;
    use rmpv::Value;

    #[test]
    fn renders_scalars_and_nested_maps() {
        let value = Value::Map(vec![
            (Value::from("id"), Value::from(42u64)),
            (Value::from("ok"), Value::from(true)),
            (
                Value::from("tags"),
                Value::Array(vec![Value::from("a"), Value::from("b")]),
            ),
        ]);
        let json = value_to_json(&value);
        assert_eq!(json["id"], 42);
        assert_eq!(json["ok"], true);
        assert_eq!(json["tags"][1], "b");
    }

    #[test]
    fn binary_becomes_base64_and_int_keys_stringify() {
        let value = Value::Map(vec![
            (Value::from(7), Value::Binary(vec![0xDE, 0xAD])),
        ]);
        let json = value_to_json(&value);
        assert_eq!(json["7"], "3q0=");
    }
}
