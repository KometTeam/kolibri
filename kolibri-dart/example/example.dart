import 'dart:io';

import 'package:kolibri/kolibri.dart';

/// Minimal end-to-end example: load the native library, open a session,
/// perform the handshake, then disconnect.
///
/// On Flutter you would omit `libraryPath` — the plugin bundles the native
/// library and `flutter_rust_bridge` finds it automatically. For `dart run`
/// point `KOLIBRI_LIB` at a library you built with
/// `cargo build --manifest-path rust/Cargo.toml`.
Future<void> main() async {
  final libPath = Platform.environment['KOLIBRI_LIB'] ??
      'rust/target/debug/libkolibri_dart.dylib';
  await initKolibri(libraryPath: libPath);

  final session = openSession(host: 'api.oneme.ru');
  print('state: ${session.state()}');

  final info = await session.connect(); // sessionInit handshake
  print('state       : ${session.state()}');
  print('calls_seed  : ${info.callsSeed}');
  print('device_name : ${info.deviceName}');
  print('payload     : ${info.payload.length} bytes of msgpack');

  // Server pushes arrive on a Stream; decode payloads to maps with pushesMap().
  final sub = session.pushesMap().listen((push) {
    final (opcode, payload) = push;
    print('push $opcode: $payload');
  });

  session.disconnect();
  await sub.cancel();
  print('state: ${session.state()}');
}
