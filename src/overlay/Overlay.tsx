// ABOUTME: Translucent overlay window component used as a visual placeholder.
// ABOUTME: Task 5.1 will replace this with a screenshot backdrop.
export function Overlay() {
  return (
    <div
      style={{
        position: "fixed",
        inset: 0,
        background: "rgba(0, 64, 255, 0.15)",
        pointerEvents: "auto",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        color: "white",
        font: "600 32px system-ui",
        textShadow: "0 1px 2px rgba(0,0,0,0.4)",
      }}
    >
      Glitch Spot overlay spike
    </div>
  );
}
