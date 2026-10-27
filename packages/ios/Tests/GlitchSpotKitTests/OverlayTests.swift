// ABOUTME: Tests the overlay path from open to onSnapshot: marks, context, a throwing callback, and window focus.
import UIKit
import XCTest
@testable import GlitchSpotKit

@MainActor
final class OverlayTests: XCTestCase {
    override func tearDown() async throws {
        GlitchSpot.stop()
    }

    func testOnSnapshotGetsAnnotationsAndContext() async throws {
        var received: [Snapshot] = []
        GlitchSpot.start(onSnapshot: { received.append($0) }, metadata: { ["screen": "Checkout"] })
        let model = try await openOverlay()
        let scale = model.scale

        model.marks.append(Mark(kind: .annotation, rect: CGRect(x: 10, y: 20, width: 100, height: 50), note: "Total is cut off"))
        model.marks.append(Mark(kind: .redaction, rect: CGRect(x: 10, y: 100, width: 80, height: 30)))
        model.marks.append(Mark(kind: .annotation, rect: CGRect(x: 40, y: 200, width: 60, height: 60), note: "Wrong icon"))
        await model.save()

        let snapshot = try XCTUnwrap(received.first)
        XCTAssertEqual(received.count, 1)
        XCTAssertEqual(snapshot.mode, "frozen")
        XCTAssertEqual(snapshot.annotations.map(\.note), ["Total is cut off", "Wrong icon"])
        XCTAssertEqual(snapshot.annotations.map(\.label), ["01", "02"])
        XCTAssertEqual(snapshot.annotations.map(\.shape), ["rect", "rect"])
        XCTAssertEqual(snapshot.annotations[0].coords.x, 10 * scale)
        XCTAssertEqual(snapshot.annotations[0].coords.w, 100 * scale)
        XCTAssertEqual(snapshot.redactions.count, 1)
        XCTAssertEqual(snapshot.redactions[0].y, 100 * scale)

        XCTAssertEqual(snapshot.context.os.platform, "iOS")
        XCTAssertEqual(snapshot.context.os.version, UIDevice.current.systemVersion)
        XCTAssertEqual(snapshot.context.app.name, "Glitch Spot Example")
        XCTAssertEqual(snapshot.context.app.version, "1.0")
        XCTAssertEqual(snapshot.context.app.build, "1")
        XCTAssertEqual(snapshot.context.app.metadata, ["screen": "Checkout"])
        XCTAssertTrue(snapshot.context.device.model.hasPrefix("iPhone"), snapshot.context.device.model)
        XCTAssertEqual(snapshot.context.screen.scaleFactor, Double(scale))

        let clean = try Pixels(png: snapshot.images.clean)
        XCTAssertEqual(clean.width, snapshot.context.screen.width)
        XCTAssertEqual(clean.height, snapshot.context.screen.height)
        XCTAssertNoThrow(try Pixels(png: snapshot.images.annotated))
        XCTAssertNotNil(ISO8601DateFormatter().date(from: snapshot.createdAt), snapshot.createdAt)
        XCTAssertNil(GlitchSpot.session?.overlay, "the overlay closes after a save")
    }

    func testThrowingCallbackDoesNotCrash() async throws {
        struct HostFailure: Error {}
        var calls = 0
        GlitchSpot.start(onSnapshot: { _ in
            calls += 1
            throw HostFailure()
        })

        let first = try await openOverlay()
        first.marks.append(Mark(kind: .annotation, rect: CGRect(x: 10, y: 10, width: 40, height: 40), note: "a"))
        await first.save()
        let second = try await openOverlay()
        second.marks.append(Mark(kind: .annotation, rect: CGRect(x: 10, y: 10, width: 40, height: 40), note: "b"))
        await second.save()

        XCTAssertEqual(calls, 2, "the SDK keeps working after the host callback throws")
    }

    func testCancelClosesWithoutCallbackAndGivesFocusBack() async throws {
        let host = try XCTUnwrap(try activeScene().keyWindow)
        var calls = 0
        GlitchSpot.start(onSnapshot: { _ in calls += 1 })

        let model = try await openOverlay()
        XCTAssertFalse(host.isKeyWindow, "the overlay takes the keyboard for the notes")
        model.cancel()

        XCTAssertNil(GlitchSpot.session?.overlay)
        XCTAssertTrue(host.isKeyWindow)
        XCTAssertEqual(calls, 0)
    }

    func testCaptureHappensBeforeTheOverlayShows() async throws {
        let host = UIWindow(windowScene: try activeScene())
        host.rootViewController = UIViewController()
        host.rootViewController?.view.backgroundColor = UIColor(red: 0, green: 1, blue: 0, alpha: 1)
        host.makeKeyAndVisible()
        defer { host.isHidden = true }
        GlitchSpot.start(onSnapshot: { _ in })

        let model = try await openOverlay()

        // A frame taken after the overlay shows would hold the dark overlay, not the green host.
        let pixels = Pixels(try XCTUnwrap(model.frame.cgImage))
        XCTAssertEqual(pixels[pixels.width / 2, pixels.height / 2], RGBA(r: 0, g: 255, b: 0, a: 255))
    }

    func testToastTapOpensOverlay() async throws {
        GlitchSpot.start(onSnapshot: { _ in })
        NotificationCenter.default.post(name: UIApplication.userDidTakeScreenshotNotification, object: nil)
        try await waitUntil { GlitchSpot.session?.toast != nil }
        let button = try XCTUnwrap(
            GlitchSpot.session?.toast?.rootViewController?.view.firstSubview(withIdentifier: "glitchspot.toast") as? UIButton
        )

        button.sendActions(for: .primaryActionTriggered)

        try await waitUntil { GlitchSpot.session?.overlay != nil }
        XCTAssertNil(GlitchSpot.session?.toast)
    }

    func testOpenBeforeStartDoesNothing() throws {
        // The Example host starts the SDK at launch.
        GlitchSpot.stop()
        GlitchSpot.open()
        XCTAssertNil(GlitchSpot.session)
    }

    private func openOverlay() async throws -> OverlayModel {
        GlitchSpot.open()
        try await waitUntil { GlitchSpot.session?.overlay?.isKeyWindow == true }
        return try XCTUnwrap(GlitchSpot.session?.overlay?.model)
    }
}

extension UIView {
    func firstSubview(withIdentifier identifier: String) -> UIView? {
        if accessibilityIdentifier == identifier { return self }
        return subviews.lazy.compactMap { $0.firstSubview(withIdentifier: identifier) }.first
    }
}
