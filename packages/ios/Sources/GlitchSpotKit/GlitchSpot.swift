// ABOUTME: Public entry point of the SDK: start, open, and stop the screenshot toast and the overlay.
#if os(iOS) && DEBUG
@_exported import GlitchSpotCore
import UIKit

@MainActor
public enum GlitchSpot {
    public typealias SnapshotHandler = @MainActor (Snapshot) async throws -> Void
    public typealias MetadataProvider = @MainActor () -> [String: String]

    private(set) static var session: Session?

    /// Shows the toast after each system screenshot. A second call replaces the first.
    public static func start(onSnapshot: @escaping SnapshotHandler, metadata: @escaping MetadataProvider = { [:] }) {
        stop()
        session = Session(onSnapshot: onSnapshot, metadata: metadata)
    }

    /// Captures the host key window now and opens the overlay on it.
    public static func open() {
        session?.openOverlay()
    }

    public static func stop() {
        session?.end()
        session = nil
    }
}
#endif
