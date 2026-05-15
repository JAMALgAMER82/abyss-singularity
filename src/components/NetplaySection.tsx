import { useCallback, useEffect, useState } from "react";
import {
  myProfile,
  recommendPair,
  tailscaleStatus,
  type LatencyProfile,
  type RecommendedRegion,
  type TailscaleStatus,
} from "../lib/network";

/**
 * Real-netplay helper panel. Shows the user's own tailnet IP (big,
 * one-click copy), the list of online peers with their reachable
 * tailnet IPs, and a quick latency-profile-exchange flow that calls
 * `recommend_pair` once both sides paste each other's profile —
 * picking the DERP region with the lowest *worst-case* RTT, so neither
 * player gets an unfair lag advantage.
 *
 * Netplay support is per-emulator: Dolphin, RetroArch, PPSSPP all have
 * built-in netplay menus. The panel just hands the user the IPs +
 * region hint; the actual session setup happens in the emulator's
 * own netplay UI.
 */
export function NetplaySection() {
  const [ts, setTs]                 = useState<TailscaleStatus | null>(null);
  const [mine, setMine]             = useState<LatencyProfile | null>(null);
  const [friendProfile, setFriend]  = useState("");
  const [pairResult, setPairResult] = useState<RecommendedRegion | null | "pending">("pending");
  const [error, setError]           = useState<string | null>(null);
  const [copied, setCopied]         = useState<string | null>(null);

  useEffect(() => {
    tailscaleStatus().then(setTs).catch(() => setTs(null));
    myProfile().then(setMine).catch(() => setMine(null));
    const t = setInterval(() => {
      tailscaleStatus().then(setTs).catch(() => {});
    }, 5000);
    return () => clearInterval(t);
  }, []);

  const copy = useCallback(async (label: string, text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(label);
      setTimeout(() => setCopied(null), 1500);
    } catch (e) { setError(String(e)); }
  }, []);

  const computePair = useCallback(async () => {
    setError(null);
    setPairResult("pending");
    try {
      const parsed = JSON.parse(friendProfile) as LatencyProfile;
      if (!mine) throw new Error("Your own profile hasn't loaded yet — refresh and try again.");
      const r = await recommendPair(mine, parsed);
      setPairResult(r);
    } catch (e) {
      setError(String(e));
      setPairResult(null);
    }
  }, [friendProfile, mine]);

  const myIp = ts?.self_ip ?? null;
  const onlinePeers = (ts?.peers ?? []).filter((p) => p.online);

  return (
    <section className="rounded-md border border-abyss-border bg-abyss-panel/40 p-4">
      <h3 className="text-sm font-semibold text-abyss-fg">Real netplay</h3>
      <p className="mt-1 text-[11px] leading-relaxed text-abyss-fg-muted">
        Both players run the same emulator + game and connect over the tailnet. Built-in netplay
        is available in <b>Dolphin</b> (Tools → Start Netplay), <b>RetroArch</b> (Online tab), and
        <b> PPSSPP</b> (Adhoc). PCSX2 / RPCS3 / DuckStation / Snes9x have no netplay support.
      </p>

      {/* Your IP — big, copyable */}
      <div className="mt-4 rounded-md border border-abyss-border bg-abyss-panel-2/60 p-3">
        <p className="text-[10px] uppercase tracking-widest text-abyss-fg-dim">Your tailnet IP</p>
        <div className="mt-1 flex items-center gap-2">
          <code className="flex-1 truncate font-mono text-base text-abyss-accent abyss-text-glow">
            {myIp ?? <span className="text-abyss-fg-dim">—</span>}
          </code>
          {myIp && (
            <button
              type="button"
              onClick={() => copy("self", myIp)}
              className="h-7 shrink-0 rounded-md border border-abyss-accent/60 bg-abyss-accent/10 px-2 text-[11px] font-medium text-abyss-accent hover:bg-abyss-accent/20"
            >
              {copied === "self" ? "✓ copied" : "Copy"}
            </button>
          )}
        </div>
        <p className="mt-1 text-[10px] text-abyss-fg-dim">
          Share with your friend — paste into their emulator's netplay "Connect to" / "Server" field.
        </p>
      </div>

      {/* Peers list */}
      <div className="mt-3 rounded-md border border-abyss-border bg-abyss-panel-2/40">
        <p className="border-b border-abyss-border px-3 py-2 text-[10px] uppercase tracking-widest text-abyss-fg-dim">
          Online peers ({onlinePeers.length})
        </p>
        {onlinePeers.length === 0 ? (
          <p className="px-3 py-3 text-[11px] text-abyss-fg-dim">
            No peers online yet. Friend needs Abyss installed + signed into the same Tailscale account.
          </p>
        ) : (
          <ul className="divide-y divide-abyss-border">
            {onlinePeers.map((p) => {
              const ip = p.addrs.find((a) => !a.includes(":")) ?? p.addrs[0] ?? "";
              return (
                <li key={p.host_name} className="flex items-center gap-3 px-3 py-2">
                  <div className="min-w-0 flex-1">
                    <p className="text-xs font-medium text-abyss-fg">{p.host_name}</p>
                    <code className="font-mono text-[10px] text-abyss-fg-dim">{ip}</code>
                  </div>
                  <button
                    type="button"
                    onClick={() => copy(p.host_name, ip)}
                    className="h-7 shrink-0 rounded-md border border-abyss-border bg-abyss-panel-2 px-2 text-[11px] text-abyss-fg-muted hover:border-abyss-accent/40 hover:text-abyss-accent"
                  >
                    {copied === p.host_name ? "✓ copied" : "Copy IP"}
                  </button>
                </li>
              );
            })}
          </ul>
        )}
      </div>

      {/* Balanced-region recommender */}
      <details className="mt-3 rounded-md border border-abyss-border bg-abyss-panel-2/40">
        <summary className="cursor-pointer px-3 py-2 text-[11px] font-medium text-abyss-fg-muted hover:text-abyss-fg">
          Balanced-relay region (advanced)
        </summary>
        <div className="space-y-2 px-3 pb-3">
          <p className="text-[11px] leading-relaxed text-abyss-fg-muted">
            Useful only if peers can't connect directly and Tailscale falls back to a DERP relay.
            Both peers run <em>Network → Probe regions</em>, copy their profile here, exchange,
            then this picks the region minimising worst-case RTT — same lag for both players.
          </p>
          <div className="grid grid-cols-2 gap-2">
            <div>
              <p className="mb-1 text-[10px] uppercase tracking-widest text-abyss-fg-dim">Yours</p>
              <textarea
                readOnly
                value={mine ? JSON.stringify(mine) : ""}
                placeholder="Click Probe regions on the Network tab first."
                className="h-20 w-full resize-none rounded-md border border-abyss-border bg-abyss-panel-2 px-2 py-1 font-mono text-[10px] text-abyss-fg placeholder:text-abyss-fg-dim"
              />
              {mine && (
                <button
                  type="button"
                  onClick={() => copy("mine-profile", JSON.stringify(mine))}
                  className="mt-1 h-6 rounded-md border border-abyss-border bg-abyss-panel-2 px-2 text-[10px] text-abyss-fg-muted hover:border-abyss-accent/40 hover:text-abyss-accent"
                >
                  {copied === "mine-profile" ? "✓ copied" : "Copy"}
                </button>
              )}
            </div>
            <div>
              <p className="mb-1 text-[10px] uppercase tracking-widest text-abyss-fg-dim">Friend's</p>
              <textarea
                value={friendProfile}
                onChange={(e) => setFriend(e.target.value)}
                placeholder="Paste friend's profile JSON here."
                className="h-20 w-full resize-none rounded-md border border-abyss-border bg-abyss-panel-2 px-2 py-1 font-mono text-[10px] text-abyss-fg placeholder:text-abyss-fg-dim"
              />
            </div>
          </div>
          <button
            type="button"
            onClick={computePair}
            disabled={!friendProfile.trim() || !mine}
            className="h-7 rounded-md border border-abyss-accent/60 bg-abyss-accent/10 px-3 text-[11px] font-medium text-abyss-accent hover:bg-abyss-accent/20 disabled:opacity-50"
          >
            Recommend region
          </button>
          {pairResult && pairResult !== "pending" && (
            <p className="text-[11px] text-abyss-success">
              ✓ Best region: <b>{pairResult.label}</b> — ~{pairResult.latency_ms} ms worst-case.
            </p>
          )}
          {pairResult === null && !error && (
            <p className="text-[11px] text-abyss-warning">
              No region is reachable from both of you. Direct connection should still work over the tailnet.
            </p>
          )}
        </div>
      </details>

      {error && <p className="mt-2 text-[11px] text-abyss-danger">{error}</p>}
    </section>
  );
}
