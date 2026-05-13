interface PlaceholderViewProps {
  title: string;
  phase: number;
  description: string;
}

/**
 * A common placeholder for views that will be wired up in later phases.
 * Renders a "scheduled" panel so the operator can see at a glance which
 * subsystem this section will host once its phase is implemented.
 */
export function PlaceholderView({ title, phase, description }: PlaceholderViewProps) {
  return (
    <div className="flex h-full w-full items-center justify-center p-10">
      <div
        className="
          relative w-full max-w-2xl rounded-xl border border-abyss-border
          bg-abyss-panel/60 p-8 shadow-[0_8px_40px_-12px_rgba(0,0,0,0.6)]
        "
      >
        <div className="mb-3 inline-flex items-center gap-2 rounded-full border border-abyss-accent/30 bg-abyss-accent/5 px-3 py-1 text-[11px] font-mono uppercase tracking-widest text-abyss-accent">
          <span className="h-1.5 w-1.5 rounded-full bg-abyss-accent abyss-glow" />
          Phase {phase} · Scheduled
        </div>

        <h2 className="text-2xl font-semibold text-abyss-fg abyss-text-glow">
          {title}
        </h2>
        <p className="mt-3 max-w-prose text-sm leading-relaxed text-abyss-fg-muted">
          {description}
        </p>

        <div className="mt-6 grid grid-cols-3 gap-3">
          {[0, 1, 2].map((i) => (
            <div
              key={i}
              className="h-20 rounded-md border border-dashed border-abyss-border-2 bg-abyss-panel-2/40"
            />
          ))}
        </div>
      </div>
    </div>
  );
}
