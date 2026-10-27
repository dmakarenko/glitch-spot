// ABOUTME: Bakes the redactions into the captured frame and draws the annotations on a copy, as the Desktop does.
#if os(iOS) && DEBUG
import UIKit

enum SnapshotRendererError: Error {
    case unreadableFrame
    case pngEncodingFailed
}

@MainActor
enum SnapshotRenderer {
    /// Returns the clean PNG (redactions baked in) and the annotated PNG (the clean image plus frames and notes).
    static func images(frame: CGImage, scale: CGFloat, marks: [Mark]) throws -> (clean: Data, annotated: Data) {
        let clean = try redacted(frame, scale: scale, marks: marks)
        let annotated = annotate(clean, scale: scale, annotations: marks.filter { $0.kind == .annotation })
        guard let cleanPNG = UIImage(cgImage: clean).pngData(), let annotatedPNG = annotated.pngData() else {
            throw SnapshotRendererError.pngEncodingFailed
        }
        return (cleanPNG, annotatedPNG)
    }

    /// The Desktop rule (`redact.rs`): blocks of 12 points, at least 2 px, anchored at the region origin.
    static func blockPixels(scale: CGFloat) -> Int {
        max(2, Int((12 * scale).rounded()))
    }

    /// The frame with each redaction mark pixelated.
    static func redacted(_ frame: CGImage, scale: CGFloat, marks: [Mark]) throws -> CGImage {
        let regions = marks.filter { $0.kind == .redaction }.map { $0.pixelRect(scale: scale) }
        return try pixelate(frame, regions: regions, scale: scale)
    }

    private static func pixelate(_ frame: CGImage, regions: [CGRect], scale: CGFloat) throws -> CGImage {
        let width = frame.width, height = frame.height
        guard let context = CGContext(
            data: nil, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4,
            space: CGColorSpace(name: CGColorSpace.sRGB)!,
            bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
        ), let data = context.data else { throw SnapshotRendererError.unreadableFrame }
        context.draw(frame, in: CGRect(x: 0, y: 0, width: width, height: height))

        let pixels = data.bindMemory(to: UInt8.self, capacity: width * height * 4)
        let block = blockPixels(scale: scale)
        for region in regions {
            let x0 = max(0, Int(region.minX)), y0 = max(0, Int(region.minY))
            let x1 = min(width, Int(region.maxX)), y1 = min(height, Int(region.maxY))
            guard x1 > x0, y1 > y0 else { continue }
            for by in stride(from: y0, to: y1, by: block) {
                for bx in stride(from: x0, to: x1, by: block) {
                    averageBlock(pixels, width: width, xs: bx..<min(bx + block, x1), ys: by..<min(by + block, y1))
                }
            }
        }
        guard let image = context.makeImage() else { throw SnapshotRendererError.unreadableFrame }
        return image
    }

    private static func averageBlock(_ pixels: UnsafeMutablePointer<UInt8>, width: Int, xs: Range<Int>, ys: Range<Int>) {
        var sum = [Int](repeating: 0, count: 4)
        for y in ys {
            for x in xs {
                let i = (y * width + x) * 4
                for c in 0..<4 { sum[c] += Int(pixels[i + c]) }
            }
        }
        let n = xs.count * ys.count
        let mean = sum.map { UInt8($0 / n) }
        for y in ys {
            for x in xs {
                let i = (y * width + x) * 4
                for c in 0..<4 { pixels[i + c] = mean[c] }
            }
        }
    }

    private static func annotate(_ clean: CGImage, scale: CGFloat, annotations: [Mark]) -> UIImage {
        let size = CGSize(width: clean.width, height: clean.height)
        let format = UIGraphicsImageRendererFormat()
        format.scale = 1
        format.opaque = true
        format.preferredRange = .standard
        return UIGraphicsImageRenderer(size: size, format: format).image { context in
            UIImage(cgImage: clean).draw(in: CGRect(origin: .zero, size: size))
            let rects = annotations.map { $0.rect.standardized.applying(CGAffineTransform(scaleX: scale, y: scale)) }
            for (index, rect) in rects.enumerated() {
                drawFrame(rect, label: String(format: "%02d", index + 1), scale: scale, in: context.cgContext)
            }
            for (mark, rect) in zip(annotations, rects) {
                let note = mark.note.trimmingCharacters(in: .whitespacesAndNewlines)
                if !note.isEmpty { drawCaption(note, below: rect, scale: scale, canvas: size) }
            }
        }
    }

    private static func drawFrame(_ rect: CGRect, label: String, scale: CGFloat, in cg: CGContext) {
        cg.setStrokeColor(Palette.accent.cgColor)
        cg.setLineWidth(2 * scale)
        cg.stroke(rect)

        let pill = CGRect(x: rect.minX - 13 * scale, y: rect.minY - 9 * scale, width: 26 * scale, height: 18 * scale)
        Palette.accent.setFill()
        UIBezierPath(roundedRect: pill, cornerRadius: 9 * scale).fill()
        let attributes: [NSAttributedString.Key: Any] = [
            .font: UIFont.boldSystemFont(ofSize: 11 * scale),
            .foregroundColor: UIColor.white,
        ]
        let text = label as NSString
        let textSize = text.size(withAttributes: attributes)
        text.draw(
            at: CGPoint(x: pill.midX - textSize.width / 2, y: pill.midY - textSize.height / 2),
            withAttributes: attributes
        )
    }

    private static func drawCaption(_ note: String, below rect: CGRect, scale: CGFloat, canvas: CGSize) {
        let offset = 6 * scale, padX = 8 * scale, padY = 6 * scale, radius = 6 * scale
        let border = max(1, (1 * scale).rounded())
        let maxWidth = min(240 * scale, canvas.width)
        let paragraph = NSMutableParagraphStyle()
        paragraph.lineHeightMultiple = 1.4
        let attributes: [NSAttributedString.Key: Any] = [
            .font: UIFont.systemFont(ofSize: 13 * scale),
            .foregroundColor: UIColor(red: 170 / 255, green: 170 / 255, blue: 170 / 255, alpha: 1),
            .paragraphStyle: paragraph,
        ]
        let text = note as NSString
        let textBounds = text.boundingRect(
            with: CGSize(width: max(1, maxWidth - 2 * padX), height: .greatestFiniteMagnitude),
            options: [.usesLineFragmentOrigin], attributes: attributes, context: nil
        )
        let boxW = min(maxWidth, textBounds.width.rounded(.up) + 2 * padX)
        let boxH = textBounds.height.rounded(.up) + 2 * padY

        // Below the rect; above when it overflows the bottom; always fully on the canvas.
        let belowY = rect.maxY + offset
        var boxY = belowY + boxH > canvas.height ? rect.minY - offset - boxH : belowY
        boxY = max(0, min(boxY, canvas.height - boxH))
        let boxX = max(0, min(rect.minX, canvas.width - boxW))
        let box = CGRect(x: boxX, y: boxY, width: boxW, height: boxH)

        Palette.accent.setFill()
        UIBezierPath(roundedRect: box, cornerRadius: radius).fill()
        UIColor(red: 22 / 255, green: 22 / 255, blue: 22 / 255, alpha: 1).setFill()
        UIBezierPath(roundedRect: box.insetBy(dx: border, dy: border), cornerRadius: max(0, radius - border)).fill()
        text.draw(
            with: CGRect(x: boxX + padX, y: boxY + padY, width: boxW - 2 * padX, height: boxH - 2 * padY),
            options: [.usesLineFragmentOrigin], attributes: attributes, context: nil
        )
    }
}
#endif
