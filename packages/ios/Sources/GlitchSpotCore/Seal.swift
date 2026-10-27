// ABOUTME: Seals a delivery with HPKE (PSK mode) to the Desktop app's public key from pairing.
import CryptoKit
import Foundation

public enum SealError: Error, Equatable, Sendable {
    case tooLarge(Int)
}

/// The sealed body is the version byte, the encapsulated key, then the ciphertext.
public enum Seal {
    static let info = Data("glitch-spot sealed snapshot v1".utf8)
    static let versionByte: UInt8 = 0x01
    static let encapsulatedKeyBytes = 32
    static let tagBytes = 16
    static let maxBytes = 25 * 1024 * 1024

    /// Uses one HPKE context per upload, with an empty AAD.
    public static func seal(_ payload: Data, for link: PairingLink) throws -> Data {
        let size = 1 + encapsulatedKeyBytes + payload.count + tagBytes
        guard size <= maxBytes else { throw SealError.tooLarge(size) }

        var sender = try HPKE.Sender(
            recipientKey: link.desktopKey,
            ciphersuite: .Curve25519_SHA256_ChachaPoly,
            info: info,
            presharedKey: link.senderSecret,
            presharedKeyIdentifier: Data(link.senderID.utf8)
        )
        let ciphertext = try sender.seal(payload)
        return Data([versionByte]) + sender.encapsulatedKey + ciphertext
    }
}
