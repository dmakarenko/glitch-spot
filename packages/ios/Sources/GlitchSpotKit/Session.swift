// ABOUTME: One started SDK: it observes system screenshots, shows the toast, and owns the overlay window.
#if os(iOS) && DEBUG
import UIKit

@MainActor
final class Session {
    var toastDuration: Duration = .seconds(4)
    private(set) var toast: ToastWindow?
    private let onSnapshot: GlitchSpot.SnapshotHandler
    private let metadata: GlitchSpot.MetadataProvider
    private var observer: NSObjectProtocol?
    private var hideToastTask: Task<Void, Never>?

    init(onSnapshot: @escaping GlitchSpot.SnapshotHandler, metadata: @escaping GlitchSpot.MetadataProvider) {
        self.onSnapshot = onSnapshot
        self.metadata = metadata
        observer = NotificationCenter.default.addObserver(
            forName: UIApplication.userDidTakeScreenshotNotification,
            object: nil,
            queue: .main
        ) { [weak self] _ in
            MainActor.assumeIsolated { self?.showToast() }
        }
    }

    func end() {
        if let observer { NotificationCenter.default.removeObserver(observer) }
        observer = nil
        hideToast()
    }

    func openOverlay() {}

    private func showToast() {
        guard let scene = Self.activeScene() else { return }
        let toast = toast ?? ToastWindow(windowScene: scene) { [weak self] in
            self?.hideToast()
            self?.openOverlay()
        }
        self.toast = toast
        toast.isHidden = false

        hideToastTask?.cancel()
        let duration = toastDuration
        hideToastTask = Task { [weak self] in
            try? await Task.sleep(for: duration)
            guard !Task.isCancelled else { return }
            self?.hideToast()
        }
    }

    private func hideToast() {
        hideToastTask?.cancel()
        hideToastTask = nil
        toast?.isHidden = true
        toast = nil
    }

    static func activeScene() -> UIWindowScene? {
        let scenes = UIApplication.shared.connectedScenes.compactMap { $0 as? UIWindowScene }
        return scenes.first { $0.activationState == .foregroundActive } ?? scenes.first
    }
}
#endif
