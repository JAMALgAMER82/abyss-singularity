import { useCallback, useEffect, useMemo, useState } from "react";

import {
  dirAcceptRequest,
  dirFriendRequests,
  dirFriends,
  dirGetConfig,
  dirGetDms,
  dirGetGlobalChat,
  dirOnline,
  dirRejectRequest,
  dirSendDm,
  dirSendFriendRequest,
  dirSendGlobalChat,
  dirSetConfig,
  onDirDm,
  onDirFriendRequest,
  onDirFriendResponse,
  type DirectMessage,
  type DirectoryConfig,
  type Friend,
  type FriendRequest,
  type GlobalChatMessage,
  type OnlineUser,
} from "../lib/directory";
import {
  netCreateInvite,
  netRedeemInvite,
} from "../lib/network";

/**
 * Discover — GameRanger-style global lobby.
 *
 * Three columns when the directory is configured:
 *   1. Online users (with "Add friend" button)
 *   2. Friend list + DM thread for the selected friend
 *   3. Global chat
 *
 * When the directory ISN'T configured, shows a single onboarding card:
 * "Set up the directory" → opens Settings.
 *
 * Friend-request acceptance can optionally include a mesh invite code
 * (auto-generated from the user's tailnet) so that on accept, both sides
 * also become mesh peers. That's the "GameRanger meets Tailscale" join.
 */
