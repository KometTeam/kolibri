//! JNI bindings over kolibri-net, for the Kotlin/Android binding. Mirrors the
//! Swift/Go C-ABI wrappers: a `Session` owns a tokio runtime and every network
//! call blocks until it completes. Handles cross the boundary as `jlong` (a
//! boxed raw pointer). Fallible calls throw `ru.kolibri.KolibriException` and
//! return a null/zero default; results come back as `String`, `byte[]`, or via
//! an `int[]` out-param.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use jni::objects::{GlobalRef, JByteArray, JIntArray, JObject, JString, JValue};
use jni::sys::{jboolean, jbyteArray, jint, jlong, jstring, JNI_FALSE, JNI_TRUE};
use jni::{JNIEnv, JavaVM};

use kolibri_net::auth::chat_cache_fingerprint;
use kolibri_net::calls::{
    parse_connection, parse_transmitted_data, ConnectionInfo, ConversationParams, TransmittedData,
    Ws2ClientInfo, Ws2Signaling,
};
use kolibri_net::protocol::value_to_json;
use kolibri_net::{
    ClientConfig, Direction, HandshakeConfig, Packet, ProxyConfig, Session, SessionConfig,
    SessionState, UserAgent, WireTap,
};
use serde_json::{json, Value as Json};
use tokio::runtime::Runtime;
use tokio::sync::broadcast;

const EXCEPTION: &str = "ru/kolibri/KolibriException";

/// A live protocol session backed by the Rust core.
struct KSession {
    rt: Arc<Runtime>,
    inner: Arc<Session>,
    push_rx: Mutex<broadcast::Receiver<Packet>>,
    proxy: Option<ProxyConfig>,
    /// Keeps the Kotlin wire-tap object alive for the session's lifetime.
    _wire: Option<GlobalRef>,
}

/// A ws2 signaling client.
struct KCall {
    rt: Arc<Runtime>,
    inner: Arc<Ws2Signaling>,
    notif_rx: Mutex<broadcast::Receiver<Json>>,
}

// ---- small JNI helpers ----

fn throw(env: &mut JNIEnv, msg: impl AsRef<str>) {
    let _ = env.throw_new(EXCEPTION, msg.as_ref());
}

/// Reads a `JString` into a Rust `String`; NULL becomes "".
fn jstr(env: &mut JNIEnv, s: &JString) -> String {
    if s.is_null() {
        return String::new();
    }
    env.get_string(s).map(|s| s.into()).unwrap_or_default()
}

