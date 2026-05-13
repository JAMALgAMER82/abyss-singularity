import { NAV_ITEMS, type NavId } from "../lib/nav";
import { NAV_ICONS } from "./icons";

interface SidebarProps {
  active: NavId;
  onSelect: (id: NavId) => void;
}

export function Sidebar({ active, onSelect }: SidebarProps) {
  return (
    <aside
      aria-label="Primary navigation"
      className="
        flex h-full w-[64px] flex-col items-center
        border-r border-abyss-border bg-abyss-panel/80
        backdrop-blur-sm
      "
    >
      {/* Brand mark — also acts as drag handle for the column above title bar. */}
      <div
        className="flex h-14 w-full items-center justify-center"
        aria-hidden
      >
        <div className="relative h-7 w-7">
          <div className="absolute inset-0 rounded-full bg-abyss-accent/15 blur-md" />
          <div className="relative h-full w-full rounded-full border border-abyss-accent/60 abyss-glow" />
        </div>
      </div>

      <nav className="flex flex-1 flex-col items-center gap-1 pt-2">
        {NAV_ITEMS.map((item) => {
          const Icon = NAV_ICONS[item.id];
          const isActive = item.id === active;
          return (
            <button
              key={item.id}
              type="button"
              onClick={() => onSelect(item.id)}
              title={`${item.label}${item.hotkey ? `  (Ctrl+${item.hotkey})` : ""}`}
              aria-current={isActive ? "page" : undefined}
              className={[
                "group relative flex h-11 w-11 items-center justify-center",
                "rounded-md transition-colors duration-150",
                isActive
                  ? "bg-abyss-accent/10 text-abyss-accent"
                  : "text-abyss-fg-muted hover:bg-abyss-panel-2 hover:text-abyss-fg",
              ].join(" ")}
            >
              {/* Active indicator bar */}
              <span
                className={[
                  "absolute left-0 top-1.5 h-8 w-[2px] rounded-r",
                  "transition-all duration-200",
                  isActive ? "bg-abyss-accent abyss-glow" : "bg-transparent",
                ].join(" ")}
              />
              <Icon size={20} />
            </button>
          );
        })}
      </nav>

      <div className="mb-3 text-[10px] font-mono text-abyss-fg-dim">v0.1</div>
    </aside>
  );
}
