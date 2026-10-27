// ABOUTME: Tests that a system screenshot shows the toast in its own window, and that the toast hides and stops.
import UIKit
import XCTest
@testable import GlitchSpotKit

@MainActor
final class ToastTests: XCTestCase {
    override func tearDown() async throws {
        GlitchSpot.stop()
    }

    func testScreenshotNotificationShowsToastWindow() async throws {
        let scene = try activeScene()
        let hostWindows = Set(scene.windows.map(ObjectIdentifier.init))
        GlitchSpot.start(onSnapshot: { _ in })

        NotificationCenter.default.post(name: UIApplication.userDidTakeScreenshotNotification, object: nil)

        let toast = try await waitForToast(in: scene)
        XCTAssertFalse(hostWindows.contains(ObjectIdentifier(toast)), "the toast must not reuse a host window")
        XCTAssertFalse(toast.isHidden)
        XCTAssertGreaterThan(toast.windowLevel, .normal)
        XCTAssertFalse(toast.isKeyWindow, "the toast must not take the keyboard from the host")
    }

    func testToastHidesAfterItsDuration() async throws {
        let scene = try activeScene()
        GlitchSpot.start(onSnapshot: { _ in })
        GlitchSpot.session?.toastDuration = .milliseconds(100)

        NotificationCenter.default.post(name: UIApplication.userDidTakeScreenshotNotification, object: nil)
        let toast = try await waitForToast(in: scene)

        try await waitUntil { toast.isHidden }
    }

    func testStopRemovesToastAndIgnoresLaterScreenshots() async throws {
        let scene = try activeScene()
        GlitchSpot.start(onSnapshot: { _ in })
        NotificationCenter.default.post(name: UIApplication.userDidTakeScreenshotNotification, object: nil)
        let toast = try await waitForToast(in: scene)

        GlitchSpot.stop()
        XCTAssertTrue(toast.isHidden)
        NotificationCenter.default.post(name: UIApplication.userDidTakeScreenshotNotification, object: nil)
        try await Task.sleep(for: .milliseconds(200))

        XCTAssertNil(visibleToast(in: scene))
    }

    func testToastOnlyCatchesTouchesOnTheToast() async throws {
        let scene = try activeScene()
        GlitchSpot.start(onSnapshot: { _ in })
        NotificationCenter.default.post(name: UIApplication.userDidTakeScreenshotNotification, object: nil)
        let toast = try await waitForToast(in: scene)
        toast.layoutIfNeeded()

        let corner = CGPoint(x: toast.bounds.maxX - 2, y: toast.bounds.maxY - 2)
        XCTAssertNil(toast.hitTest(corner, with: nil), "a touch outside the toast must reach the host")
    }

    private func visibleToast(in scene: UIWindowScene) -> ToastWindow? {
        scene.windows.compactMap { $0 as? ToastWindow }.first { !$0.isHidden }
    }

    private func waitForToast(in scene: UIWindowScene) async throws -> ToastWindow {
        try await waitUntil { self.visibleToast(in: scene) != nil }
        return try XCTUnwrap(visibleToast(in: scene))
    }
}

@MainActor
func waitUntil(
    timeout: Duration = .seconds(3),
    file: StaticString = #filePath,
    line: UInt = #line,
    _ condition: () -> Bool
) async throws {
    let deadline = ContinuousClock.now + timeout
    while !condition() {
        guard ContinuousClock.now < deadline else {
            XCTFail("condition still false after \(timeout)", file: file, line: line)
            throw CancellationError()
        }
        try await Task.sleep(for: .milliseconds(20))
    }
}
