---
date: 2026-10-27
pr: dmakarenko/glitch-spot#2
branch: ios-sealed-contract-20261027
---

# Sealed snapshot contract between iPhone and Desktop

## Engineering
- `Package.swift` at the repo root declares one product, `GlitchSpotKit`, with no dependencies. Its internal target `GlitchSpotCore` (`packages/ios/`) encodes the wire snapshot, parses the `glitchspot-pair://v1` link, and seals with CryptoKit HPKE in PSK mode.
- `src-tauri/src/seal/` opens a sealed body (version byte `0x01`, 32-byte encapsulated key, ciphertext) with `hpke 0.13` and makes the pairing link. `src-tauri/src/snapshot/wire.rs` turns a wire snapshot into a `BuiltSnapshot` with a Desktop-made id. It also applies the redactions again and checks the scale, the redaction rectangles, and the image size.
- The Desktop `Snapshot.context` gets optional `app` and `device` blocks, and a v0.10.2 `snapshot.json` still loads unchanged.
- The fixtures in `tests/fixtures/ios-contract/` are made by one side and tested by the other. `GLOSSARY.md` and ADR 0001 record the language and the transport decision.

## User value
Nothing changes in the app yet. This work proves that an iPhone can seal a snapshot that only the paired Desktop app can open, through any tunnel provider. That was the largest technical risk of the iOS SDK.

## Initiative positioning
This is the first of four stages of the Glitch Spot iOS SDK initiative. The next two stages can now start in parallel: iOS screenshot capture and overlay, and the Desktop receive endpoint with iPhone pairing. The last stage, iOS pairing and sealed send, joins them for the end-to-end path from phone to MCP.
