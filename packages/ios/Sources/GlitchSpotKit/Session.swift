// ABOUTME: One started SDK: it observes system screenshots, shows the toast, and owns the overlay window.
#if os(iOS) && DEBUG
import GlitchSpotCore
import os
import UIKit

@MainActor
final class Session {
    private static let log = Logger(subsystem: "dev.glitchspot.kit", category: "session")

    var toastDuration: Duration = .seconds(4)
    private(set) var toast: ToastWindow?
    private(set) var overlay: OverlayWindow?
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
        closeOverlay()
    }

    func openOverlay() {
        hideToast()
        guard overlay == nil, let scene = Self.activeScene(),
              let host = scene.keyWindow ?? scene.windows.first(where: { !$0.isHidden && !($0 is ToastWindow) })
        else { return }
        // The frame is taken before the overlay window exists, so the overlay is never in it.
        let frame = WindowCapture.image(of: host)
        let model = OverlayModel(
            frame: frame,
            onSave: { [weak self] marks in await self?.deliver(frame: frame, marks: marks) },
            onCancel: { [weak self] in self?.closeOverlay() }
        )
        let window = OverlayWindow(windowScene: scene, model: model, host: host)
        overlay = window
        window.makeKeyAndVisible()
    }

    private func closeOverlay() {
        overlay?.close()
        overlay = nil
    }

    private func deliver(frame: UIImage, marks: [Mark]) async {
        let snapshot: Snapshot
        do {
            snapshot = try SnapshotBuilder.snapshot(frame: frame, marks: marks, metadata: metadata())
        } catch {
            Self.log.error("Glitch Spot could not build the snapshot: \(error, privacy: .public)")
            closeOverlay()
            return
        }
        closeOverlay()
        do {
            try await onSnapshot(snapshot)
        } catch {
            Self.log.error("Glitch Spot onSnapshot threw: \(error, privacy: .public)")
        }
    }

    private func showToast() {
        guard overlay == nil, let scene = Self.activeScene() else { return }
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