export function DiscoverView() {
  const [cfg,          setCfg]          = useState<DirectoryConfig | null>(null);
  const [online,       setOnline]       = useState<OnlineUser[]>([]);
  const [requests,     setRequests]     = useState<FriendRequest[]>([]);
  const [friends,      setFriends]      = useState<Friend[]>([]);
  const [dms,          setDms]          = useState<DirectMessage[]>([]);
  const [globalChat,   setGlobalChat]   = useState<GlobalChatMessage[]>([]);
  const [selected,     setSelected]     = useState<string | null>(null);
  const [draft,        setDraft]        = useState("");
  const [globalDraft,  setGlobalDraft]  = useState("");
  const [filter,       setFilter]       = useState("");
  const [error,        setError]        = useState<string | null>(null);
  const [info,         setInfo]         = useState<string | null>(null);

  const ready = Boolean(cfg?.worker_url && cfg?.user_id && cfg?.handle);

  // ---- initial config load (cheap, always runs) -------------------------
  useEffect(() => {
    dirGetConfig().then(setCfg).catch((e) => setError(String(e)));
  }, []);

  // ---- once ready, poll the directory -----------------------------------
  useEffect(() => {
    if (!ready) return;
    const refresh = () => {
      dirOnline().then(setOnline).catch(() => {});
      dirFriendRequests().then(setRequests).catch(() => {});
      dirFriends().then(setFriends).catch(() => {});
      dirGetDms().then(setDms).catch(() => {});
      dirGetGlobalChat().then(setGlobalChat).catch(() => {});
    };
    refresh();
    const t = setInterval(refresh, 15_000);
    return () => clearInterval(t);
  }, [ready]);

  // ---- live events -------------------------------------------------------
  useEffect(() => {
    if (!ready) return;
    let unA: undefined | (() => void);
    let unB: undefined | (() => void);
    let unC: undefined | (() => void);
    onDirFriendRequest((rs) => {
      setRequests((prev) => {
        // Merge by id, newer wins.
        const map = new Map(prev.map((r) => [r.id, r]));
        for (const r of rs) map.set(r.id, r);
        return Array.from(map.values());
      });
    }).then((u) => { unA = u; });
    onDirFriendResponse(async () => {
      // A response landed — re-pull friends so the new pair appears.
      dirFriends().then(setFriends).catch(() => {});
    }).then((u) => { unB = u; });
    onDirDm((ms) => {
      setDms((prev) => {
        const map = new Map(prev.map((m) => [m.id, m]));
        for (const m of ms) map.set(m.id, m);
        return Array.from(map.values()).sort((a, b) => a.sent_at - b.sent_at);
      });
    }).then((u) => { unC = u; });
    return () => { unA?.(); unB?.(); unC?.(); };
  }, [ready]);

  // ---- derived -----------------------------------------------------------
  const me = cfg?.user_id ?? "";
  const friendIds = useMemo(() => new Set(friends.map((f) => f.id)), [friends]);
  const filtered  = useMemo(() => {
    const q = filter.trim().toLowerCase();
    return online
      .filter((u) => u.id !== me)
      .filter((u) => !q || u.handle.toLowerCase().includes(q))
      .sort((a, b) => b.last_seen - a.last_seen);
  }, [online, filter, me]);

  const conversation = useMemo(() => {
    if (!selected) return [];
    return dms.filter((d) => d.from_id === selected || d.to_id === selected)
              .sort((a, b) => a.sent_at - b.sent_at);
  }, [dms, selected]);

  // ---- handlers ----------------------------------------------------------
  const sendFriendRequest = useCallback(async (userId: string, includeMeshInvite: boolean) => {
    setError(null); setInfo(null);
    try {
      let invite: string | undefined = undefined;
      if (includeMeshInvite) {
        // Pull a fresh invite code from the existing mesh-invite system.
        // Falls through silently if the user hasn't set up an invite key yet.
        try { invite = await netCreateInvite(); }
        catch (e) {
          setInfo("Sent without mesh invite. Set up Tailscale invite in Friends → Invite codes to also enable peer-to-peer play.");
        }
      }
      await dirSendFriendRequest(userId, invite, undefined);
      setInfo("Friend request sent.");
    } catch (e) { setError(String(e)); }
  }, []);

  const accept = useCallback(async (req: FriendRequest, alsoMeshPair: boolean) => {
    setError(null); setInfo(null);
    try {
      // If the requester offered an invite code AND we want mesh pairing,
      // redeem it now so we end up on their tailnet. Send our own back
      // in the same accept so they can join ours too (if they choose).
      if (alsoMeshPair && req.invite_code) {
        await netRedeemInvite(req.invite_code).catch(() => {});
      }
      let myInvite: string | undefined = undefined;
      if (alsoMeshPair) {
        try { myInvite = await netCreateInvite(); } catch { /* not configured yet */ }
      }
      await dirAcceptRequest(req.id, myInvite);
      setRequests((prev) => prev.filter((r) => r.id !== req.id));
      setInfo(`Accepted ${req.from_handle}'s friend request.`);
      dirFriends().then(setFriends).catch(() => {});
    } catch (e) { setError(String(e)); }
  }, []);

  const reject = useCallback(async (req: FriendRequest) => {
    setError(null); setInfo(null);
    try {
      await dirRejectRequest(req.id);
      setRequests((prev) => prev.filter((r) => r.id !== req.id));
    } catch (e) { setError(String(e)); }
  }, []);

  const sendDm = useCallback(async () => {
    if (!selected || !draft.trim()) return;
    setError(null);
    try {
      await dirSendDm(selected, draft.trim());
      setDraft("");
      dirGetDms().then(setDms).catch(() => {});
    } catch (e) { setError(String(e)); }
  }, [selected, draft]);

  const sendGlobal = useCallback(async () => {
    if (!globalDraft.trim()) return;
    setError(null);
    try {
      await dirSendGlobalChat(globalDraft.trim());
      setGlobalDraft("");
      dirGetGlobalChat().then(setGlobalChat).catch(() => {});
    } catch (e) { setError(String(e)); }
  }, [globalDraft]);

  const toggleHidden = useCallback(async () => {
    if (!cfg) return;
    setError(null);
    try {
      const next = await dirSetConfig({ hidden: !cfg.hidden });
      setCfg(next);
    } catch (e) { setError(String(e)); }
  }, [cfg]);

  // ---- render ------------------------------------------------------------
  if (!ready) {
    return (
      <div className="flex h-full flex-col overflow-auto p-8">
        <header className="border-b border-abyss-border pb-4">
          <h2 className="text-2xl font-bold text-abyss-fg abyss-text-glow">Discover</h2>
          <p className="mt-1 text-sm text-abyss-fg-muted">
            See everyone using Abyss online, add friends, chat with strangers — GameRanger style.
          </p>
        </header>

        <section className="mt-6 rounded-xl border-2 border-abyss-accent/40 bg-abyss-accent/5 p-6">
          <h3 className="text-lg font-bold text-abyss-fg">One-time setup needed</h3>
          <p className="mt-2 text-sm leading-relaxed text-abyss-fg-muted">
            The Discover feature talks to a tiny Cloudflare Worker that you deploy once (free).
            The Worker is the only piece of infrastructure Abyss needs — everything else stays
            peer-to-peer over Tailscale.
          </p>
          <ol className="mt-4 list-decimal space-y-2 pl-5 text-sm text-abyss-fg-muted">
            <li>Open the <code className="text-abyss-accent">abyss-directory/</code> folder in this repo, follow the README.md to deploy the Worker (~5 minutes, free Cloudflare account).</li>
            <li>Go to <em>Settings → Directory</em> and paste your Worker URL (e.g. <code className="text-abyss-accent">https://abyss-directory.you.workers.dev</code>).</li>
            <li>Pick a display name. That's it — return here and you'll see who's online.</li>
          </ol>
        </section>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col overflow-hidden">
      <header className="flex shrink-0 items-center gap-3 border-b border-abyss-border px-6 py-4">
        <h2 className="text-lg font-semibold text-abyss-fg abyss-text-glow">Discover</h2>
        <span className="text-xs text-abyss-fg-dim">
          everyone on Abyss · {online.length} online
        </span>
        <div className="ml-auto flex items-center gap-2">
          <span className="font-mono text-[11px] text-abyss-fg-muted">
            you: <span className="text-abyss-accent">{cfg?.handle}</span>
          </span>
          <button
            type="button"
            onClick={toggleHidden}
            className={`h-8 rounded-md border px-3 text-xs font-medium transition-colors ${
              cfg?.hidden
                ? "border-abyss-warning/40 bg-abyss-warning/10 text-abyss-warning hover:bg-abyss-warning/20"
                : "border-abyss-border bg-abyss-panel-2 text-abyss-fg-muted hover:border-abyss-accent/40 hover:text-abyss-accent"
            }`}
          >
            {cfg?.hidden ? "● Appearing offline" : "● Online"}
          </button>
        </div>
      </header>

      {/* Friend-request inbox banner — surfaces above the columns. */}
      {requests.length > 0 && (
        <div className="border-b border-abyss-accent/30 bg-abyss-accent/5 px-6 py-3">
          <p className="text-[11px] font-mono uppercase tracking-widest text-abyss-accent">
            ↘ Friend requests ({requests.length})
          </p>
          <ul className="mt-2 space-y-2">
            {requests.map((r) => (
              <li key={r.id} className="flex items-center gap-3 rounded-md border border-abyss-accent/30 bg-abyss-panel/60 px-3 py-2 text-xs">
                <div className="flex-1 min-w-0">
                  <p className="truncate font-semibold text-abyss-fg">
                    {r.from_handle}
                    {r.invite_code && (
                      <span className="ml-2 inline-flex items-center gap-1 rounded-full border border-abyss-accent/40 bg-abyss-accent/10 px-2 py-0.5 text-[10px] font-mono uppercase tracking-widest text-abyss-accent">
                        + mesh invite
                      </span>
                    )}
                  </p>
                  {r.message && <p className="truncate text-abyss-fg-muted">{r.message}</p>}
                </div>
                <button type="button" onClick={() => accept(r, Boolean(r.invite_code))} className={smallPrimaryBtn}>
                  ✓ Accept{r.invite_code ? " + mesh" : ""}
                </button>
                <button type="button" onClick={() => reject(r)} className={smallGhostBtn}>✕</button>
              </li>
            ))}
          </ul>
        </div>
      )}

      <div className="grid flex-1 grid-cols-1 gap-0 overflow-hidden lg:grid-cols-[320px_1fr_300px]">
        {/* ============================== ONLINE LIST ========================== */}
        <aside className="overflow-auto border-r border-abyss-border bg-abyss-panel/40">
          <div className="border-b border-abyss-border bg-abyss-panel-2/40 px-3 py-2">
            <input
              type="text"
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              placeholder="Filter by name…"
              className="h-8 w-full rounded-md border border-abyss-border bg-abyss-panel-2 px-2 text-xs text-abyss-fg placeholder:text-abyss-fg-dim focus:border-abyss-accent/60 focus:outline-none"
            />
          </div>
          {filtered.length === 0 ? (
            <p className="p-4 text-xs text-abyss-fg-dim">No one matches that filter.</p>
          ) : (
            <ul className="divide-y divide-abyss-border">
              {filtered.map((u) => {
                const isFriend = friendIds.has(u.id);
                return (
                  <li key={u.id} className="flex items-center gap-3 px-3 py-2">
                    <span className="h-2 w-2 rounded-full bg-abyss-success" />
                    <div className="flex-1 min-w-0">
                      <p className="truncate text-sm font-medium text-abyss-fg">
                        {u.handle}
                        {isFriend && (
                          <span className="ml-2 text-[10px] font-mono uppercase tracking-widest text-abyss-success">friend</span>
                        )}
                      </p>
                      <p className="truncate font-mono text-[10px] text-abyss-fg-dim">
                        {u.country ?? "—"} · v{u.app_version}
                      </p>
                    </div>
                    {isFriend ? (
                      <button type="button" onClick={() => setSelected(u.id)} className={smallGhostBtn}>
                        chat
                      </button>
                    ) : (
                      <button type="button" onClick={() => sendFriendRequest(u.id, true)} className={smallPrimaryBtn}>
                        + add
                      </button>
                    )}
                  </li>
                );
              })}
            </ul>
          )}
        </aside>

        {/* ============================== DM PANEL ============================ */}
        <main className="flex flex-1 flex-col overflow-hidden">
          {selected ? (
            <>
              <header className="flex shrink-0 items-center gap-3 border-b border-abyss-border bg-abyss-panel/60 px-4 py-2">
                <p className="text-sm font-semibold text-abyss-fg">
                  {friends.find((f) => f.id === selected)?.handle
                    ?? online.find((u) => u.id === selected)?.handle
                    ?? selected.slice(0, 8)}
                </p>
              </header>
              <div className="flex-1 overflow-auto p-4">
                {conversation.length === 0 ? (
                  <p className="text-center text-xs text-abyss-fg-dim">No messages yet. Say hi.</p>
                ) : (
                  <ul className="flex flex-col gap-1.5">
                    {conversation.map((m) => (
                      <li
                        key={m.id}
                        className={[
                          "max-w-[78%] rounded-md px-3 py-1.5 text-sm",
                          m.from_id === me
                            ? "self-end bg-abyss-accent/15 text-abyss-fg"
                            : "self-start bg-abyss-panel-2 text-abyss-fg",
                        ].join(" ")}
                      >
                        <span className="whitespace-pre-wrap break-words">{m.body}</span>
                      </li>
                    ))}
                  </ul>
                )}
              </div>
              <footer className="border-t border-abyss-border bg-abyss-panel/60 px-3 py-2">
                <form onSubmit={(e) => { e.preventDefault(); sendDm(); }} className="flex gap-2">
                  <input
                    type="text"
                    value={draft}
                    onChange={(e) => setDraft(e.target.value)}
                    placeholder="Message…"
                    className="h-9 flex-1 rounded-md border border-abyss-border bg-abyss-panel-2 px-3 text-sm text-abyss-fg placeholder:text-abyss-fg-dim focus:border-abyss-accent/60 focus:outline-none"
                  />
                  <button type="submit" disabled={!draft.trim()} className={primaryBtn}>Send</button>
                </form>
              </footer>
            </>
          ) : (
            <div className="flex h-full items-center justify-center p-10">
              <p className="max-w-md text-center text-xs text-abyss-fg-muted">
                Pick someone in the left column to chat. Add them as a friend to keep them in your friends list once they go offline.
              </p>
            </div>
          )}
        </main>

        {/* ============================== GLOBAL CHAT ========================== */}
        <aside className="flex flex-col overflow-hidden border-l border-abyss-border bg-abyss-panel/40">
          <header className="border-b border-abyss-border bg-abyss-panel-2/40 px-3 py-2">
            <p className="text-xs font-bold text-abyss-fg">🌐 Global chat</p>
            <p className="text-[10px] text-abyss-fg-dim">visible to every Abyss user</p>
          </header>
          <div className="flex-1 overflow-auto p-3">
            {globalChat.length === 0 ? (
              <p className="text-center text-xs text-abyss-fg-dim">No messages yet.</p>
            ) : (
              <ul className="space-y-1.5">
                {globalChat.map((m) => (
                  <li key={m.id} className="text-xs">
                    <span className={m.user_id === me ? "font-bold text-abyss-accent" : "font-bold text-abyss-fg"}>
                      {m.handle}:
                    </span>{" "}
                    <span className="text-abyss-fg-muted">{m.body}</span>
                  </li>
                ))}
              </ul>
            )}
          </div>
          <footer className="border-t border-abyss-border bg-abyss-panel-2/40 px-3 py-2">
            <form onSubmit={(e) => { e.preventDefault(); sendGlobal(); }} className="flex gap-2">
              <input
                type="text"
                value={globalDraft}
                onChange={(e) => setGlobalDraft(e.target.value)}
                placeholder="Say something to everyone…"
                className="h-8 flex-1 rounded-md border border-abyss-border bg-abyss-panel-2 px-2 text-xs text-abyss-fg placeholder:text-abyss-fg-dim focus:border-abyss-accent/60 focus:outline-none"
              />
              <button type="submit" disabled={!globalDraft.trim()} className={smallPrimaryBtn}>Send</button>
            </form>
          </footer>
        </aside>
      </div>

      {(error || info) && (
        <div className="border-t border-abyss-border px-6 py-2">
          {error && <p className="text-xs text-abyss-danger">{error}</p>}
          {!error && info && <p className="text-xs text-abyss-success">{info}</p>}
        </div>
      )}
    </div>
  );
}

const primaryBtn = `
  h-9 rounded-md border-2 border-abyss-accent/60 bg-abyss-accent/15 px-4
  text-sm font-bold text-abyss-accent transition-all hover:bg-abyss-accent/25
  disabled:cursor-not-allowed disabled:opacity-50
`;
const smallPrimaryBtn = `
  h-7 rounded-md border border-abyss-accent/60 bg-abyss-accent/10 px-2
  text-[11px] font-medium text-abyss-accent hover:bg-abyss-accent/20
`;
const smallGhostBtn = `
  h-7 rounded-md border border-abyss-border bg-abyss-panel-2 px-2
  text-[11px] text-abyss-fg-muted hover:border-abyss-accent/40 hover:text-abyss-accent
`;
