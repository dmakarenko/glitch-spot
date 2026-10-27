// ABOUTME: Tests that the SDK parses the Rust-made pairing link and rejects malformed links.
import CryptoKit
import Foundation
import XCTest
@testable import GlitchSpotCore

final class PairingLinkTests: XCTestCase {
    private let valid: [(String, String)] = [
        ("url", "https://glitch-mac.tail-1234.ts.net/receive"),
        ("sender", "snd_4f7c2a9e1b3d"),
        ("secret", "mVPNkvg8706mH5eB-y23YGh_UQdR7xSsBfA-qB018nw"),
        ("key", "2ovEm9AcHkn5yOhN-SW0WHIZk6UKixxYi_ay469feA0"),
    ]

    private func link(_ items: [(String, String)], prefix: String = "glitchspot-pair://v1") -> String {
        let unreserved = CharacterSet(charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~")
        let query = items.map { "\($0.0)=\($0.1.addingPercentEncoding(withAllowedCharacters: unreserved)!)" }
        return "\(prefix)?\(query.joined(separator: "&"))"
    }

    private func replacing(_ name: String, with value: String) -> [(String, String)] {
        valid.map { $0.0 == name ? ($0.0, value) : $0 }
    }

    private func assertRejects(_ string: String, _ expected: PairingLinkError, line: UInt = #line) {
        XCTAssertThrowsError(try PairingLink(parsing: string), line: line) { error in
            XCTAssertEqual(error as? PairingLinkError, expected, line: line)
        }
    }

    func testParsesRustGeneratedLink() throws {
        let inputs = try Fixtures.pairingInputs()
        let keys = try Fixtures.json("desktop-test-key.json")
        let parsed = try Fixtures.pairingLink()

        XCTAssertEqual(parsed.receiveURL.absoluteString, inputs["receive_url"])
        XCTAssertEqual(parsed.senderID, inputs["sender_id"])
        XCTAssertEqual(
            parsed.senderSecret.withUnsafeBytes { Data($0) },
            Data(base64URL: try XCTUnwrap(inputs["sender_secret"]))
        )
        XCTAssertEqual(
            parsed.desktopKey.rawRepresentation,
            Data(base64URL: try XCTUnwrap(keys["public_key"] as? String))
        )
    }

    func testTestHelperMakesTheFixtureLink() throws {
        XCTAssertEqual(link(valid), try Fixtures.text("pairing-link.txt"))
    }

    func testRejectsWrongScheme() {
        assertRejects(link(valid, prefix: "glitchspot://v1"), .wrongScheme)
        assertRejects(link(valid, prefix: "https://v1"), .wrongScheme)
    }

    func testRejectsWrongVersion() {
        assertRejects(link(valid, prefix: "glitchspot-pair://v2"), .wrongVersion)
        assertRejects(link(valid, prefix: "glitchspot-pair://v1/extra"), .wrongVersion)
    }

    func testRejectsKeyThatIsNot32Bytes() {
        assertRejects(link(replacing("key", with: "2ovEm9AcHkn5yOhN-SW0WHIZk6UKixxYi_ay469fe")), .badKey)
        assertRejects(link(replacing("key", with: "2ovEm9AcHkn5yOhN-SW0WHIZk6UKixxYi_ay469feA0AA")), .badKey)
        assertRejects(link(replacing("key", with: "2ovEm9AcHkn5yOhN+SW0WHIZk6UKixxYi/ay469feA0")), .badKey)
    }

    func testRejectsSecretThatIsNot32Bytes() {
        assertRejects(link(replacing("secret", with: "mVPNkvg8706mH5eB")), .badSecret)
    }

    func testRejectsMissingOrEmptyFields() {
        for name in ["url", "sender", "secret", "key"] {
            assertRejects(link(valid.filter { $0.0 != name }), .missingField(name))
            assertRejects(link(replacing(name, with: "")), .missingField(name))
        }
    }

    func testRejectsNonHTTPSURL() {
        assertRejects(link(replacing("url", with: "http://glitch-mac.tail-1234.ts.net/receive")), .insecureURL)
        assertRejects(link(replacing("url", with: "https:///receive")), .insecureURL)
        assertRejects(link(replacing("url", with: "not a url")), .insecureURL)
    }

    func testRejectsDuplicateParameters() {
        for name in ["url", "sender", "secret", "key"] {
            let duplicate = valid.first { $0.0 == name }!
            assertRejects(link(valid + [duplicate]), .duplicateField(name))
        }
    }
}
