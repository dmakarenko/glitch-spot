// ABOUTME: Top-center floating toolbar for the overlay session.
// ABOUTME: Shows the rect count and a Save button. Hides until at least one rect exists.

type Props = {
  rectCount: number;
  onSave: () => void;
};

export function Toolbar({ rectCount, onSave }: Props) {
  if (rectCount === 0) return null;
  return (
    <div
      className="glitch-frame"
      style={{
        position: "fixed",
        top: 12,
        left: "50%",
        transform: "translateX(-50%)",
        borderRadius: 9,
        zIndex: 10,
      }}
    >
      <div
        onMouseDown={(e) => e.stopPropagation()}
        onClick={(e) => e.stopPropagation()}
        style={{
          display: "flex",
          alignItems: "center",
          gap: 12,
          padding: "8px 12px",
          background: "rgba(18,18,18,0.94)",
          color: "#e0e0e0",
          font: "13px/1 system-ui",
          borderRadius: 8,
          border: "1px solid rgba(255,255,255,0.07)",
          boxShadow: "0 4px 20px rgba(0,0,0,0.6)",
          backdropFilter: "blur(8px)",
          WebkitBackdropFilter: "blur(8px)",
          pointerEvents: "auto",
        }}
      >
        <span>{rectCount} rectangle{rectCount === 1 ? "" : "s"}</span>
        <button
          type="button"
          onClick={onSave}
          style={{
            padding: "6px 12px",
            background: "#ee6633",
            color: "white",
            border: "none",
            borderRadius: 4,
            fontWeight: 600,
            cursor: "pointer",
          }}
        >
          Save (⌘↩)
        </button>
      </div>
    </div>
  );
}
