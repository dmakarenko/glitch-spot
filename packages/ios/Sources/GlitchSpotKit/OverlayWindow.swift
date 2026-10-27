// ABOUTME: The full-screen window that holds the overlay above the host, and gives the key status back on close.
#if os(iOS) && DEBUG
import SwiftUI
import UIKit

@MainActor
final class OverlayWindow: UIWindow {
    let model: OverlayModel
    private weak var host: UIWindow?

    init(windowScene: UIWindowScene, model: OverlayModel, host: UIWindow) {
        self.model = model
        self.host = host
        super.init(windowScene: windowScene)
        windowLevel = .alert
        let controller = UIHostingController(rootView: OverlayView(model: model))
        controller.view.backgroundColor = Palette.background
        rootViewController = controller
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    func close() {
        isHidden = true
        host?.makeKey()
    }
}
#endif
