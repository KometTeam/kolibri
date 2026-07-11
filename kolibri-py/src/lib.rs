//! Python bindings for `kolibri-net`. Exposes a synchronous `Session` that owns a
//! tokio runtime internally; MessagePack payloads are converted to/from native
//! Python dicts/lists so callers never touch bytes.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use kolibri_net::{
    ClientConfig, HandshakeConfig, Packet, Session as NetSession, SessionConfig, SessionState,
    TransportError, UserAgent,
};
use pyo3::exceptions::{PyRuntimeError, PyTimeoutError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{
    PyBool, PyBytes, PyDict, PyFloat, PyInt, PyList, PyString, PyTuple,
};
use rmpv::Value;
use tokio::runtime::Runtime;
use tokio::sync::broadcast;

/// A connected Komet session. Blocking API: each method drives the internal
/// tokio runtime to completion.
#[pyclass]
struct Session {
    rt: Arc<Runtime>,
    inner: Arc<NetSession>,
    push_rx: Mutex<broadcast::Receiver<Packet>>,
}

#[pymethods]
impl Session {
    #[new]
    #[pyo3(signature = (
        host,
        port = 443,
        device_id = "kolibri-rs-device",
        instance_id = "kolibri-rs-instance",
        app_version = "26.20.2",
        build_number = 6758,
        device_type = "ANDROID",
        os_version = "Android 14",
        timezone = "Europe/Moscow",
        screen = "420dpi 420dpi 1080x2340",
        push_device_type = "GCM",
        arch = "arm64-v8a",
        locale = "ru",
        device_name = "Rust",
        device_locale = "ru",
        client_session_id = 1_700_000_000i64,
        ping_interval_secs = 10u64,
        auto_reconnect = true,
        insecure_tls = false
    ))]
    #[allow(clippy::too_many_arguments)]
    fn new(
        host: &str,
        port: u16,
        device_id: &str,
        instance_id: &str,
        app_version: &str,
        build_number: i64,
        device_type: &str,
        os_version: &str,
        timezone: &str,
        screen: &str,
        push_device_type: &str,
        arch: &str,
        locale: &str,
        device_name: &str,
        device_locale: &str,
        client_session_id: i64,
        ping_interval_secs: u64,
        auto_reconnect: bool,
        insecure_tls: bool,
    ) -> PyResult<Self> {
        let user_agent = UserAgent {
            device_type: device_type.to_string(),
            app_version: app_version.to_string(),
            os_version: os_version.to_string(),
            timezone: timezone.to_string(),
            screen: screen.to_string(),
            push_device_type: push_device_type.to_string(),
            arch: arch.to_string(),
            locale: locale.to_string(),
            build_number,
            device_name: device_name.to_string(),
            device_locale: device_locale.to_string(),
        };
        let handshake = HandshakeConfig {
            instance_id: instance_id.to_string(),
            device_id: device_id.to_string(),
            client_session_id,
            user_agent,
        };
        let mut client = ClientConfig::new(host, port);
        client.insecure_tls = insecure_tls;
        let mut config = SessionConfig::new(client, handshake);
        config.ping_interval = Duration::from_secs(ping_interval_secs);
        config.auto_reconnect = auto_reconnect;

        let rt = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))?,
        );
        let inner = Arc::new(NetSession::new(config));
        let push_rx = Mutex::new(inner.subscribe());

        Ok(Self { rt, inner, push_rx })
    }

    /// Connect and perform the sessionInit handshake. Returns a dict with
    /// `calls_seed`, `device_name`, and the full `payload`.
    fn connect(&self, py: Python<'_>) -> PyResult<PyObject> {
        let rt = self.rt.clone();
        let inner = self.inner.clone();
        let info = py
            .allow_threads(move || rt.block_on(inner.connect()))
            .map_err(to_pyerr)?;

        let dict = PyDict::new(py);
        dict.set_item("calls_seed", info.calls_seed)?;
        dict.set_item("device_name", info.device_name)?;
        dict.set_item("payload", value_to_py(py, &info.payload)?)?;
        Ok(dict.into_any().unbind())
    }

    /// Send a request and return the decoded response payload. Raises on a
    /// server error packet or timeout.
    fn request(&self, py: Python<'_>, opcode: u16, payload: &Bound<'_, PyAny>) -> PyResult<PyObject> {
        let bytes = encode_value(&py_to_value(payload)?);
        let rt = self.rt.clone();
        let inner = self.inner.clone();
        let packet = py
            .allow_threads(move || rt.block_on(inner.request(opcode, &bytes)))
            .map_err(to_pyerr)?;
        let value = packet
            .value()
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok(value_to_py(py, &value)?.unbind())
    }

    /// Fire-and-forget send; returns the assigned sequence number.
    fn send(&self, opcode: u16, payload: &Bound<'_, PyAny>) -> PyResult<u16> {
        let bytes = encode_value(&py_to_value(payload)?);
        self.inner.send(opcode, &bytes).map_err(to_pyerr)
    }

    /// Wait for the next server push. Returns a dict `{opcode, payload}` or None
    /// on timeout.
    #[pyo3(signature = (timeout_secs = None))]
    fn next_push(&self, py: Python<'_>, timeout_secs: Option<f64>) -> PyResult<Option<PyObject>> {
        let rt = self.rt.clone();
        let result = py.allow_threads(move || {
            let mut rx = self.push_rx.lock().unwrap();
            rt.block_on(async {
                match timeout_secs {
                    Some(t) => tokio::time::timeout(Duration::from_secs_f64(t), rx.recv())
                        .await
                        .ok()
                        .and_then(|r| r.ok()),
                    None => rx.recv().await.ok(),
                }
            })
        });

        match result {
            Some(packet) => {
                let dict = PyDict::new(py);
                dict.set_item("opcode", packet.opcode)?;
                let value = packet.value().unwrap_or(Value::Nil);
                dict.set_item("payload", value_to_py(py, &value)?)?;
                Ok(Some(dict.into_any().unbind()))
            }
            None => Ok(None),
        }
    }

    /// Current session state: "disconnected" | "connecting" | "connected" | "online".
    fn state(&self) -> &'static str {
        match self.inner.state() {
            SessionState::Disconnected => "disconnected",
            SessionState::Connecting => "connecting",
            SessionState::Connected => "connected",
            SessionState::Online => "online",
        }
    }

    /// Stop the session and disable auto-reconnect.
    fn disconnect(&self) {
        self.inner.disconnect();
    }
}

