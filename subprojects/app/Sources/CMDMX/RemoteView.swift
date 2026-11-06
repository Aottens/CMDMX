import SwiftUI
import CoreImage.CIFilterBuiltins

// Remote screen (RMT-15): renders `engine.remote` and sends `remote.set_login` / `remote.off`.
// The engine validates and refuses with a message; the checks here only spare a round trip.
// The password is never filled from the engine (it never sends it).

struct RemoteView: View {
    @Environment(\.theme) private var t
    @EnvironmentObject private var engine: EngineClient
    @State private var username = ""
    @State private var password = ""
    @State private var port = ""
    @State private var problem: String?

    var body: some View {
        let remote = engine.remote
        HStack(alignment: .top, spacing: 8) {
            PanelWindow(title: "Remote", framed: true, accessory: { EmptyView() }) {
                VStack(alignment: .leading, spacing: 8) {
                    Text(status(remote)).font(Typo.legend(Typo.large, .semibold))
                        .foregroundStyle(remote.on ? t.green : t.fg2)
                    TextField("Username", text: $username).textFieldStyle(.roundedBorder)
                    SecureField("Password", text: $password).textFieldStyle(.roundedBorder)
                    TextField("Port", text: $port).textFieldStyle(.roundedBorder)
                    HStack(spacing: 8) {
                        Button("Turn on") { turnOn() }.buttonStyle(FlatButton())
                        Button("Turn off") { problem = nil; engine.remoteOff() }
                            .buttonStyle(FlatButton()).disabled(!remote.on)
                    }
                    if let problem {
                        Text(problem).font(Typo.ui(11)).foregroundStyle(t.amber)
                    } else if let message = engine.lastMessage {
                        Text(message).font(Typo.ui(11)).foregroundStyle(t.amber)
                    }
                    Text("Plain HTTP: the password crosses the network readable.")
                        .font(Typo.ui(11)).foregroundStyle(t.fg2)
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(8)
            }
            .frame(width: 320)
            if remote.on {
                PanelWindow(title: "Address", framed: true, accessory: { EmptyView() }) {
                    VStack(alignment: .leading, spacing: 8) {
                        if remote.addresses.isEmpty {
                            Text("No network address: is the Mac on the network?")
                                .font(Typo.ui(11)).foregroundStyle(t.amber)
                        } else {
                            ForEach(remote.addresses, id: \.self) { address in
                                Text(url(address, remote.port)).font(Typo.ui(13)).textSelection(.enabled)
                            }
                            if let first = remote.addresses.first, let image = Self.qr(url(first, remote.port)) {
                                // Black on white with a quiet zone, so phones read it on the dark theme.
                                Image(nsImage: image).interpolation(.none).resizable()
                                    .frame(width: 180, height: 180).padding(12).background(Color.white)
                            }
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(8)
                }
                .frame(width: 320)
            }
            Spacer(minLength: 0)
        }
        .padding(8)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(t.bg)
        .onAppear { prefill(remote) }
        .onChange(of: remote) { _, new in prefill(new) }
        // A login set elsewhere (another client, the engine restarted) shows as it is.
        .onChange(of: remote.port) { _, new in port = String(new) }
        .onChange(of: remote.username) { _, new in if let new { username = new } }
        .onChange(of: username) { _, _ in problem = nil; engine.clearMessage() }
        .onChange(of: password) { _, _ in problem = nil; engine.clearMessage() }
        .onChange(of: port) { _, _ in problem = nil; engine.clearMessage() }
    }

    private func status(_ remote: RemoteState) -> String {
        guard remote.on else { return "Off" }
        return "On · \(remote.clients) \(remote.clients == 1 ? "phone" : "phones") connected"
    }

    private func url(_ address: String, _ port: Int) -> String { "http://\(address):\(port)" }

    /// Fills only empty fields, so typing is never overwritten.
    private func prefill(_ remote: RemoteState) {
        if username.isEmpty, let name = remote.username { username = name }
        if port.isEmpty { port = String(remote.port) }
    }

    private func turnOn() {
        guard !username.isEmpty, !password.isEmpty else { problem = "Username and password are required."; return }
        guard let number = Int(port), (1024...65535).contains(number) else {
            problem = "Port must be a number from 1024 to 65535."; return
        }
        problem = nil
        engine.setRemoteLogin(username: username, password: password, port: number)
        password = ""
    }

    private static func qr(_ text: String) -> NSImage? {
        let filter = CIFilter.qrCodeGenerator()
        filter.message = Data(text.utf8)
        guard let output = filter.outputImage else { return nil }
        let scaled = output.transformed(by: CGAffineTransform(scaleX: 8, y: 8))   // crisp modules
        let rep = NSCIImageRep(ciImage: scaled)
        let image = NSImage(size: rep.size)
        image.addRepresentation(rep)
        return image
    }
}
