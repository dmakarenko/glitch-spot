// ABOUTME: Test helpers that read RGBA pixels from images and make windows on the test host's scene.
import UIKit
import XCTest

struct RGBA: Equatable {
    var r: UInt8, g: UInt8, b: UInt8, a: UInt8
}

/// The RGBA bytes of a CGImage, row by row, with no row padding.
struct Pixels {
    let width: Int
    let height: Int
    let bytes: [UInt8]

    init(_ image: CGImage) {
        width = image.width
        height = image.height
        var buffer = [UInt8](repeating: 0, count: width * height * 4)
        buffer.withUnsafeMutableBytes { raw in
            let context = CGContext(
                data: raw.baseAddress,
                width: image.width,
                height: image.height,
                bitsPerComponent: 8,
                bytesPerRow: image.width * 4,
                space: CGColorSpaceCreateDeviceRGB(),
                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
            )!
            context.draw(image, in: CGRect(x: 0, y: 0, width: image.width, height: image.height))
        }
        bytes = buffer
    }

    init(png: Data) throws {
        let image = try XCTUnwrap(UIImage(data: png)?.cgImage)
        self.init(image)
    }

    subscript(x: Int, y: Int) -> RGBA {
        let i = (y * width + x) * 4
        return RGBA(r: bytes[i], g: bytes[i + 1], b: bytes[i + 2], a: bytes[i + 3])
    }
}

@MainActor
func activeScene() throws -> UIWindowScene {
    try XCTUnwrap(
        UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }.first,
        "the test host has no window scene"
    )
}
