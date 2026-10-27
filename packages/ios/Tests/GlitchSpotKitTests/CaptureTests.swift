// ABOUTME: Tests that the window capture keeps GPU-drawn content, which a layer-only render loses.
import MetalKit
import UIKit
import XCTest
@testable import GlitchSpotKit

@MainActor
final class CaptureTests: XCTestCase {
    func testMetalLayerRendersInCapture() async throws {
        let window = UIWindow(windowScene: try activeScene())
        let device = try XCTUnwrap(MTLCreateSystemDefaultDevice())
        let view = MTKView(frame: window.bounds, device: device)
        let renderer = try ClearToRed(device: device)
        view.clearColor = MTLClearColor(red: 1, green: 0, blue: 0, alpha: 1)
        view.delegate = renderer
        let controller = UIViewController()
        controller.view = view
        window.rootViewController = controller
        window.makeKeyAndVisible()
        defer { window.isHidden = true }

        await fulfillment(of: [renderer.presented], timeout: 5)
        let image = try XCTUnwrap(WindowCapture.image(of: window).cgImage)

        let pixels = Pixels(image)
        XCTAssertEqual(CGFloat(pixels.width), window.bounds.width * window.traitCollection.displayScale)
        XCTAssertEqual(pixels[pixels.width / 2, pixels.height / 2], RGBA(r: 255, g: 0, b: 0, a: 255))
    }
}

/// Clears each frame to the view's clear color and reports when the GPU finishes the fifth frame.
@MainActor
private final class ClearToRed: NSObject, MTKViewDelegate {
    let presented = XCTestExpectation(description: "red frames are on screen")
    private let queue: MTLCommandQueue
    private var frames = 0

    init(device: MTLDevice) throws {
        queue = try XCTUnwrap(device.makeCommandQueue())
    }

    func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {}

    func draw(in view: MTKView) {
        guard let pass = view.currentRenderPassDescriptor,
              let drawable = view.currentDrawable,
              let buffer = queue.makeCommandBuffer(),
              let encoder = buffer.makeRenderCommandEncoder(descriptor: pass)
        else { return }
        encoder.endEncoding()
        frames += 1
        // The iOS 27 SDK has no presented handler, and earlier frames are on screen when frame 5 completes.
        if frames == 5 {
            let presented = presented
            buffer.addCompletedHandler { _ in presented.fulfill() }
        }
        buffer.present(drawable)
        buffer.commit()
    }
}
