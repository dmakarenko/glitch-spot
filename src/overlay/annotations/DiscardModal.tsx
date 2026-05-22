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
        // Click outside the dialog cancels.
        if (e.target === e.currentTarget) onCancel();
      }}
      style={{
        position: "fixed",
        inset: 0,
        background: "rgba(0,0,0,0.35)",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        zIndex: 100,
      }}
    >
      <div
        style={{
          background: "white",
          color: "#111",
          minWidth: 320,
          padding: "20px 22px",
          borderRadius: 10,
          boxShadow: "0 16px 48px rgba(0,0,0,0.4)",
          font: "14px/1.5 system-ui",
        }}
      >
        <h2 style={{ margin: "0 0 8px", font: "600 16px/1.3 system-ui" }}>
          Discard annotations?
        </h2>
        <p style={{ margin: "0 0 16px", color: "#555" }}>
          You have unsaved rectangles. Save them to the clipboard, or discard?
        </p>
        <div style={{ display: "flex", justifyContent: "flex-end", gap: 8 }}>
          <button
            type="button"
            onClick={onCancel}
            style={btn("transparent", "#444")}
          >
            Cancel
          </button>
          <button
            type="button"
            onClick={onDiscard}
            style={btn("#fff", "#b00020", "1px solid #b00020")}
          >
            Discard
          </button>
          <button
            type="button"
            onClick={onSave}
            style={btn("#0a84ff", "white")}
          >
            Save (⌘↩)
          </button>
        </div>
      </div>
    </div>
  );
}

function btn(bg: string, color: string, border = "none"): React.CSSProperties {
  return {
    padding: "6px 12px",
    background: bg,
    color,
    border,
    borderRadius: 4,
    cursor: "pointer",
    font: "13px/1 system-ui",
  };
}
