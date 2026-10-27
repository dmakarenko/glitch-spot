// ABOUTME: Parses the glitchspot-pair link that the Desktop app shows as a QR code.
import CryptoKit
import Foundation

public enum PairingLinkError: Error, Equatable, Sendable {
    case wrongScheme
    case wrongVersion
    case missingField(String)
    case duplicateField(String)
    case insecureURL
    case badSecret
    case badKey
}

/// What a Sender keeps after pairing: where to upload, who it is, and what to seal to.
public struct PairingLink: Sendable {
    public let receiveURL: URL
    public let senderID: String
    let senderSecret: SymmetricKey
    let desktopKey: Curve25519.KeyAgreement.PublicKey

    public init(parsing string: String) throws(PairingLinkError) {
        guard let components = URLComponents(string: string),
              components.scheme == "glitchspot-pair"
        else { throw .wrongScheme }
        guard components.host == "v1", components.path.isEmpty else { throw .wrongVersion }

        var fields: [String: String] = [:]
        for item in components.queryItems ?? [] {
            guard fields[item.name] == nil else { throw .duplicateField(item.name) }
            fields[item.name] = item.value ?? ""
        }
        func field(_ name: String) throws(PairingLinkError) -> String {
            guard let value = fields[name], !value.isEmpty else { throw .missingField(name) }
            return value
        }

        let url = try field("url")
        let sender = try field("sender")
        let secret = try field("secret")
        let key = try field("key")

        guard let receiveURL = URL(string: url),
              receiveURL.scheme == "https",
              receiveURL.host?.isEmpty == false
        else { throw .insecureURL }
        guard let secretBytes = Data(base64URL: secret), secretBytes.count == 32 else { throw .badSecret }
        guard let keyBytes = Data(base64URL: key), keyBytes.count == 32,
              let desktopKey = try? Curve25519.KeyAgreement.PublicKey(rawRepresentation: keyBytes)
        else { throw .badKey }

        self.receiveURL = receiveURL
        self.senderID = sender
        self.senderSecret = SymmetricKey(data: secretBytes)
        self.desktopKey = desktopKey
    }
}

extension Data {
    /// Decodes base64url without padding; returns nil for any other alphabet or for padding.
    init?(base64URL string: String) {
        let alphabet = CharacterSet(charactersIn: "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_")
        guard string.unicodeScalars.allSatisfy(alphabet.contains) else { return nil }
        var base64 = string.replacingOccurrences(of: "-", with: "+").replacingOccurrences(of: "_", with: "/")
        base64 += String(repeating: "=", count: (4 - base64.count % 4) % 4)
        self.init(base64Encoded: base64)
    }
}
