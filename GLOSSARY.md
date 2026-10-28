# Glitch Spot

A tool that turns a marked-up screenshot into a structured snapshot that AI coding agents read.

## Language

**Snapshot**:
One marked-up screenshot: the clean image, the annotated image, its annotations and redactions, and the context of the device and app.
_Avoid_: capture, report, bug report

**Frame**:
The captured screen image that the overlay shows and that a save builds a snapshot from.
_Avoid_: screenshot, capture

**Annotation**:
A labelled rectangle on a snapshot with the user's note for that area.
_Avoid_: markup, comment, pin

**Redaction**:
A region that the user covers so its pixels do not leave the Mac in either image of a snapshot.
_Avoid_: mask, blur, censor

**Redaction style**:
How a redaction covers its region: solid fill, pixelated, or mosaic.

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

**Share link**:
A public URL, served through a tunnel, that gives one snapshot to a person who has the link.
_Avoid_: share URL, public link

**Tunnel**:
The outside program (cloudflared, ngrok, or Tailscale Funnel) that makes the local share server reachable from the internet.