fn to_pyerr(e: TransportError) -> PyErr {
    match e {
        TransportError::Timeout => PyTimeoutError::new_err("request timed out"),
        other => PyRuntimeError::new_err(other.to_string()),
    }
}

fn encode_value(value: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    rmpv::encode::write_value(&mut out, value).expect("in-memory msgpack write cannot fail");
    out
}

/// Convert a Python object into a MessagePack value. bool is checked before int
/// (Python bool is a subclass of int).
fn py_to_value(obj: &Bound<'_, PyAny>) -> PyResult<Value> {
    if obj.is_none() {
        return Ok(Value::Nil);
    }
    if let Ok(b) = obj.downcast::<PyBool>() {
        return Ok(Value::Boolean(b.is_true()));
    }
    if let Ok(i) = obj.downcast::<PyInt>() {
        if let Ok(v) = i.extract::<i64>() {
            return Ok(Value::from(v));
        }
        if let Ok(v) = i.extract::<u64>() {
            return Ok(Value::from(v));
        }
        return Err(PyValueError::new_err("integer out of 64-bit range"));
    }
    if let Ok(f) = obj.downcast::<PyFloat>() {
        return Ok(Value::from(f.value()));
    }
    if let Ok(s) = obj.downcast::<PyString>() {
        return Ok(Value::from(s.extract::<String>()?));
    }
    if let Ok(b) = obj.downcast::<PyBytes>() {
        return Ok(Value::Binary(b.as_bytes().to_vec()));
    }
    if let Ok(list) = obj.downcast::<PyList>() {
        let mut arr = Vec::with_capacity(list.len());
        for item in list.iter() {
            arr.push(py_to_value(&item)?);
        }
        return Ok(Value::Array(arr));
    }
    if let Ok(tuple) = obj.downcast::<PyTuple>() {
        let mut arr = Vec::with_capacity(tuple.len());
        for item in tuple.iter() {
            arr.push(py_to_value(&item)?);
        }
        return Ok(Value::Array(arr));
    }
    if let Ok(dict) = obj.downcast::<PyDict>() {
        let mut map = Vec::with_capacity(dict.len());
        for (k, v) in dict.iter() {
            map.push((py_to_value(&k)?, py_to_value(&v)?));
        }
        return Ok(Value::Map(map));
    }
    Err(PyTypeError::new_err(format!(
        "unsupported type for MessagePack: {}",
        obj.get_type().name()?
    )))
}

