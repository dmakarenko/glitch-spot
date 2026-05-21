// ABOUTME: Root application component for the main Glitch Spot window.
// ABOUTME: Placeholder UI until the capture and overlay features are wired up.
import { invoke } from "@tauri-apps/api/core";
import { useState } from "react";

export function App() {
  const [result, setResult] = useState<string>("");

  async function run() {
    try {
      const path = await invoke<string>("spike_capture");
      setResult(`Saved to ${path}`);
    } catch (e) {
      setResult(`Failed: ${e}`);
    }
  }

  return (
    <main style={{ padding: "2rem", fontFamily: "system-ui" }}>
      <h1>Glitch Spot</h1>
      <button onClick={run}>Run capture spike</button>
      <pre>{result}</pre>
    </main>
  );
}
