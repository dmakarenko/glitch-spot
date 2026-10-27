// ABOUTME: A Metal view whose clear colour cycles, so a capture shows whether GPU content survives.
import MetalKit
import SwiftUI

struct SpinningMetalView: UIViewRepresentable {
    func makeCoordinator() -> HueCycler { HueCycler() }

    func makeUIView(context: Context) -> MTKView {
        let view = MTKView(frame: .zero, device: MTLCreateSystemDefaultDevice())
        view.preferredFramesPerSecond = 30
        view.delegate = context.coordinator
        return view
    }

    func updateUIView(_ view: MTKView, context: Context) {}
}

@MainActor
final class HueCycler: NSObject, MTKViewDelegate {
    private var queue: MTLCommandQueue?
    private var hue = 0.0

    func mtkView(_ view: MTKView, drawableSizeWillChange size: CGSize) {}

    func draw(in view: MTKView) {
        guard let device = view.device else { return }
        if queue == nil { queue = device.makeCommandQueue() }
        hue = (hue + 0.005).truncatingRemainder(dividingBy: 1)
        var r: CGFloat = 0, g: CGFloat = 0, b: CGFloat = 0
        UIColor(hue: hue, saturation: 0.8, brightness: 0.9, alpha: 1).getRed(&r, green: &g, blue: &b, alpha: nil)
        view.clearColor = MTLClearColor(red: r, green: g, blue: b, alpha: 1)
        guard let pass = view.currentRenderPassDescriptor,
              let drawable = view.currentDrawable,
              let buffer = queue?.makeCommandBuffer(),
              let encoder = buffer.makeRenderCommandEncoder(descriptor: pass)
        else { return }
        encoder.endEncoding()
        buffer.present(drawable)
        buffer.commit()
    }
}