/// Convert a MessagePack value into a Python object.
fn value_to_py<'py>(py: Python<'py>, value: &Value) -> PyResult<Bound<'py, PyAny>> {
    Ok(match value {
        Value::Nil => py.None().into_bound(py),
        Value::Boolean(b) => PyBool::new(py, *b).to_owned().into_any(),
        Value::Integer(i) => {
            if let Some(v) = i.as_i64() {
                v.into_pyobject(py).unwrap().into_any()
            } else if let Some(v) = i.as_u64() {
                v.into_pyobject(py).unwrap().into_any()
            } else {
                return Err(PyValueError::new_err("integer out of range"));
            }
        }
        Value::F32(f) => (*f as f64).into_pyobject(py).unwrap().into_any(),
        Value::F64(f) => (*f).into_pyobject(py).unwrap().into_any(),
        Value::String(s) => match s.as_str() {
            Some(text) => text.into_pyobject(py).unwrap().into_any(),
            None => PyBytes::new(py, s.as_bytes()).into_any(),
        },
        Value::Binary(b) => PyBytes::new(py, b).into_any(),
        Value::Array(arr) => {
            let list = PyList::empty(py);
            for v in arr {
                list.append(value_to_py(py, v)?)?;
            }
            list.into_any()
        }
        Value::Map(map) => {
            let dict = PyDict::new(py);
            for (k, v) in map {
                dict.set_item(value_to_py(py, k)?, value_to_py(py, v)?)?;
            }
            dict.into_any()
        }
        Value::Ext(_, data) => PyBytes::new(py, data).into_any(),
    })
}

/// Compute the 96-byte anti-spoof `mode` fingerprint for the authRequest
/// payload: three SHA-256 hashes of (digest || int64_be(calls_seed) ||
/// utf8(device_id)) concatenated.
#[pyfunction]
fn auth_mode<'py>(py: Python<'py>, calls_seed: i64, device_id: &str) -> Bound<'py, PyBytes> {
    use sha2::{Digest, Sha256};

    const SIGNATURE: &str = "1684414033eb263e2c615f8b7df5ed8793850a07656304997fbf07e9e21e1e93";
    const SO: &str = "90e2fb8745b17b42a10182f8d8ac590e3fca5b311e2ce2d5144fa2c18cb3090d";
    const DEX: &str = "0a6265f6e5d8231b9cba641f8c40475e6f3baeb06ed41b804b9bf7307aa4214e";

    fn hex(s: &str) -> Vec<u8> {
        (0..s.len() / 2)
            .map(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
            .collect()
    }
    fn digest(prefix: &[u8], seed: &[u8], device: &[u8]) -> Vec<u8> {
        let mut h = Sha256::new();
        h.update(prefix);
        h.update(seed);
        h.update(device);
        h.finalize().to_vec()
    }

    let seed = calls_seed.to_be_bytes();
    let device = device_id.as_bytes();
    let mut out = Vec::with_capacity(96);
    out.extend(digest(&hex(SIGNATURE), &seed, device));
    out.extend(digest(&hex(DEX), &seed, device));
    out.extend(digest(&hex(SO), &seed, device));
    PyBytes::new(py, &out)
}

#[pymodule]
fn kolibri(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Session>()?;
    m.add_function(wrap_pyfunction!(auth_mode, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
