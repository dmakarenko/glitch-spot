// ABOUTME: Example host app that embeds GlitchSpotKit and shows the last snapshot the SDK delivered.
import SwiftUI
#if DEBUG
import GlitchSpotKit
#endif

@main
struct ExampleApp: App {
    @State private var received = ReceivedSnapshot()

    var body: some Scene {
        WindowGroup {
            ContentView(received: received)
                .onAppear(perform: startGlitchSpot)
        }
    }

    private func startGlitchSpot() {
        #if DEBUG
        GlitchSpot.start(
            onSnapshot: { [received] snapshot in received.show(snapshot) },
            metadata: { ["screen": "Example"] }
        )
        #endif
    }
}

struct ContentView: View {
    let received: ReceivedSnapshot

    var body: some View {
        VStack(spacing: 16) {
            SpinningMetalView()
                .frame(height: 180)
                .clipShape(RoundedRectangle(cornerRadius: 12))
            #if DEBUG
            Button("Open Glitch Spot") { GlitchSpot.open() }
                .buttonStyle(.borderedProminent)
            Text("Take a screenshot, then tap the toast.")
                .font(.footnote)
                .foregroundStyle(.secondary)
            #else
            Text("Glitch Spot is not in this build.")
            #endif
            if let clean = received.clean, let annotated = received.annotated {
                HStack {
                    Image(uiImage: clean).resizable().scaledToFit()
                    Image(uiImage: annotated).resizable().scaledToFit()
                }
                .frame(height: 200)
            }
            ScrollView {
                Text(received.json)
                    .font(.system(size: 11, design: .monospaced))
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .textSelection(.enabled)
            }
        }
        .padding()
    }
}

@MainActor
@Observable
final class ReceivedSnapshot {
    var json = "No snapshot yet."
    var clean: UIImage?
    var annotated: UIImage?

    #if DEBUG
    /// Shows the callback JSON with each image replaced by its size, so the text stays short.
    func show(_ snapshot: Snapshot) {
        clean = UIImage(data: snapshot.images.clean)
        annotated = UIImage(data: snapshot.images.annotated)
        do {
            let data = try Delivery(snapshot: snapshot).encoded()
            guard var root = try JSONSerialization.jsonObject(with: data) as? [String: Any],
                  var body = root["snapshot"] as? [String: Any]
            else { return }
            body["images"] = [
                "clean_base64": "<\(snapshot.images.clean.count) bytes>",
                "annotated_base64": "<\(snapshot.images.annotated.count) bytes>",
            ]
            root["snapshot"] = body
            let pretty = try JSONSerialization.data(withJSONObject: root, options: [.prettyPrinted, .sortedKeys])
            json = String(decoding: pretty, as: UTF8.self)
        } catch {
            json = "Could not encode the snapshot: \(error)"
        }
    }
    #endif
}
