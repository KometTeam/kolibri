// A minimal handshake example, mirroring kolibri-go/example/handshake.
//
// Not built by the package. To run it, add an executable target to Package.swift
// (or paste the body into your own app) after building the native lib with
// ./build-rust.sh.
import Kolibri

@main
struct Handshake {
    static func main() async {
        var config = Config(host: "api.oneme.ru")
        config.onWire = { event in
            let arrow = event.direction == "out" ? "->" : "<-"
            let json = event.json.prefix(90)
            print("\(arrow) \(event.command) op=\(event.opcode) seq=\(event.seq) \(json)")
        }

        do {
            let session = try Session(config: config)

            let info = try await session.connect()
            print("connected: state=\(session.state), callsSeed=\(info["callsSeed"] ?? "nil")")

            print("switching interactive=false on the live socket")
            session.pingInteractive = false

            session.disconnect()
        } catch {
            print("error: \(error)")
        }
    }
}
