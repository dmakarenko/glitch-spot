// ABOUTME: DESIGN.md colour tokens and the chromatic-split frame that surfaced containers get.
#if os(iOS) && DEBUG
import UIKit

enum Palette {
    static let background = UIColor(red: 12 / 255, green: 12 / 255, blue: 12 / 255, alpha: 1)
    static let surfaceToolbar = UIColor(red: 18 / 255, green: 18 / 255, blue: 18 / 255, alpha: 0.94)
    static let accent = UIColor(red: 238 / 255, green: 102 / 255, blue: 51 / 255, alpha: 1)
    static let delete = UIColor(red: 1, green: 59 / 255, blue: 48 / 255, alpha: 1)
    static let textPrimary = UIColor(red: 224 / 255, green: 224 / 255, blue: 224 / 255, alpha: 1)
    static let textSecondary = UIColor(red: 136 / 255, green: 136 / 255, blue: 136 / 255, alpha: 1)
    static let borderSubtle = UIColor(white: 1, alpha: 0.07)
    static let glitchRed = UIColor(red: 1, green: 0, blue: 60 / 255, alpha: 0.4)
    static let glitchCyan = UIColor(red: 0, green: 245 / 255, blue: 1, alpha: 0.4)
}

/// Wraps a container with a red edge 2 pt left and a cyan edge 2 pt right, outside its layout.
@MainActor
final class GlitchFrameView: UIView {
    init(content: UIView, cornerRadius: CGFloat) {
        super.init(frame: .zero)
        for (color, dx) in [(Palette.glitchRed, -2.0), (Palette.glitchCyan, 2.0)] {
            let edge = UIView()
            edge.isUserInteractionEnabled = false
            edge.backgroundColor = color
            edge.layer.cornerRadius = cornerRadius
            edge.translatesAutoresizingMaskIntoConstraints = false
            addSubview(edge)
            NSLayoutConstraint.activate([
                edge.leadingAnchor.constraint(equalTo: leadingAnchor, constant: dx),
                edge.trailingAnchor.constraint(equalTo: trailingAnchor, constant: dx),
                edge.topAnchor.constraint(equalTo: topAnchor),
                edge.bottomAnchor.constraint(equalTo: bottomAnchor),
            ])
        }
        content.translatesAutoresizingMaskIntoConstraints = false
        addSubview(content)
        NSLayoutConstraint.activate([
            content.leadingAnchor.constraint(equalTo: leadingAnchor),
            content.trailingAnchor.constraint(equalTo: trailingAnchor),
            content.topAnchor.constraint(equalTo: topAnchor),
            content.bottomAnchor.constraint(equalTo: bottomAnchor),
        ])
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) { fatalError("init(coder:) is not supported") }
}
#endif
