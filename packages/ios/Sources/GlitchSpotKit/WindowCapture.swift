// ABOUTME: Renders a host window into an image in screen pixels, with no change to its view tree.
#if os(iOS) && DEBUG
import UIKit

@MainActor
enum WindowCapture {
    static func image(of window: UIWindow) -> UIImage {
        let format = UIGraphicsImageRendererFormat()
        format.scale = window.traitCollection.displayScale
        format.opaque = true
        return UIGraphicsImageRenderer(bounds: window.bounds, format: format).image { _ in
            // A layer render loses Metal and other GPU content; the render-server snapshot keeps it.
            _ = window.drawHierarchy(in: window.bounds, afterScreenUpdates: true)
        }
    }
}
#endif
