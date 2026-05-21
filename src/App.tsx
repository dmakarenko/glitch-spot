// ABOUTME: Root application component for the main Glitch Spot window.
// ABOUTME: Placeholder UI until the capture and overlay features are wired up.
import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";

type CapturedFrame = {
  width: number;
  height: number;
  scale_factor: number;
  png: number[];
  display_index: number;
};

export function App() {
  const [result, setResult] = useState<string>("");

  async function runCapture() {
    try {
      const frame = await invoke<CapturedFrame>("capture_active_monitor");
      setResult(
        `Captured ${frame.width}x${frame.height} @ ${frame.scale_factor}x from display ${frame.display_index}, ${frame.png.length} PNG bytes`
      );
    } catch (e) {
      setResult(`Failed: ${e}`);
    }
  }

  async function runSpike() {
    try {
      const path = await invoke<string>("spike_capture");
      setResult(`Spike saved to ${path}`);
    } catch (e) {
      setResult(`Spike failed: ${e}`);
    }
  }

  return (
    <main style={{ padding: "2rem", fontFamily: "system-ui" }}>
      <h1>Glitch Spot</h1>
      <button onClick={runCapture}>Capture active monitor</button>
      <button onClick={runSpike} style={{ marginLeft: 8 }}>
        Run spike (file)
      </button>
      <button
        onClick={() =>
          invoke("open_overlay_with_capture").catch((e) =>
            setResult(`Failed: ${e}`)
          )
        }
        style={{ marginLeft: 8 }}
      >
        Open overlay
      </button>
      <button
        onClick={() =>
          invoke("hide_overlay").catch((e) => setResult(`Failed: ${e}`))
        }
        style={{ marginLeft: 8 }}
      >
        Hide overlay
      </button>
      <pre>{result}</pre>
    </main>
  );
}
