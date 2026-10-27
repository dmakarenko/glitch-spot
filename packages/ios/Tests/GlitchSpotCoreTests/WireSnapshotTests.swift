// ABOUTME: Tests the JSON that the SDK encodes for a delivery: keys, version, and image encoding.
import Foundation
import XCTest
@testable import GlitchSpotCore

final class WireSnapshotTests: XCTestCase {
    func testCommittedFixtureIsTheProductionEncoding() throws {
        XCTAssertEqual(try Fixtures.sampleDelivery().encoded(), try Fixtures.data("wire-snapshot.json"))
    }

    func testEncodesSnakeCaseKeysAndVersion() throws {
        let json = try XCTUnwrap(
            JSONSerialization.jsonObject(with: Fixtures.sampleDelivery().encoded()) as? [String: Any]
        )
        XCTAssertEqual(json["v"] as? Int, 1)
        XCTAssertEqual(json["delivery_id"] as? String, "6F1C2B8E-4D3A-4E5F-9A0B-1C2D3E4F5A6B")

        let snapshot = try XCTUnwrap(json["snapshot"] as? [String: Any])
        XCTAssertEqual(snapshot["created_at"] as? String, "2026-10-27T16:30:00Z")
        let context = try XCTUnwrap(snapshot["context"] as? [String: Any])
        let screen = try XCTUnwrap(context["screen"] as? [String: Any])
        XCTAssertEqual(screen["scale_factor"] as? Double, 3)
        XCTAssertNil(screen["display_index"])
        let app = try XCTUnwrap(context["app"] as? [String: Any])
        XCTAssertEqual(app["build"] as? String, "512")
        XCTAssertEqual(app["metadata"] as? [String: String], ["screen": "Checkout"])
        XCTAssertEqual((context["device"] as? [String: Any])?["model"] as? String, "iPhone17,1")
        let images = try XCTUnwrap(snapshot["images"] as? [String: Any])
        XCTAssertNotNil(images["clean_base64"] as? String)
        XCTAssertNotNil(images["annotated_base64"] as? String)
    }

    func testEncodesImagesAsPaddedStandardBase64WithoutLineBreaks() throws {
        var delivery = Fixtures.sampleDelivery()
        delivery.snapshot.images = .init(clean: Data([0xFB, 0xFF]), annotated: Data(repeating: 0xAB, count: 200))
        let text = String(decoding: try delivery.encoded(), as: UTF8.self)

        XCTAssertTrue(text.contains(#""clean_base64":"+/8=""#), text)
        XCTAssertFalse(text.contains("\n"))
        XCTAssertFalse(text.contains(#"\/"#))
    }
}