/// Like `jstr`, but "" maps to `None` (a direct connection / absent value).
fn jstr_opt(env: &mut JNIEnv, s: &JString) -> Option<String> {
    let s = jstr(env, s);
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

fn jbytes(env: &mut JNIEnv, a: &JByteArray) -> Vec<u8> {
    if a.is_null() {
        return Vec::new();
    }
    env.convert_byte_array(a).unwrap_or_default()
}

/// Allocates a Java `byte[]` from `data`; throws + null on OOM.
fn to_jbytearray(env: &mut JNIEnv, data: &[u8]) -> jbyteArray {
    match env.byte_array_from_slice(data) {
        Ok(a) => a.into_raw(),
        Err(_) => {
            throw(env, "failed to allocate byte[]");
            std::ptr::null_mut()
        }
    }
}

/// Allocates a Java `String`; throws + null on failure.
fn to_jstring(env: &mut JNIEnv, s: &str) -> jstring {
    match env.new_string(s) {
        Ok(v) => v.into_raw(),
        Err(_) => {
            throw(env, "failed to allocate String");
            std::ptr::null_mut()
        }
    }
}

/// Writes `val` into element 0 of an `int[]` out-param (ignored if null/empty).
fn set_int0(env: &mut JNIEnv, arr: &JIntArray, val: i32) {
    if arr.is_null() {
        return;
    }
    let _ = env.set_int_array_region(arr, 0, &[val]);
}

/// Turns a `jlong` handle into a shared reference, throwing on a null handle.
unsafe fn session_ref<'a>(env: &mut JNIEnv, handle: jlong) -> Option<&'a KSession> {
    if handle == 0 {
        throw(env, "null session handle");
        None
    } else {
        Some(&*(handle as *const KSession))
    }
}

unsafe fn call_ref<'a>(env: &mut JNIEnv, handle: jlong) -> Option<&'a KCall> {
    if handle == 0 {
        throw(env, "null call handle");
        None
    } else {
        Some(&*(handle as *const KCall))
    }
}

// ===================================================================
// Session
// ===================================================================

#[no_mangle]
#[allow(clippy::too_many_arguments, non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_sessionNew<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    host: JString<'local>,
    port: jint,
    device_id: JString<'local>,
    instance_id: JString<'local>,
    app_version: JString<'local>,
    build_number: jlong,
    device_type: JString<'local>,
    os_version: JString<'local>,
    timezone: JString<'local>,
    screen: JString<'local>,
    push_device_type: JString<'local>,
    arch: JString<'local>,
    locale: JString<'local>,
    device_name: JString<'local>,
    device_locale: JString<'local>,
    client_session_id: jlong,
    ping_interval_secs: jlong,
    ping_interactive: jboolean,
    auto_reconnect: jboolean,
    insecure_tls: jboolean,
    proxy: JString<'local>,
    wire: JObject<'local>,
) -> jlong {
    let proxy = match jstr_opt(&mut env, &proxy) {
        Some(url) => match ProxyConfig::parse(&url) {
            Ok(p) => Some(p),
            Err(e) => {
                throw(&mut env, e);
                return 0;
            }
        },
        None => None,
    };

    let user_agent = UserAgent {
        device_type: jstr(&mut env, &device_type),
        app_version: jstr(&mut env, &app_version),
        os_version: jstr(&mut env, &os_version),
        timezone: jstr(&mut env, &timezone),
        screen: jstr(&mut env, &screen),
        push_device_type: jstr(&mut env, &push_device_type),
        arch: jstr(&mut env, &arch),
        locale: jstr(&mut env, &locale),
        build_number,
        device_name: jstr(&mut env, &device_name),
        device_locale: jstr(&mut env, &device_locale),
    };
    let handshake = HandshakeConfig {
        instance_id: jstr(&mut env, &instance_id),
        device_id: jstr(&mut env, &device_id),
        client_session_id,
        user_agent,
    };
    let mut client = ClientConfig::new(jstr(&mut env, &host), port as u16);
    client.insecure_tls = insecure_tls == JNI_TRUE;
    client.proxy = proxy.clone();
    let mut config = SessionConfig::new(client, handshake);
    config.ping_interval = Duration::from_secs(ping_interval_secs.max(0) as u64);
    config.ping_interactive = ping_interactive == JNI_TRUE;
    config.auto_reconnect = auto_reconnect == JNI_TRUE;

    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => Arc::new(rt),
        Err(e) => {
            throw(&mut env, e.to_string());
            return 0;
        }
    };

    // Build the wire tap: keep a global ref to the Kotlin object and re-attach
    // the calling (tokio) thread to the JVM for each packet.
    let mut wire_ref: Option<GlobalRef> = None;
    let tap: Option<WireTap> = if wire.is_null() {
        None
    } else {
        let global = match env.new_global_ref(&wire) {
            Ok(g) => g,
            Err(e) => {
                throw(&mut env, e.to_string());
                return 0;
            }
        };
        let vm = match env.get_java_vm() {
            Ok(vm) => Arc::new(vm),
            Err(e) => {
                throw(&mut env, e.to_string());
                return 0;
            }
        };
        wire_ref = Some(global.clone());
        Some(Arc::new(
            move |dir: Direction, cmd: u8, opcode: u16, seq: u16, payload: &[u8]| {
                dispatch_wire(&vm, &global, dir, cmd, opcode, seq, payload);
            },
        ) as WireTap)
    };

    let inner = Arc::new(Session::with_wire_tap(config, tap));
    let push_rx = Mutex::new(inner.subscribe());
    let session = Box::new(KSession {
        rt,
        inner,
        push_rx,
        proxy,
        _wire: wire_ref,
    });
    Box::into_raw(session) as jlong
}

