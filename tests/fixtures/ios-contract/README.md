# iOS contract fixtures

Shared fixtures for the data contract between the iPhone SDK (`GlitchSpotCore`) and the Desktop app (`src-tauri`). Each side makes some of them and tests the files that the other side makes.

**`desktop-test-key.json` is a TEST-ONLY key pair.** Its private key is public in this repository. Never use it outside tests.

| File | Made by | Tested by |
| --- | --- | --- |
| `contract.json` | hand | Swift and Rust compare their constants with it |
| `desktop-test-key.json`, `pairing-inputs.json` | hand | both |
| `pairing-link.txt` | Rust (`seal::pairing::link`) | Swift parses it |
| `wire-snapshot.json`, `sealed-snapshot.bin` | Swift (`FixtureWriterTests`) | Rust opens and converts it |
| `snapshot-v0.10.2.json` | the v0.10.2 Desktop serializer | Rust loads it |

To make the Swift fixtures again, run from the repo root:

```bash
GLITCH_SPOT_WRITE_FIXTURES=1 swift test --filter FixtureWriterTests
```
