// ABOUTME: The snapshot JSON that the SDK seals and sends to the Desktop app (the Electron shape plus device context).
import Foundation

/// One upload: the snapshot plus the id that lets the Desktop app drop a retried duplicate.
public struct Delivery: Encodable, Sendable {
    public static let version = 1

    public let v: Int
    public let deliveryID: UUID
    public var snapshot: Snapshot

    public init(deliveryID: UUID = UUID(), snapshot: Snapshot) {
        self.v = Delivery.version
        self.deliveryID = deliveryID
        self.snapshot = snapshot
    }

    enum CodingKeys: String, CodingKey {
        case v
        case deliveryID = "delivery_id"
        case snapshot
    }

    /// The bytes that the SDK seals: sorted keys, standard padded base64 images, no line breaks.
    public func encoded() throws -> Data {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.sortedKeys, .withoutEscapingSlashes]
        return try encoder.encode(self)
    }
}

public struct Snapshot: Encodable, Sendable {
    public var id: String
    public var createdAt: String
    public var mode: String
    public var annotations: [Annotation]
    public var redactions: [Rect]
    public var context: Context
    public var images: Images

    public init(
        id: String,
        createdAt: String,
        mode: String,
        annotations: [Annotation],
        redactions: [Rect],
        context: Context,
        images: Images
    ) {
        self.id = id
        self.createdAt = createdAt
        self.mode = mode
        self.annotations = annotations
        self.redactions = redactions
        self.context = context
        self.images = images
    }

    enum CodingKeys: String, CodingKey {
        case id
        case createdAt = "created_at"
        case mode, annotations, redactions, context, images
    }

    public struct Context: Encodable, Sendable {
        public var screen: Screen
        public var os: OS
        public var app: App
        public var device: Device

        public init(screen: Screen, os: OS, app: App, device: Device) {
            self.screen = screen
            self.os = os
            self.app = app
            self.device = device
        }
    }

    public struct Screen: Encodable, Sendable {
        public var width: Int
        public var height: Int
        public var scaleFactor: Double

        public init(width: Int, height: Int, scaleFactor: Double) {
            self.width = width
            self.height = height
            self.scaleFactor = scaleFactor
        }

        enum CodingKeys: String, CodingKey {
            case width, height
            case scaleFactor = "scale_factor"
        }
    }

    public struct OS: Encodable, Sendable {
        public var platform: String
        public var version: String

        public init(platform: String, version: String) {
            self.platform = platform
            self.version = version
        }
    }

    public struct App: Encodable, Sendable {
        public var name: String
        public var version: String
        public var build: String
        public var metadata: [String: String]?

        public init(name: String, version: String, build: String, metadata: [String: String]? = nil) {
            self.name = name
            self.version = version
            self.build = build
            self.metadata = metadata
        }
    }

    public struct Device: Encodable, Sendable {
        public var model: String

        public init(model: String) {
            self.model = model
        }
    }

    public struct Images: Encodable, Sendable {
        public var clean: Data
        public var annotated: Data

        public init(clean: Data, annotated: Data) {
            self.clean = clean
            self.annotated = annotated
        }

        enum CodingKeys: String, CodingKey {
            case clean = "clean_base64"
            case annotated = "annotated_base64"
        }
    }
}

public struct Annotation: Encodable, Sendable {
    public var id: String
    public var shape: String
    public var coords: Rect
    public var note: String
    public var label: String

    public init(id: String, shape: String, coords: Rect, note: String, label: String) {
        self.id = id
        self.shape = shape
        self.coords = coords
        self.note = note
        self.label = label
    }
}

/// A rectangle in image pixels.
public struct Rect: Encodable, Sendable {
    public var x: Double
    public var y: Double
    public var w: Double
    public var h: Double

    public init(x: Double, y: Double, w: Double, h: Double) {
        self.x = x
        self.y = y
        self.w = w
        self.h = h
    }
}
