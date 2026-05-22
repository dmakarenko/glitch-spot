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
      onMouseDown={(e) => e.stopPropagation()}
      onClick={(e) => e.stopPropagation()}
      style={{
        position: "fixed",
        top: 12,
        left: "50%",
        transform: "translateX(-50%)",
        display: "flex",
        alignItems: "center",
        gap: 12,
        padding: "8px 12px",
        background: "rgba(20,20,20,0.85)",
        color: "white",
        font: "13px/1 system-ui",
        borderRadius: 8,
        boxShadow: "0 4px 18px rgba(0,0,0,0.35)",
        pointerEvents: "auto",
        zIndex: 10,
      }}
    >
      <span>{rectCount} rectangle{rectCount === 1 ? "" : "s"}</span>
      <button
        type="button"
        onClick={onSave}
        style={{
          padding: "6px 12px",
          background: "#0a84ff",
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
  );
}
