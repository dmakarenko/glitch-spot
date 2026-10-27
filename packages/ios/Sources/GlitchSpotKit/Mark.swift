// ABOUTME: One rectangle that the user draws on the overlay: an annotation with a note, or a redaction.
#if os(iOS) && DEBUG
import Foundation

struct Mark: Identifiable, Equatable, Sendable {
    enum Kind: Sendable { case annotation, redaction }

    let id: UUID
    var kind: Kind
    /// In points of the captured frame.
    var rect: CGRect
    var note: String

    init(id: UUID = UUID(), kind: Kind, rect: CGRect, note: String = "") {
        self.id = id
        self.kind = kind
        self.rect = rect
        self.note = note
    }

    /// The rectangle in image pixels, grown to whole pixels so that a redaction never leaves a partial edge.
    func pixelRect(scale: CGFloat) -> CGRect {
        let r = rect.standardized
        let x0 = (r.minX * scale).rounded(.down), y0 = (r.minY * scale).rounded(.down)
        let x1 = (r.maxX * scale).rounded(.up), y1 = (r.maxY * scale).rounded(.up)
        return CGRect(x: x0, y: y0, width: x1 - x0, height: y1 - y0)
    }
}
#endif
