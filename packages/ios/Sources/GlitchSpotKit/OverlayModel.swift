// ABOUTME: State of one open overlay: the captured frame, the user's marks, the tool, and the redacted preview.
#if os(iOS) && DEBUG
import Observation
import UIKit

@MainActor
@Observable
final class OverlayModel {
    enum Tool { case annotate, redact }

    let frame: UIImage
    var tool: Tool = .annotate
    var marks: [Mark] = [] {
        didSet { refreshPreview() }
    }
    /// The frame with the current redactions pixelated, as the snapshot will show them.
    private(set) var preview: UIImage
    private(set) var isSaving = false

    @ObservationIgnored private var previewRedactions: [CGRect] = []
    @ObservationIgnored private let onSave: @MainActor ([Mark]) async -> Void
    @ObservationIgnored private let onCancel: @MainActor () -> Void

    init(frame: UIImage, onSave: @escaping @MainActor ([Mark]) async -> Void, onCancel: @escaping @MainActor () -> Void) {
        self.frame = frame
        self.preview = frame
        self.onSave = onSave
        self.onCancel = onCancel
    }

    var scale: CGFloat { frame.scale }

    func save() async {
        guard !isSaving else { return }
        isSaving = true
        await onSave(marks)
        isSaving = false
    }

    func cancel() {
        onCancel()
    }

    func delete(_ id: Mark.ID) {
        marks.removeAll { $0.id == id }
    }

    private func refreshPreview() {
        let redactions = marks.filter { $0.kind == .redaction }.map(\.rect)
        guard redactions != previewRedactions, let cgFrame = frame.cgImage else { return }
        previewRedactions = redactions
        if let redacted = try? SnapshotRenderer.redacted(cgFrame, scale: scale, marks: marks) {
            preview = UIImage(cgImage: redacted, scale: scale, orientation: .up)
        }
    }
}
#endif
