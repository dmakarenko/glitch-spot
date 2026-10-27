# Glitch Spot

A tool that turns a marked-up screenshot into a structured snapshot that AI coding agents read.

## Language

**Snapshot**:
One marked-up screenshot: the clean image, the annotated image, its annotations and redactions, and the context of the device and app.
_Avoid_: capture, report, bug report

**Annotation**:
A labelled rectangle on a snapshot with the user's note for that area.
_Avoid_: markup, comment, pin

**Redaction**:
An area of a snapshot whose pixels are destroyed in both images before the snapshot is saved or sent.
_Avoid_: mask, blur, censor

**Desktop app**:
The Glitch Spot macOS menu-bar app, which captures, stores, and serves snapshots.
_Avoid_: work computer, Mac app, standalone app

**Host app**:
An app that embeds a Glitch Spot SDK. The first iOS host app is Ondera.
_Avoid_: client app, integrating app

**Receive endpoint**:
The write-only address on the Desktop app that accepts snapshots from a paired device, reached through a tunnel.
_Avoid_: inbox, upload server, ingest

**Sender**:
A device that is allowed to send snapshots to the Desktop app. Only Dmitry's own devices are senders.
_Avoid_: client, tester, peer

**Pairing**:
The one-time exchange in which a Sender scans a QR code from the Desktop app and gets the receive endpoint URL, a sender secret, and the Desktop app's public key.
_Avoid_: linking, registration, handshake

**Sealed snapshot**:
A snapshot that a Sender encrypts with the Desktop app's public key, so that only the Desktop app can read it.
_Avoid_: encrypted payload, envelope
