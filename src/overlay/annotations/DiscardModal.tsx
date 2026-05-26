// ABOUTME: Confirm modal shown when Esc is pressed with at least one rectangle drawn.
// ABOUTME: Lets the user discard the session or save it.

type Props = {
  onDiscard: () => void;
  onSave: () => void;
  onCancel: () => void;
};

export function DiscardModal({ onDiscard, onSave, onCancel }: Props) {
  return (
    <div
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onCancel();
      }}
      style={{
        position: "fixed",
        inset: 0,
        background: "rgba(0,0,0,0.6)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 100,
      }}
    >
      <div
        className="glitch-frame"
        style={{ borderRadius: 11 }}
      >
        <div
          style={{
            background: "#1c1c1e",
            color: "#e5e5e7",
            minWidth: 320,
            padding: "20px 22px",
            borderRadius: 10,
            border: "1px solid rgba(255,255,255,0.07)",
            boxShadow: "0 16px 48px rgba(0,0,0,0.7)",
            font: "14px/1.5 system-ui",
          }}
        >
          <h2 style={{ margin: "0 0 8px", font: "600 16px/1.3 system-ui" }}>
            Discard annotations?
          </h2>
          <p style={{ margin: "0 0 16px", color: "#888" }}>
            You have unsaved rectangles. Save them to the clipboard, or discard?
          </p>
          <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
            <button type="button" onClick={onCancel} style={btnCancel()}>
              Cancel
            </button>
            <button type="button" onClick={onDiscard} style={btnDiscard()}>
              Discard
            </button>
            <button type="button" onClick={onSave} style={btnSave()}>
              Save (⌘↩)
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

function btnCancel(): React.CSSProperties {
  return {
    padding: "6px 12px",
    background: "rgba(255,255,255,0.07)",
    color: "#ccc",
    border: "none",
    borderRadius: 4,
    cursor: "pointer",
    font: "13px/1 system-ui",
  };
}

function btnDiscard(): React.CSSProperties {
  return {
    padding: "6px 12px",
    background: "transparent",
    color: "#ff453a",
    border: "1px solid rgba(255,69,58,0.35)",
    borderRadius: 4,
    cursor: "pointer",
    font: "13px/1 system-ui",
  };
}

function btnSave(): React.CSSProperties {
  return {
    padding: "6px 12px",
    background: "#ee6633",
    color: "white",
    border: "none",
    borderRadius: 4,
    cursor: "pointer",
    font: "13px/1 system-ui",
  };
}
