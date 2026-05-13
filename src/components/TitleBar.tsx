import { getCurrentWindow } from "@tauri-apps/api/window";
import { CloseIcon, MaximizeIcon, MinimizeIcon } from "./icons";
import { useCouch } from "../lib/couch";

interface TitleBarProps {
  title: string;
  subtitle?: string;
}

/**
 * Custom title bar. The Tauri window is created with `decorations: false`
 * (see tauri.conf.json) so we render our own — keeps the abyss aesthetic
 * coherent across platforms.
 */
export function TitleBar({ title, subtitle }: TitleBarProps) {
  const win = getCurrentWindow();
  const { couch, toggle } = useCouch();

  return (
    <div
      data-tauri-drag-region
      className="
        flex h-9 shrink-0 items-center justify-between
        border-b border-abyss-border bg-abyss-panel/60
        px-3 select-none
      "
    >
      <div data-tauri-drag-region className="flex items-center gap-2 text-xs">
        <span className="font-semibold tracking-wider text-abyss-accent abyss-text-glow">
          ABYSS&nbsp;SINGULARITY
        </span>
        <span className="text-abyss-fg-dim">/</span>
        <span className="text-abyss-fg-muted">{title}</span>
        {subtitle && (
          <>
            <span className="text-abyss-fg-dim">·</span>
            <span className="text-abyss-fg-dim">{subtitle}</span>
          </>
        )}
      </div>

      <div className="flex items-center gap-0.5">
        <button
          type="button"
          onClick={toggle}
          title={couch ? "Exit big-picture mode (F11 or Start)" : "Enter big-picture / couch mode (F11 or Start)"}
          aria-label="Toggle couch mode"
          className={[
            "inline-flex h-7 items-center gap-1.5 rounded-sm px-2 mr-1 text-[10px] font-mono uppercase tracking-widest transition-colors",
            couch
              ? "border border-abyss-accent/60 bg-abyss-accent/10 text-abyss-accent"
              : "text-abyss-fg-dim hover:bg-abyss-panel-2 hover:text-abyss-fg",
          ].join(" ")}
        >
          <TvIcon size={12} />
          {couch ? "couch" : "couch"}
        </button>
        <WindowButton onClick={() => win.minimize()} aria-label="Minimize">
          <MinimizeIcon size={14} />
        </WindowButton>
        <WindowButton onClick={() => win.toggleMaximize()} aria-label="Maximize">
          <MaximizeIcon size={12} />
        </WindowButton>
        <WindowButton onClick={() => win.close()} aria-label="Close" danger>
          <CloseIcon size={14} />
        </WindowButton>
      </div>
    </div>
  );
}

function TvIcon({ size = 12 }: { size?: number }) {
  return (
    <svg
      width={size} height={size} viewBox="0 0 24 24"
      fill="none" stroke="currentColor" strokeWidth={2}
      strokeLinecap="round" strokeLinejoin="round"
    >
      <rect x="2" y="4" width="20" height="14" rx="2" />
      <path d="M8 21h8M12 18v3" />
    </svg>
  );
}

function WindowButton({
  children,
  danger,
  ...rest
}: React.ButtonHTMLAttributes<HTMLButtonElement> & { danger?: boolean }) {
  return (
    <button
      type="button"
      className={[
        "inline-flex h-7 w-9 items-center justify-center rounded-sm",
        "text-abyss-fg-muted transition-colors",
        danger
          ? "hover:bg-abyss-danger/80 hover:text-white"
          : "hover:bg-abyss-panel-2 hover:text-abyss-fg",
      ].join(" ")}
      {...rest}
    >
      {children}
    </button>
  );
}
