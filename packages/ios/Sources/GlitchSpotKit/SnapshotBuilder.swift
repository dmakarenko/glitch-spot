// ABOUTME: Turns a captured frame and the user's marks into the GlitchSpotCore snapshot with app and device context.
#if os(iOS) && DEBUG
import GlitchSpotCore
import UIKit

@MainActor
enum SnapshotBuilder {
    static func snapshot(frame: UIImage, marks: [Mark], metadata: [String: String], now: Date = Date()) throws -> Snapshot {
        guard let cgFrame = frame.cgImage else { throw SnapshotRendererError.unreadableFrame }
        let scale = frame.scale
        let images = try SnapshotRenderer.images(frame: cgFrame, scale: scale, marks: marks)
        let annotations = marks.filter { $0.kind == .annotation }.enumerated().map { index, mark in
            let r = mark.rect.standardized
            return Annotation(
                id: mark.id.uuidString,
                shape: "rect",
                coords: Rect(x: r.minX * scale, y: r.minY * scale, w: r.width * scale, h: r.height * scale),
                note: mark.note.trimmingCharacters(in: .whitespacesAndNewlines),
                label: String(format: "%02d", index + 1)
            )
        }
        let redactions = marks.filter { $0.kind == .redaction }.map { mark in
            let r = mark.pixelRect(scale: scale)
            return Rect(x: r.minX, y: r.minY, w: r.width, h: r.height)
        }
        let info = Bundle.main.infoDictionary ?? [:]
        return Snapshot(
            id: UUID().uuidString,
            createdAt: ISO8601DateFormatter().string(from: now),
            mode: "frozen",
            annotations: annotations,
            redactions: redactions,
            context: Snapshot.Context(
                screen: .init(width: cgFrame.width, height: cgFrame.height, scaleFactor: Double(scale)),
                os: .init(platform: "iOS", version: UIDevice.current.systemVersion),
                app: .init(
                    name: info["CFBundleDisplayName"] as? String ?? info["CFBundleName"] as? String ?? "",
                    version: info["CFBundleShortVersionString"] as? String ?? "",
                    build: info["CFBundleVersion"] as? String ?? "",
                    metadata: metadata.isEmpty ? nil : metadata
                ),
                device: .init(model: deviceModel())
            ),
            images: .init(clean: images.clean, annotated: images.annotated)
        )
    }

    /// The hardware model id, for example `iPhone17,1`; the Simulator reports the model that it imitates.
    static func deviceModel() -> String {
        if let simulated = ProcessInfo.processInfo.environment["SIMULATOR_MODEL_IDENTIFIER"] { return simulated }
        var system = utsname()
        uname(&system)
        return withUnsafeBytes(of: &system.machine) { String(decoding: $0.prefix { $0 != 0 }, as: UTF8.self) }
    }
}
#endif