/// Forwards one tapped packet to the Kotlin `WireTap.onPacket`.
fn dispatch_wire(
    vm: &JavaVM,
    cb: &GlobalRef,
    dir: Direction,
    cmd: u8,
    opcode: u16,
    seq: u16,
    payload: &[u8],
) {
    let mut env = match vm.attach_current_thread() {
        Ok(e) => e,
        Err(_) => return,
    };
    let direction: JObject = match env.new_string(dir.as_str()) {
        Ok(s) => s.into(),
        Err(_) => return,
    };
    let command: JObject = match env.new_string(cmd_label(dir, cmd)) {
        Ok(s) => s.into(),
        Err(_) => return,
    };
    let json: JObject = match env.new_string(payload_to_json(payload)) {
        Ok(s) => s.into(),
        Err(_) => return,
    };
    let _ = env.call_method(
        cb,
        "onPacket",
        "(Ljava/lang/String;Ljava/lang/String;IILjava/lang/String;)V",
        &[
            JValue::Object(&direction),
            JValue::Object(&command),
            JValue::Int(opcode as i32),
            JValue::Int(seq as i32),
            JValue::Object(&json),
        ],
    );
    // Swallow any exception the callback raised so we don't unwind into Rust.
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_sessionConnect<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
) -> jbyteArray {
    let s = match unsafe { session_ref(&mut env, handle) } {
        Some(s) => s,
        None => return std::ptr::null_mut(),
    };
    match s.rt.block_on(s.inner.connect()) {
        Ok(info) => {
            let mut buf = Vec::new();
            if rmpv::encode::write_value(&mut buf, &info.payload).is_err() {
                throw(&mut env, "failed to encode handshake payload");
                return std::ptr::null_mut();
            }
            to_jbytearray(&mut env, &buf)
        }
        Err(e) => {
            throw(&mut env, e.to_string());
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_sessionConnectJson<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
) -> jstring {
    let s = match unsafe { session_ref(&mut env, handle) } {
        Some(s) => s,
        None => return std::ptr::null_mut(),
    };
    match s.rt.block_on(s.inner.connect()) {
        Ok(info) => {
            let json = kolibri_net::protocol::value_to_json_tagged(&info.payload).to_string();
            to_jstring(&mut env, &json)
        }
        Err(e) => {
            throw(&mut env, e.to_string());
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_sessionRequest<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
    opcode: jint,
    payload: JByteArray<'local>,
) -> jbyteArray {
    let s = match unsafe { session_ref(&mut env, handle) } {
        Some(s) => s,
        None => return std::ptr::null_mut(),
    };
    let bytes = jbytes(&mut env, &payload);
    match s.rt.block_on(s.inner.request(opcode as u16, &bytes)) {
        Ok(packet) => to_jbytearray(&mut env, &packet.payload),
        Err(e) => {
            throw(&mut env, e.to_string());
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_sessionRequestJson<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
    opcode: jint,
    json_in: JString<'local>,
) -> jstring {
    let s = match unsafe { session_ref(&mut env, handle) } {
        Some(s) => s,
        None => return std::ptr::null_mut(),
    };
    let value: Json = match serde_json::from_str(&jstr(&mut env, &json_in)) {
        Ok(v) => v,
        Err(e) => {
            throw(&mut env, e.to_string());
            return std::ptr::null_mut();
        }
    };
    let mut payload = Vec::new();
    if rmpv::encode::write_value(&mut payload, &kolibri_net::protocol::json_to_value(&value)).is_err()
    {
        throw(&mut env, "failed to encode request payload");
        return std::ptr::null_mut();
    }
    match s.rt.block_on(s.inner.request(opcode as u16, &payload)) {
        Ok(packet) => match packet.json_tagged() {
            Ok(json) => to_jstring(&mut env, &json.to_string()),
            Err(e) => {
                throw(&mut env, e.to_string());
                std::ptr::null_mut()
            }
        },
        Err(e) => {
            throw(&mut env, e.to_string());
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_sessionSend<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
    opcode: jint,
    payload: JByteArray<'local>,
) -> jint {
    let s = match unsafe { session_ref(&mut env, handle) } {
        Some(s) => s,
        None => return 0,
    };
    let bytes = jbytes(&mut env, &payload);
    match s.inner.send(opcode as u16, &bytes) {
        Ok(seq) => seq as jint,
        Err(e) => {
            throw(&mut env, e.to_string());
            0
        }
    }
}

/// Blocks up to `timeout_ms` for the next push; writes its opcode into
/// `out_opcode[0]` and returns the msgpack payload, or null on timeout.
#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_sessionNextPush<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
    timeout_ms: jlong,
    out_opcode: JIntArray<'local>,
) -> jbyteArray {
    let s = match unsafe { session_ref(&mut env, handle) } {
        Some(s) => s,
        None => return std::ptr::null_mut(),
    };
    match recv_push(s, timeout_ms) {
        Some(p) => {
            set_int0(&mut env, &out_opcode, p.opcode as i32);
            to_jbytearray(&mut env, &p.payload)
        }
        None => std::ptr::null_mut(),
    }
}

/// like `sessionNextPush`, but returns the push payload as a JSON string.
#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_sessionNextPushJson<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
    timeout_ms: jlong,
    out_opcode: JIntArray<'local>,
) -> jstring {
    let s = match unsafe { session_ref(&mut env, handle) } {
        Some(s) => s,
        None => return std::ptr::null_mut(),
    };
    match recv_push(s, timeout_ms) {
        Some(p) => {
            set_int0(&mut env, &out_opcode, p.opcode as i32);
            to_jstring(&mut env, &payload_to_json_tagged(&p.payload))
        }
        None => std::ptr::null_mut(),
    }
}

fn recv_push(s: &KSession, timeout_ms: i64) -> Option<Packet> {
    let mut rx = s.push_rx.lock().unwrap();
    s.rt.block_on(async {
        if timeout_ms < 0 {
            rx.recv().await.ok()
        } else {
            tokio::time::timeout(Duration::from_millis(timeout_ms as u64), rx.recv())
                .await
                .ok()
                .and_then(|r| r.ok())
        }
    })
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_sessionState<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
) -> jint {
    match unsafe { session_ref(&mut env, handle) } {
        Some(s) => match s.inner.state() {
            SessionState::Disconnected => 0,
            SessionState::Connecting => 1,
            SessionState::Connected => 2,
            SessionState::Online => 3,
        },
        None => -1,
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_sessionPingInteractive<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
) -> jboolean {
    match unsafe { session_ref(&mut env, handle) } {
        Some(s) if s.inner.ping_interactive() => JNI_TRUE,
        _ => JNI_FALSE,
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_sessionSetPingInteractive<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
    interactive: jboolean,
) {
    if let Some(s) = unsafe { session_ref(&mut env, handle) } {
        s.inner.set_ping_interactive(interactive == JNI_TRUE);
    }
}

/// Trust the bundled Минцифры CA (socket, media, calls); off by default, set at startup.
#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_setTrustMincifryCa<'local>(
    _env: JNIEnv<'local>,
    _this: JObject<'local>,
    enabled: jboolean,
) {
    kolibri_net::set_trust_mincifry_ca(enabled == JNI_TRUE);
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_trustMincifryCa<'local>(
    _env: JNIEnv<'local>,
    _this: JObject<'local>,
) -> jboolean {
    if kolibri_net::trust_mincifry_ca() {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_sessionUserAgent<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
) -> jstring {
    match unsafe { session_ref(&mut env, handle) } {
        Some(s) => {
            let ua = s.inner.http_user_agent();
            to_jstring(&mut env, &ua)
        }
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_sessionDisconnect<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
) {
    if let Some(s) = unsafe { session_ref(&mut env, handle) } {
        s.inner.disconnect();
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_sessionFree<'local>(
    _env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
) {
    if handle != 0 {
        unsafe { drop(Box::from_raw(handle as *mut KSession)) };
    }
}

// ===================================================================
// Media uploads
// ===================================================================

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_uploadFile<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
    url: JString<'local>,
    data: JByteArray<'local>,
    filename: JString<'local>,
    out_status: JIntArray<'local>,
) -> jbyteArray {
    upload_single(&mut env, handle, url, data, filename, out_status, false)
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_uploadPhoto<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
    url: JString<'local>,
    data: JByteArray<'local>,
    filename: JString<'local>,
    out_status: JIntArray<'local>,
) -> jbyteArray {
    upload_single(&mut env, handle, url, data, filename, out_status, true)
}

fn upload_single(
    env: &mut JNIEnv,
    handle: jlong,
    url: JString,
    data: JByteArray,
    filename: JString,
    out_status: JIntArray,
    photo: bool,
) -> jbyteArray {
    let s = match unsafe { session_ref(env, handle) } {
        Some(s) => s,
        None => return std::ptr::null_mut(),
    };
    let url = jstr(env, &url);
    let filename = jstr(env, &filename);
    let bytes = jbytes(env, &data);
    let ua = s.inner.http_user_agent();
    let proxy = s.proxy.clone();
    let result = if photo {
        s.rt.block_on(kolibri_net::media::upload_photo(
            &url,
            &bytes,
            &filename,
            false,
            proxy.as_ref(),
            None,
            &ua,
        ))
    } else {
        s.rt.block_on(kolibri_net::media::upload_file(
            &url,
            &bytes,
            &filename,
            false,
            proxy.as_ref(),
            None,
            &ua,
        ))
    };
    match result {
        Ok(resp) => {
            set_int0(env, &out_status, resp.status as i32);
            to_jbytearray(env, &resp.body)
        }
        Err(e) => {
            throw(env, e.to_string());
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_uploadVideo<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
    url: JString<'local>,
    data: JByteArray<'local>,
    chunk_size: jint,
    concurrency: jint,
) -> jboolean {
    let s = match unsafe { session_ref(&mut env, handle) } {
        Some(s) => s,
        None => return JNI_FALSE,
    };
    let url = jstr(&mut env, &url);
    let bytes = jbytes(&mut env, &data);
    let proxy = s.proxy.clone();
    let result = s.rt.block_on(kolibri_net::media::upload_video(
        &url,
        bytes,
        chunk_size.max(0) as usize,
        concurrency.max(0) as usize,
        false,
        proxy,
        None,
    ));
    match result {
        Ok(ok) => {
            if ok {
                JNI_TRUE
            } else {
                JNI_FALSE
            }
        }
        Err(e) => {
            throw(&mut env, e.to_string());
            JNI_FALSE
        }
    }
}

// ===================================================================
// Auth fingerprint
// ===================================================================

#[no_mangle]
#[allow(clippy::too_many_arguments, non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_authMode<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    signature: JByteArray<'local>,
    dex: JByteArray<'local>,
    so: JByteArray<'local>,
    calls_seed: jlong,
    device_id: JString<'local>,
) -> jbyteArray {
    let sig = jbytes(&mut env, &signature);
    let dex = jbytes(&mut env, &dex);
    let so = jbytes(&mut env, &so);
    let device_id = jstr(&mut env, &device_id);
    let fp = chat_cache_fingerprint(&sig, &dex, &so, calls_seed, &device_id);
    to_jbytearray(&mut env, &fp)
}

// ===================================================================
// Calls: notification parsing (pure)
// ===================================================================

/// vcp -> JSON, or null if it can't be decoded.
#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_decodeVcp<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    vcp: JString<'local>,
    conversation_id: JString<'local>,
) -> jstring {
    let vcp = jstr(&mut env, &vcp);
    let cid = jstr_opt(&mut env, &conversation_id);
    match ConversationParams::decode(&vcp) {
        Some(p) => {
            let json = vcp_json(&p, cid.as_deref());
            to_jstring(&mut env, &json.to_string())
        }
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_parseConnection<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    notification: JString<'local>,
    my_user_id: jlong,
    has_user_id: jboolean,
) -> jstring {
    let notification = jstr(&mut env, &notification);
    let value: Json = match serde_json::from_str(&notification) {
        Ok(v) => v,
        Err(e) => {
            throw(&mut env, e.to_string());
            return std::ptr::null_mut();
        }
    };
    let info = parse_connection(&value);
    let uid = if has_user_id == JNI_TRUE {
        Some(my_user_id)
    } else {
        None
    };
    to_jstring(&mut env, &connection_json(&info, uid).to_string())
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_parseTransmittedData<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    notification: JString<'local>,
) -> jstring {
    let notification = jstr(&mut env, &notification);
    let value: Json = match serde_json::from_str(&notification) {
        Ok(v) => v,
        Err(e) => {
            throw(&mut env, e.to_string());
            return std::ptr::null_mut();
        }
    };
    match parse_transmitted_data(&value) {
        Some(data) => to_jstring(&mut env, &transmitted_json(&data).to_string()),
        None => std::ptr::null_mut(),
    }
}

// ===================================================================
// Calls: ws2 signaling client
// ===================================================================

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_callConnect<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    url: JString<'local>,
    user_agent: JString<'local>,
    proxy: JString<'local>,
) -> jlong {
    let url = jstr(&mut env, &url);
    let ua = jstr_opt(&mut env, &user_agent);
    let proxy = match jstr_opt(&mut env, &proxy) {
        Some(p) => match ProxyConfig::parse(&p) {
            Ok(p) => Some(p),
            Err(e) => {
                throw(&mut env, e);
                return 0;
            }
        },
        None => None,
    };
    let rt = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => Arc::new(rt),
        Err(e) => {
            throw(&mut env, e.to_string());
            return 0;
        }
    };
    let result = rt.block_on(Ws2Signaling::connect_via(&url, ua.as_deref(), proxy.as_ref()));
    match result {
        Ok(sig) => {
            let inner = Arc::new(sig);
            let notif_rx = Mutex::new(inner.notifications());
            let call = Box::new(KCall {
                rt,
                inner,
                notif_rx,
            });
            Box::into_raw(call) as jlong
        }
        Err(e) => {
            throw(&mut env, e.to_string());
            0
        }
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_callAccept<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
) -> jstring {
    call_result(&mut env, handle, |c| c.rt.block_on(c.inner.accept_call()))
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_callHangup<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
    reason: JString<'local>,
) -> jstring {
    let reason = jstr(&mut env, &reason);
    call_result(&mut env, handle, |c| c.rt.block_on(c.inner.hangup(&reason)))
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_callTransmitSdp<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
    participant_id: jlong,
    sdp_type: JString<'local>,
    sdp: JString<'local>,
) -> jstring {
    let (t, d) = (jstr(&mut env, &sdp_type), jstr(&mut env, &sdp));
    call_result(&mut env, handle, |c| {
        c.rt.block_on(c.inner.transmit_sdp(participant_id, &t, &d))
    })
}

#[no_mangle]
#[allow(clippy::too_many_arguments, non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_callTransmitCandidate<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
    participant_id: jlong,
    candidate: JString<'local>,
    sdp_mid: JString<'local>,
    sdp_mline_index: jlong,
) -> jstring {
    let (cand, mid) = (jstr(&mut env, &candidate), jstr(&mut env, &sdp_mid));
    call_result(&mut env, handle, |c| {
        c.rt.block_on(
            c.inner
                .transmit_candidate(participant_id, &cand, &mid, sdp_mline_index),
        )
    })
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_callChangeMedia<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
    audio: jboolean,
    video: jboolean,
    screen: jboolean,
) -> jstring {
    call_result(&mut env, handle, |c| {
        c.rt.block_on(c.inner.change_media_settings(
            audio == JNI_TRUE,
            video == JNI_TRUE,
            screen == JNI_TRUE,
        ))
    })
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_callSendCommand<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
    command: JString<'local>,
    extra_json: JString<'local>,
) -> jstring {
    let extra = match jstr_opt(&mut env, &extra_json) {
        Some(s) => match serde_json::from_str::<Json>(&s) {
            Ok(v) => v,
            Err(e) => {
                throw(&mut env, e.to_string());
                return std::ptr::null_mut();
            }
        },
        None => Json::Object(Default::default()),
    };
    let command = jstr(&mut env, &command);
    call_result(&mut env, handle, |c| {
        c.rt.block_on(c.inner.send_command(&command, extra))
    })
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_callNextNotification<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
    timeout_ms: jlong,
) -> jstring {
    let c = match unsafe { call_ref(&mut env, handle) } {
        Some(c) => c,
        None => return std::ptr::null_mut(),
    };
    let mut rx = c.notif_rx.lock().unwrap();
    let value = c.rt.block_on(async {
        if timeout_ms < 0 {
            rx.recv().await.ok()
        } else {
            tokio::time::timeout(Duration::from_millis(timeout_ms as u64), rx.recv())
                .await
                .ok()
                .and_then(|r| r.ok())
        }
    });
    match value {
        Some(v) => to_jstring(&mut env, &v.to_string()),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_callIsConnected<'local>(
    mut env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
) -> jboolean {
    match unsafe { call_ref(&mut env, handle) } {
        Some(c) if c.inner.is_connected() => JNI_TRUE,
        _ => JNI_FALSE,
    }
}

#[no_mangle]
#[allow(non_snake_case)]
pub extern "system" fn Java_ru_kolibri_Native_callClose<'local>(
    _env: JNIEnv<'local>,
    _this: JObject<'local>,
    handle: jlong,
) {
    if handle != 0 {
        let call = unsafe { Box::from_raw(handle as *mut KCall) };
        call.inner.close();
    }
}

fn call_result(
    env: &mut JNIEnv,
    handle: jlong,
    f: impl FnOnce(&KCall) -> Result<Json, kolibri_net::calls::CallError>,
) -> jstring {
    let c = match unsafe { call_ref(env, handle) } {
        Some(c) => c,
        None => return std::ptr::null_mut(),
    };
    match f(c) {
        Ok(v) => to_jstring(env, &v.to_string()),
        Err(e) => {
            throw(env, e.to_string());
            std::ptr::null_mut()
        }
    }
}

// ===================================================================
// JSON shaping helpers (shared with the Swift/Go wrappers)
// ===================================================================

fn ice_json(urls: &[String], username: &Option<String>, credential: &Option<String>) -> Json {
    json!({ "urls": urls, "username": username, "credential": credential })
}

fn vcp_json(p: &ConversationParams, conversation_id: Option<&str>) -> Json {
    let ice: Vec<Json> = p
        .ice_servers()
        .iter()
        .map(|s| ice_json(&s.urls, &s.username, &s.credential))
        .collect();
    let mut obj = json!({
        "token": p.token,
        "ws_endpoint": p.ws_endpoint,
        "stun": p.stun,
        "turn": p.turn,
        "turn_user": p.turn_user,
        "turn_password": p.turn_password,
        "is_video": p.is_video,
        "expires_at": p.expires_at,
        "user_id": p.user_id(),
        "ice_servers": ice,
    });
    if let Some(cid) = conversation_id {
        obj["ws2_url"] = json!(p.ws2_url(cid, &Ws2ClientInfo::default()));
    }
    obj
}

fn connection_json(info: &ConnectionInfo, my_user_id: Option<i64>) -> Json {
    let ice: Vec<Json> = info
        .ice_servers
        .iter()
        .map(|s| ice_json(&s.urls, &s.username, &s.credential))
        .collect();
    let mut obj = json!({
        "topology": info.topology,
        "is_sfu": info.is_sfu(),
        "participants": info.participants,
        "ice_servers": ice,
    });
    if let Some(uid) = my_user_id {
        obj["peer"] = json!(info.peer_of(uid));
    }
    obj
}

fn transmitted_json(data: &TransmittedData) -> Json {
    match data {
        TransmittedData::Sdp { sdp_type, sdp } => {
            json!({ "kind": "sdp", "type": sdp_type, "sdp": sdp })
        }
        TransmittedData::Candidate {
            candidate,
            sdp_mid,
            sdp_mline_index,
        } => json!({
            "kind": "candidate",
            "candidate": candidate,
            "sdp_mid": sdp_mid,
            "sdp_mline_index": sdp_mline_index,
        }),
    }
}

fn payload_to_json(payload: &[u8]) -> String {
    if payload.is_empty() {
        return "null".to_string();
    }
    match rmpv::decode::read_value(&mut &payload[..]) {
        Ok(v) => value_to_json(&v).to_string(),
        Err(_) => "null".to_string(),
    }
}

fn payload_to_json_tagged(payload: &[u8]) -> String {
    if payload.is_empty() {
        return "null".to_string();
    }
    match rmpv::decode::read_value(&mut &payload[..]) {
        Ok(v) => kolibri_net::protocol::value_to_json_tagged(&v).to_string(),
        Err(_) => "null".to_string(),
    }
}

fn cmd_label(dir: Direction, cmd: u8) -> &'static str {
    match cmd {
        kolibri_net::cmd::OK => "ok",
        kolibri_net::cmd::NOT_FOUND => "not_found",
        kolibri_net::cmd::ERROR => "error",
        _ => match dir {
            Direction::Out => "request",
            Direction::In => "push",
        },
    }
}
