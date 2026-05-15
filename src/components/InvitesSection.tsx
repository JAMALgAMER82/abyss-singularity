import { useCallback, useEffect, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";

import {
  netClearRedeemedInvite,
  netCreateInvite,
  netGetConfig,
  netRedeemInvite,
  netSetInviteConfig,
  type NetworkConfig,
} from "../lib/network";

/**
 * Tailscale-backed friend invites.
 *
 * The host generates a single Tailscale pre-auth key once via their
 * admin console; Abyss wraps it into a paste-able invite code. Friends
 * paste the code, the mesh sidecar respawns authenticated against the
 * host's tailnet, and they immediately appear as a peer. Eliminates the
 * browser sign-in dance on the joining side.
 */
export function InvitesSection() {
  const [cfg,        setCfg]        = useState<NetworkConfig | null>(null);
  const [authkey,    setAuthkey]    = useState("");
  const [displayName,setDisplayName]= useState("");
  const [code,       setCode]       = useState<string | null>(null);
  const [paste,      setPaste]      = useState("");
  const [busy,       setBusy]       = useState(false);
  const [error,      setError]      = useState<string | null>(null);
  const [success,    setSuccess]    = useState<string | null>(null);
  const [copied,     setCopied]     = useState(false);

  useEffect(() => {
    netGetConfig().then((c) => {
      setCfg(c);
      setAuthkey(c.host_invite_authkey ?? "");
      setDisplayName(c.host_display_name ?? "");
    }).catch((e) => setError(String(e)));
  }, []);

  const persistAndGenerate = useCallback(async () => {
    setError(null); setSuccess(null);
    setBusy(true);
    try {
      await netSetInviteConfig(authkey, displayName);
      const next = await netGetConfig();
      setCfg(next);
      const c = await netCreateInvite();
      setCode(c);
      setSuccess("Invite code generated. Copy it and send to your friend.");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }, [authkey, displayName]);

  const copy = useCallback(async () => {
    if (!code) return;
    try {
      await navigator.clipboard.writeText(code);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch (e) { setError(String(e)); }
  }, [code]);

  const redeem = useCallback(async () => {
    setError(null); setSuccess(null);
    if (!paste.trim()) return;
    setBusy(true);
    try {
      const hostName = await netRedeemInvite(paste.trim());
      setSuccess(`Joining ${hostName}'s tailnet… mesh sidecar restarting. You'll see them appear under Peers in ~10s.`);
      setPaste("");
      setCfg(await netGetConfig());
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }, [paste]);

  const clearRedeemed = useCallback(async () => {
    setError(null); setSuccess(null);
    setBusy(true);
    try {
      await netClearRedeemedInvite();
      setCfg(await netGetConfig());
      setSuccess("Cleared. Mesh sidecar will respawn on its own tailnet — sign in via Friends if needed.");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }, []);

  return (
    <details className="rounded-md border border-abyss-border bg-abyss-panel/40">
      <summary className="cursor-pointer px-4 py-3 text-base font-semibold text-abyss-fg hover:bg-abyss-panel-2/40 list-none [&::-webkit-details-marker]:hidden">
        <span className="inline-flex items-center gap-3">
          <span className="inline-block transition-transform">▸</span>
          📨 Invite codes — set up tailnet sharing
        </span>
        {cfg?.redeemed_from && (
          <span className="ml-3 inline-flex items-center gap-2">
            <span className="inline-flex items-center gap-1 rounded-full border border-abyss-success/40 bg-abyss-success/10 px-2 py-0.5 text-[11px] font-mono uppercase tracking-widest text-abyss-success">
              ✓ joined {cfg.redeemed_from}
            </span>
            <button
              type="button"
              disabled={busy}
              onClick={(e) => {
                // Prevent the click from toggling the <details> open/closed.
                e.preventDefault();
                e.stopPropagation();
                clearRedeemed();
              }}
              title={`Disconnect from ${cfg.redeemed_from}'s tailnet — friends list will be replaced with peers from your own account. You can paste a different invite code afterwards.`}
              className="h-7 rounded-md border border-abyss-border bg-abyss-panel-2 px-3 text-[11px] font-medium text-abyss-fg-muted transition-colors hover:border-abyss-danger/40 hover:text-abyss-danger disabled:opacity-50"
            >
              ✕ Leave
            </button>
            <span className="text-[11px] text-abyss-fg-dim">
              (or paste another code below to switch)
            </span>
          </span>
        )}
      </summary>

      <div className="grid grid-cols-1 gap-5 border-t border-abyss-border p-5 md:grid-cols-2">

        {/* =============== HOST SIDE — create invite ====================== */}
        <section>
          <h4 className="text-sm font-bold text-abyss-fg">🏠 Hosting? Make an invite.</h4>
          <p className="mt-1 text-[12px] text-abyss-fg-muted">
            One-time setup. Friends use the code to join your network.
          </p>

          <ol className="mt-3 space-y-3 text-sm">
            <li className="flex items-start gap-2">
              <span className="mt-0.5 inline-flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-abyss-accent/20 text-[11px] font-bold text-abyss-accent">1</span>
              <div className="flex-1">
                <button
                  type="button"
                  onClick={() => openUrl("https://login.tailscale.com/admin/settings/keys").catch(() => {})}
                  className="h-9 rounded-md border-2 border-abyss-accent/60 bg-abyss-accent/10 px-3 text-sm font-semibold text-abyss-accent hover:bg-abyss-accent/20"
                >
                  Open Tailscale ↗
                </button>
                <p className="mt-1 text-[11px] text-abyss-fg-muted">
                  Click "Generate auth key" → check Reusable + Pre-approved → copy the key.
                </p>
              </div>
            </li>
            <li className="flex items-start gap-2">
              <span className="mt-0.5 inline-flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-abyss-accent/20 text-[11px] font-bold text-abyss-accent">2</span>
              <div className="flex-1 space-y-2">
                <input
                  type="text"
                  spellCheck={false}
                  value={authkey}
                  onChange={(e) => setAuthkey(e.target.value)}
                  placeholder="Paste the tskey-auth-... here"
                  className={inputCls}
                />
                <input
                  type="text"
                  value={displayName}
                  onChange={(e) => setDisplayName(e.target.value)}
                  placeholder="Your name (shown to friends)"
                  className={inputCls}
                />
              </div>
            </li>
            <li className="flex items-start gap-2">
              <span className="mt-0.5 inline-flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-abyss-accent/20 text-[11px] font-bold text-abyss-accent">3</span>
              <button
                type="button"
                disabled={busy || !authkey.trim() || !displayName.trim()}
                onClick={persistAndGenerate}
                className={primaryBtn}
              >
                {busy ? "Working…" : "Generate invite code"}
              </button>
            </li>
          </ol>

          {code && (
            <div className="mt-3 rounded-md border border-abyss-accent/40 bg-abyss-accent/5 p-2">
              <p className="text-[10px] font-mono uppercase tracking-widest text-abyss-accent">
                Invite code (send this to your friend)
              </p>
              <code className="mt-1 block break-all font-mono text-[11px] text-abyss-fg">
                {code}
              </code>
              <button
                type="button"
                onClick={copy}
                className="mt-2 h-7 rounded-md border border-abyss-accent/60 bg-abyss-accent/10 px-3 text-[11px] font-medium text-abyss-accent hover:bg-abyss-accent/20"
              >
                {copied ? "✓ copied" : "Copy"}
              </button>
            </div>
          )}
        </section>

        {/* =============== FRIEND SIDE — redeem invite ===================== */}
        <section>
          <h4 className="text-sm font-bold text-abyss-fg">👋 Friend sent you a code? Paste it.</h4>
          <p className="mt-1 text-[12px] text-abyss-fg-muted">
            One paste. We handle everything else.
          </p>

          <div className="mt-3 space-y-2">
            <textarea
              spellCheck={false}
              value={paste}
              onChange={(e) => setPaste(e.target.value)}
              placeholder="Paste the invite code from your friend"
              className="h-24 w-full resize-none rounded-md border-2 border-abyss-border bg-abyss-panel-2 px-3 py-2 font-mono text-[12px] text-abyss-fg placeholder:text-abyss-fg-dim focus:border-abyss-accent/60 focus:outline-none"
            />
            <div className="flex flex-wrap items-center gap-2">
              <button
                type="button"
                disabled={busy || !paste.trim()}
                onClick={redeem}
                className={primaryBtn}
              >
                {busy ? "Joining…" : "▶ Join my friend"}
              </button>
              {cfg?.redeemed_authkey && (
                <button
                  type="button"
                  onClick={clearRedeemed}
                  disabled={busy}
                  className="h-9 rounded-md border border-abyss-border bg-abyss-panel-2 px-3 text-sm font-medium text-abyss-fg-muted transition-colors hover:border-abyss-danger/40 hover:text-abyss-danger"
                >
                  Leave their network
                </button>
              )}
            </div>
          </div>
        </section>

        {(error || success) && (
          <div className="md:col-span-2">
            {error && (
              <p className="rounded-sm border border-abyss-danger/30 bg-abyss-danger/10 px-3 py-2 text-xs text-abyss-danger">{error}</p>
            )}
            {success && !error && (
              <p className="rounded-sm border border-abyss-success/30 bg-abyss-success/10 px-3 py-2 text-xs text-abyss-success">{success}</p>
            )}
          </div>
        )}
      </div>
    </details>
  );
}

const inputCls = `
  h-10 w-full rounded-md border-2 border-abyss-border bg-abyss-panel-2 px-3
  font-mono text-sm text-abyss-fg placeholder:text-abyss-fg-dim
  focus:border-abyss-accent/60 focus:outline-none
`;

const primaryBtn = `
  h-10 rounded-md border-2 border-abyss-accent/60 bg-abyss-accent/15 px-5
  text-sm font-bold text-abyss-accent transition-all hover:bg-abyss-accent/25 hover:scale-[1.02]
  disabled:cursor-not-allowed disabled:opacity-50 disabled:hover:scale-100
`;
