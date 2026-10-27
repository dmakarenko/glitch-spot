// ABOUTME: Tests the HPKE seal against the pinned contract and the committed Swift-sealed fixture.
import CryptoKit
import Foundation
import XCTest
@testable import GlitchSpotCore

final class SealTests: XCTestCase {
    func testConstantsMatchContractJSON() throws {
        let contract = try Fixtures.json("contract.json")
        let hpke = try XCTUnwrap(contract["hpke"] as? [String: Any])
        let body = try XCTUnwrap(contract["sealed_body"] as? [String: Any])
        let payload = try XCTUnwrap(contract["payload"] as? [String: Any])

        XCTAssertEqual(Seal.info, Data(try XCTUnwrap(hpke["info"] as? String).utf8))
        XCTAssertEqual(hpke["aad"] as? String, "")
        XCTAssertEqual(Int(Seal.versionByte), body["version_byte"] as? Int)
        XCTAssertEqual(Seal.encapsulatedKeyBytes, body["encapsulated_key_bytes"] as? Int)
        XCTAssertEqual(Seal.tagBytes, body["tag_bytes"] as? Int)
        XCTAssertEqual(Seal.maxBytes, body["max_bytes"] as? Int)
        XCTAssertEqual(Delivery.version, payload["v"] as? Int)
    }

    func testSealMatchesPinnedLayout() throws {
        let plaintext = try Fixtures.data("wire-snapshot.json")
        let committed = try Fixtures.data("sealed-snapshot.bin")
        let fresh = try Seal.seal(plaintext, for: Fixtures.pairingLink())

        for body in [committed, fresh] {
            XCTAssertEqual(body.first, 0x01)
            XCTAssertEqual(body.count, 1 + 32 + plaintext.count + 16)
            XCTAssertEqual(try Fixtures.open(body), plaintext)
        }
    }

    func testSealsTheProductionEncodingOfADelivery() throws {
        let payload = try Fixtures.sampleDelivery().encoded()
        let body = try Seal.seal(payload, for: Fixtures.pairingLink())
        XCTAssertEqual(try Fixtures.open(body), payload)
    }

    func testEachSealUsesAFreshEncapsulatedKey() throws {
        let link = try Fixtures.pairingLink()
        let first = try Seal.seal(Data("x".utf8), for: link)
        let second = try Seal.seal(Data("x".utf8), for: link)
        XCTAssertNotEqual(first.subdata(in: 1..<33), second.subdata(in: 1..<33))
    }

    func testRejectsBodyOverTheLimit() throws {
        let link = try Fixtures.pairingLink()
        let tooBig = Data(count: Seal.maxBytes - 1 - 32 - 16 + 1)
        XCTAssertThrowsError(try Seal.seal(tooBig, for: link)) { error in
            XCTAssertEqual(error as? SealError, .tooLarge(Seal.maxBytes + 1))
        }
        XCTAssertNoThrow(try Seal.seal(Data(count: tooBig.count - 1), for: link))
    }
}
