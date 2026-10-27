// ABOUTME: Loads the shared ios-contract fixtures and builds the sample delivery that the tests seal.
import CryptoKit
import Foundation
import XCTest
@testable import GlitchSpotCore

enum Fixtures {
    static let directory = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .deletingLastPathComponent()
        .appendingPathComponent("tests/fixtures/ios-contract")

    static func url(_ name: String) -> URL { directory.appendingPathComponent(name) }

    static func data(_ name: String) throws -> Data { try Data(contentsOf: url(name)) }

    static func text(_ name: String) throws -> String { String(decoding: try data(name), as: UTF8.self) }

    static func json(_ name: String) throws -> [String: Any] {
        try XCTUnwrap(JSONSerialization.jsonObject(with: data(name)) as? [String: Any])
    }

    static func pairingInputs() throws -> [String: String] {
        try XCTUnwrap(json("pairing-inputs.json") as? [String: String])
    }

    static func desktopPrivateKey() throws -> Curve25519.KeyAgreement.PrivateKey {
        let key = try XCTUnwrap(json("desktop-test-key.json")["private_key"] as? String)
        return try Curve25519.KeyAgreement.PrivateKey(rawRepresentation: XCTUnwrap(Data(base64URL: key)))
    }

    static func pairingLink() throws -> PairingLink {
        try PairingLink(parsing: text("pairing-link.txt"))
    }

    /// Opens a sealed body with the test key and the sender secret from `pairing-inputs.json`.
    static func open(_ body: Data) throws -> Data {
        let inputs = try pairingInputs()
        let secret = try XCTUnwrap(Data(base64URL: XCTUnwrap(inputs["sender_secret"])))
        let encapsulatedKey = body.subdata(in: 1..<33)
        var recipient = try HPKE.Recipient(
            privateKey: desktopPrivateKey(),
            ciphersuite: .Curve25519_SHA256_ChachaPoly,
            info: Data("glitch-spot sealed snapshot v1".utf8),
            encapsulatedKey: encapsulatedKey,
            presharedKey: SymmetricKey(data: secret),
            presharedKeyIdentifier: Data(XCTUnwrap(inputs["sender_id"]).utf8)
        )
        return try recipient.open(body.subdata(in: 33..<body.count))
    }

    static let png = Data(base64Encoded: "iVBORw0KGgoAAAANSUhEUgAAAAgAAAAICAYAAADED76LAAAAXElEQVR42hXKMQEDQQgAsJPyUpCCFKQgBSk4acOQLe+9+n0ESdEMy3ufQJAUzbDfhRAIkqIZNi6kQJAUzbB5oQSCpGiGrQstECRFM2xfGIEgKZph58IKBEnRDMsf+pGXwU1D2IUAAAAASUVORK5CYII=")!

    static func sampleDelivery() -> Delivery {
        Delivery(
            deliveryID: UUID(uuidString: "6F1C2B8E-4D3A-4E5F-9A0B-1C2D3E4F5A6B")!,
            snapshot: Snapshot(
                id: "../../etc",
                createdAt: "2026-10-27T16:30:00Z",
                mode: "frozen",
                annotations: [
                    Annotation(
                        id: "a1",
                        shape: "rect",
                        coords: Rect(x: 1, y: 1, w: 5, h: 3),
                        note: "Total is cut off",
                        label: "01"
                    ),
                ],
                redactions: [Rect(x: 4, y: 4, w: 4, h: 4)],
                context: Snapshot.Context(
                    screen: .init(width: 8, height: 8, scaleFactor: 3),
                    os: .init(platform: "iOS", version: "18.6"),
                    app: .init(name: "Ondera", version: "2.4.0", build: "512", metadata: ["screen": "Checkout"]),
                    device: .init(model: "iPhone17,1")
                ),
                images: .init(clean: png, annotated: png)
            )
        )
    }
}
