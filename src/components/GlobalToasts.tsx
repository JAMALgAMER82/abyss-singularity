import { useEffect, useState } from "react";
import { onTransferEvent } from "../lib/transfer";

/**
 * App-wide toast layer. Surfaces incoming-game transfers and errors —
 * each player owns their own save files locally, so there is no
 * cross-machine save sync to notify about.
 *
 * Toasts auto-dismiss after 6 s; click to dismiss earlier.
 */
export function GlobalToasts() {
  const [toasts, setToasts] = useState<Array<{ id: string; body: string; tone: "game" | "error" }>>([]);

  useEffect(() => {
    let unlisten: undefined | (() => void);
    onTransferEvent((e) => {
      if (e.kind === "completed") {
        if (!e.sha256_ok) {
          push({ tone: "error", body: "Incoming transfer failed SHA-256 verification (file discarded)." });
          return;
        }
        if (e.final_path) {
          const name = e.final_path.split(/[\\/]/).pop() ?? "file";
          push({ tone: "game", body: `↘ Received ${name}` });
        }
      } else if (e.kind === "failed") {
        push({ tone: "error", body: `Transfer failed: ${e.message}` });
      }
    }).then((u) => { unlisten = u; });
    return () => unlisten?.();
  }, []);

  function push(t: { tone: "game" | "error"; body: string }) {
    const id = `${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
    setToasts((prev) => [...prev, { id, ...t }]);
    setTimeout(() => {
      setToasts((prev) => prev.filter((x) => x.id !== id));
    }, 6000);
  }

  if (toasts.length === 0) return null;

  return (
    <div className="pointer-events-none fixed bottom-4 right-4 z-[70] flex w-80 flex-col gap-2">
      {toasts.map((t) => (
        <button
          key={t.id}
          type="button"
          onClick={() => setToasts((prev) => prev.filter((x) => x.id !== t.id))}
          className={[
            "pointer-events-auto rounded-md border px-3 py-2 text-left text-xs shadow-xl backdrop-blur",
            t.tone === "error"
              ? "border-abyss-danger/40 bg-abyss-panel/95 text-abyss-danger"
              : "border-abyss-accent/40 bg-abyss-panel/95 text-abyss-accent",
          ].join(" ")}
        >
          {t.body}
        </button>
      ))}
    </div>
  );
}
