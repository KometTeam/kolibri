import 'dart:typed_data';

import 'package:flutter_rust_bridge/flutter_rust_bridge_for_generated_io.dart'
    show ExternalLibrary;

import 'src/rust/api/session.dart' hide authMode;
import 'src/rust/api/session.dart' as _rust;
import 'src/rust/frb_generated.dart';

export 'src/rust/api/session.dart'
    show
        KolibriSession,
        SessionOptions,
        HandshakeInfo,
        PushEvent,
        UploadEvent,
        UploadEvent_Progress,
        UploadEvent_Done,
        UploadEvent_Error;

bool _initialized = false;

/// Load the native library and init the bindings; call once. On Flutter omit
/// [libraryPath] (bundled library used); for `dart run` pass the built dylib path.
Future<void> initKolibri({String? libraryPath}) async {
  if (_initialized) return;
  await RustLib.init(
    externalLibrary:
        libraryPath == null ? null : ExternalLibrary.open(libraryPath),
  );
  _initialized = true;
}

/// Create a session with sensible defaults; override any device field to spoof.
KolibriSession openSession({
  required String host,
  int port = 443,
  String deviceId = 'kolibri-dart',
  String instanceId = 'kolibri-dart',
  String appVersion = '26.20.2',
  int buildNumber = 6758,
  String deviceType = 'ANDROID',
  String osVersion = 'Android 14',
  String timezone = 'Europe/Moscow',
  String screen = '420dpi 420dpi 1080x2340',
  String pushDeviceType = 'GCM',
  String arch = 'arm64-v8a',
  String locale = 'ru',
  String deviceName = 'Dart',
  String deviceLocale = 'ru',
  int clientSessionId = 1700000000,
  int pingIntervalSecs = 10,
  bool autoReconnect = true,
  bool insecureTls = false,
}) {
  return KolibriSession(
    options: SessionOptions(
      host: host,
      port: port,
      deviceId: deviceId,
      instanceId: instanceId,
      appVersion: appVersion,
      buildNumber: buildNumber,
      deviceType: deviceType,
      osVersion: osVersion,
      timezone: timezone,
      screen: screen,
      pushDeviceType: pushDeviceType,
      arch: arch,
      locale: locale,
      deviceName: deviceName,
      deviceLocale: deviceLocale,
      clientSessionId: clientSessionId,
      pingIntervalSecs: BigInt.from(pingIntervalSecs),
      autoReconnect: autoReconnect,
      insecureTls: insecureTls,
    ),
  );
}

/// 96-byte anti-spoof fingerprint (authRequest `mode` / login `chatCacheFingerprint`).
/// signature/dex/so default to the known app values; override (raw bytes) per app build.
Uint8List authMode(
  int callsSeed,
  String deviceId, {
  List<int>? signature,
  List<int>? dex,
  List<int>? so,
}) {
  return _rust.authMode(
    signature: signature ?? _defaultSignatureDigest,
    dex: dex ?? _defaultDexDigest,
    so: so ?? _defaultSoDigest,
    callsSeed: callsSeed,
    deviceId: deviceId,
  );
}

final Uint8List _defaultSignatureDigest = _hex(
    '1684414033eb263e2c615f8b7df5ed8793850a07656304997fbf07e9e21e1e93');
final Uint8List _defaultSoDigest = _hex(
    '90e2fb8745b17b42a10182f8d8ac590e3fca5b311e2ce2d5144fa2c18cb3090d');
final Uint8List _defaultDexDigest = _hex(
    '0a6265f6e5d8231b9cba641f8c40475e6f3baeb06ed41b804b9bf7307aa4214e');

Uint8List _hex(String s) {
  final out = Uint8List(s.length ~/ 2);
  for (var i = 0; i < out.length; i++) {
    out[i] = int.parse(s.substring(i * 2, i * 2 + 2), radix: 16);
  }
  return out;
}
