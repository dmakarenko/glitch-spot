// ABOUTME: Tests that a redaction destroys the source pixels in the clean and the annotated PNG.
import UIKit
import XCTest
@testable import GlitchSpotKit

@MainActor
final class RedactionTests: XCTestCase {
    /// 120 × 120 px of 1 px black and white squares: every pixel differs from its neighbour.
    private let checkerboard = makeImage(width: 120, height: 120) { x, y in
        (x + y) % 2 == 0 ? RGBA(r: 0, g: 0, b: 0, a: 255) : RGBA(r: 255, g: 255, b: 255, a: 255)
    }

    func testRedactedRegionIsDestroyedInBothImages() throws {
        let redaction = Mark(kind: .redaction, rect: CGRect(x: 10, y: 10, width: 20, height: 20))
        let annotation = Mark(kind: .annotation, rect: CGRect(x: 40, y: 40, width: 15, height: 15), note: "Total")

        let images = try SnapshotRenderer.images(frame: checkerboard, scale: 2, marks: [redaction, annotation])

        let source = Pixels(checkerboard)
        for (name, png) in [("clean", images.clean), ("annotated", images.annotated)] {
            let output = try Pixels(png: png)
            XCTAssertEqual(output.width, 120, name)
            for y in 20..<60 {
                for x in 20..<60 {
                    let pixel = output[x, y]
                    XCTAssertNotEqual(pixel, source[x, y], "\(name) keeps the source pixel at \(x),\(y)")
                    let black = RGBA(r: 0, g: 0, b: 0, a: 255)
                    let white = RGBA(r: 255, g: 255, b: 255, a: 255)
                    XCTAssertFalse(pixel == black || pixel == white, "\(name) keeps the black-white pattern at \(x),\(y)")
                }
            }
        }
        XCTAssertEqual(try Pixels(png: images.clean)[0, 0], source[0, 0], "pixels outside a redaction stay")
    }

    func testPixelationUsesDesktopBlockSize() throws {
        let halves = makeImage(width: 48, height: 48) { x, _ in
            x < 12 ? RGBA(r: 255, g: 0, b: 0, a: 255) : RGBA(r: 0, g: 0, b: 255, a: 255)
        }
        let redaction = Mark(kind: .redaction, rect: CGRect(x: 0, y: 0, width: 24, height: 24))

        let clean = try Pixels(png: SnapshotRenderer.images(frame: halves, scale: 1, marks: [redaction]).clean)

        // At scale 1 the block is 12 px, so the red and the blue halves stay separate blocks.
        XCTAssertEqual(clean[0, 0], RGBA(r: 255, g: 0, b: 0, a: 255))
        XCTAssertEqual(clean[23, 23], RGBA(r: 0, g: 0, b: 255, a: 255))

        let wide = try Pixels(png: SnapshotRenderer.images(frame: halves, scale: 2, marks: [redaction]).clean)
        // At scale 2 the block is 24 px: one block with the truncated mean of 12 red and 12 blue columns.
        XCTAssertEqual(wide[0, 0], RGBA(r: 127, g: 0, b: 127, a: 255))
        XCTAssertEqual(wide[23, 23], wide[0, 0])
        XCTAssertEqual(wide[47, 47], RGBA(r: 0, g: 0, b: 255, a: 255))
    }

    func testAnnotationFrameIsDrawnOnlyInTheAnnotatedImage() throws {
        let gray = makeImage(width: 100, height: 100) { _, _ in RGBA(r: 50, g: 50, b: 50, a: 255) }
        let annotation = Mark(kind: .annotation, rect: CGRect(x: 20, y: 20, width: 30, height: 30))

        let images = try SnapshotRenderer.images(frame: gray, scale: 1, marks: [annotation])

        XCTAssertEqual(try Pixels(png: images.clean)[35, 20], RGBA(r: 50, g: 50, b: 50, a: 255))
        XCTAssertEqual(try Pixels(png: images.annotated)[35, 20], RGBA(r: 238, g: 102, b: 51, a: 255))
        XCTAssertEqual(try Pixels(png: images.annotated)[35, 35], RGBA(r: 50, g: 50, b: 50, a: 255))
    }
}

func makeImage(width: Int, height: Int, _ color: (Int, Int) -> RGBA) -> CGImage {
    var bytes = [UInt8](repeating: 0, count: width * height * 4)
    for y in 0..<height {
        for x in 0..<width {
            let c = color(x, y)
            let i = (y * width + x) * 4
            bytes[i] = c.r; bytes[i + 1] = c.g; bytes[i + 2] = c.b; bytes[i + 3] = c.a
        }
    }
    let provider = CGDataProvider(data: Data(bytes) as CFData)!
    return CGImage(
        width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32, bytesPerRow: width * 4,
        space: CGColorSpaceCreateDeviceRGB(),
        bitmapInfo: CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue),
        provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent
    )!
}
