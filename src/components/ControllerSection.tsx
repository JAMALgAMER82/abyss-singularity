import { useCallback, useEffect, useRef, useState } from "react";
import {
  controllerApplyToRetroarch,
  snapshotGamepads,
  type ControllerKind,
  type DetectedController,
} from "../lib/controller";

/**
 * Settings panel section that lives inside SettingsView.
 *
 * Uses the Web Gamepad API for detection — runs entirely in the webview
 * at 60 Hz so the button visualiser feels live. Only when the user
 * clicks "Smart-configure for RetroArch" do we hit the Rust side to
 * write the joypad autoconfig file.
 */
export function ControllerSection() {
  const [pads, setPads]               = useState<DetectedController[]>([]);
  const [buttonStates, setButtonStates] = useState<Record<number, number[]>>({});
  const [error, setError]             = useState<string | null>(null);
  const [appliedFlash, setAppliedFlash] = useState<string | null>(null);
  const rafRef = useRef<number | null>(null);

  // Re-snapshot connected gamepads when one is plugged in/out.
  useEffect(() => {
    setPads(snapshotGamepads());
    const onChange = () => setPads(snapshotGamepads());
    window.addEventListener("gamepadconnected",    onChange);
    window.addEventListener("gamepaddisconnected", onChange);
    return () => {
      window.removeEventListener("gamepadconnected",    onChange);
      window.removeEventListener("gamepaddisconnected", onChange);
    };
  }, []);

  // Auto-configure on connect: when a new pad is plugged in, silently
  // write its RetroArch autoconfig with `force=false`. The Rust side
  // refuses to overwrite an existing file, so this is a no-op for
  // already-configured controllers and a one-shot wiring for new ones.
  useEffect(() => {
    for (const p of pads) {
      controllerApplyToRetroarch(p.kind, p.id, false).catch(() => {
        // Silent — "already exists" / "no retroarch installed" both
        // legitimately reject. User can still hit the manual button.
      });
    }
  }, [pads]);

  // 60 Hz poll for live button visualisation. Bailing if there are no
  // pads keeps the page idle for users with no controllers.
  useEffect(() => {
    if (pads.length === 0) return;
    const tick = () => {
      const next: Record<number, number[]> = {};
      for (const p of pads) {
        const live = navigator.getGamepads?.()[p.index];
        if (live) next[p.index] = live.buttons.map((b) => b.value);
      }
      setButtonStates(next);
      rafRef.current = requestAnimationFrame(tick);
    };
    rafRef.current = requestAnimationFrame(tick);
    return () => {
      if (rafRef.current !== null) cancelAnimationFrame(rafRef.current);
    };
  }, [pads]);

  const applyRetroarch = useCallback(async (pad: DetectedController) => {
    setError(null); setAppliedFlash(null);
    try {
      const r = await controllerApplyToRetroarch(pad.kind, pad.id, true);
      setAppliedFlash(r.written_to);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  return (
    <section>
      <div className="mb-2 flex items-center justify-between">
        <div>
          <h3 className="text-sm font-semibold text-abyss-fg">Controllers</h3>
          <p className="mt-0.5 text-xs text-abyss-fg-muted">
            Live-detected gamepads. Click <em>Smart-configure for RetroArch</em> to drop a
            joypad autoconfig file into the RetroArch install so every libretro core uses the
            right button layout — no settings-menu tour required.
          </p>
        </div>
      </div>

      <ul className="divide-y divide-abyss-border rounded-md border border-abyss-border bg-abyss-panel/40">
        {pads.length === 0 ? (
          <li className="px-4 py-3 text-xs text-abyss-fg-dim">
            No gamepads detected. Plug one in (USB or Bluetooth) and it'll appear here automatically.
          </li>
        ) : (
          pads.map((p) => (
            <li key={`${p.index}-${p.id}`} className="px-4 py-3">
              <div className="flex items-start gap-3">
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <p className="text-sm font-medium text-abyss-fg">{p.id}</p>
                    <span className={kindBadge(p.kind)}>
                      {prettyKind(p.kind)}
                    </span>
                  </div>
                  <p className="mt-0.5 text-[11px] text-abyss-fg-dim">
                    slot {p.index} · {p.buttons} buttons · {p.axes} axes · mapping={p.mapping}
                  </p>
                  <ButtonStrip state={buttonStates[p.index]} count={p.buttons} />
                </div>
                <button
                  type="button"
                  onClick={() => applyRetroarch(p)}
                  className="h-8 rounded-md border border-abyss-accent/60 bg-abyss-accent/10 px-3 text-sm font-medium text-abyss-accent hover:bg-abyss-accent/20"
                >
                  Smart-configure for RetroArch
                </button>
              </div>
            </li>
          ))
        )}
      </ul>

      {appliedFlash && (
        <p className="mt-2 rounded-sm border border-abyss-success/30 bg-abyss-success/10 px-3 py-2 font-mono text-[10px] text-abyss-success">
          ✓ wrote {appliedFlash}
        </p>
      )}
      {error && (
        <p className="mt-2 rounded-sm border border-abyss-danger/30 bg-abyss-danger/10 px-3 py-2 text-xs text-abyss-danger">
          {error}
        </p>
      )}
    </section>
  );
}

function prettyKind(k: ControllerKind): string {
  switch (k) {
    case "xbox":         return "Xbox / XInput";
    case "play_station": return "PlayStation";
    case "switch_pro":   return "Switch Pro";
    default:             return "Generic";
  }
}

function kindBadge(k: ControllerKind): string {
  const base = "inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[10px] font-mono uppercase tracking-widest";
  switch (k) {
    case "xbox":         return `${base} border-emerald-400/40 bg-emerald-400/10 text-emerald-300`;
    case "play_station": return `${base} border-sky-400/40 bg-sky-400/10 text-sky-300`;
    case "switch_pro":   return `${base} border-red-400/40 bg-red-400/10 text-red-300`;
    default:             return `${base} border-abyss-border bg-abyss-panel-2 text-abyss-fg-muted`;
  }
}

function ButtonStrip({ state, count }: { state: number[] | undefined; count: number }) {
  return (
    <div className="mt-2 flex flex-wrap gap-1">
      {Array.from({ length: count }, (_, i) => {
        const v = state?.[i] ?? 0;
        const active = v > 0.05;
        return (
          <span
            key={i}
            title={`btn ${i}`}
            className={`h-2 w-2 rounded-full transition-colors ${
              active ? "bg-abyss-accent" : "bg-abyss-border-2"
            }`}
            style={active ? { boxShadow: "0 0 6px rgba(61,220,255,0.6)" } : undefined}
          />
        );
      })}
    </div>
  );
}
