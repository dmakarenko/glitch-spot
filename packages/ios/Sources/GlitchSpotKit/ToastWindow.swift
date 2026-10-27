// ABOUTME: The "Annotate in Glitch Spot" toast in its own window; touches outside the pill reach the host.
#if os(iOS) && DEBUG
import UIKit

@MainActor
final class ToastWindow: UIWindow {
    init(windowScene: UIWindowScene, onTap: @escaping @MainActor () -> Void) {
        super.init(windowScene: windowScene)
        windowLevel = .alert + 1
        backgroundColor = .clear
        let controller = ToastController(onTap: onTap)
        rootViewController = controller
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    override var canBecomeKey: Bool { false }

    override func hitTest(_ point: CGPoint, with event: UIEvent?) -> UIView? {
        let hit = super.hitTest(point, with: event)
        return hit === rootViewController?.view || hit === self ? nil : hit
    }
}

@MainActor
private final class ToastController: UIViewController {
    private let onTap: @MainActor () -> Void

    init(onTap: @escaping @MainActor () -> Void) {
        self.onTap = onTap
        super.init(nibName: nil, bundle: nil)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }

    override func viewDidLoad() {
        super.viewDidLoad()
        view.backgroundColor = .clear

        var config = UIButton.Configuration.filled()
        config.title = "Annotate in Glitch Spot"
        config.baseForegroundColor = Palette.textPrimary
        config.baseBackgroundColor = Palette.surfaceToolbar
        config.cornerStyle = .capsule
        config.contentInsets = NSDirectionalEdgeInsets(top: 12, leading: 20, bottom: 12, trailing: 20)
        let button = UIButton(configuration: config, primaryAction: UIAction { [onTap] _ in onTap() })
        button.accessibilityIdentifier = "glitchspot.toast"
        button.layer.borderColor = Palette.borderSubtle.cgColor
        button.layer.borderWidth = 1

        let pill = GlitchFrameView(content: button, cornerRadius: 22)
        pill.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(pill)
        NSLayoutConstraint.activate([
            pill.centerXAnchor.constraint(equalTo: view.centerXAnchor),
            pill.topAnchor.constraint(equalTo: view.safeAreaLayoutGuide.topAnchor, constant: 12),
        ])
    }
}
#endif
