// ABOUTME: Writes the Swift-made ios-contract fixtures when GLITCH_SPOT_WRITE_FIXTURES=1, and skips otherwise.
import Foundation
import XCTest
@testable import GlitchSpotCore

final class FixtureWriterTests: XCTestCase {
    func testWritesSealedFixture() throws {
        try XCTSkipUnless(
            ProcessInfo.processInfo.environment["GLITCH_SPOT_WRITE_FIXTURES"] == "1",
            "set GLITCH_SPOT_WRITE_FIXTURES=1 to write the fixtures again"
        )
        let payload = try Fixtures.sampleDelivery().encoded()
        try payload.write(to: Fixtures.url("wire-snapshot.json"))
        try Seal.seal(payload, for: Fixtures.pairingLink()).write(to: Fixtures.url("sealed-snapshot.bin"))
    }
}
