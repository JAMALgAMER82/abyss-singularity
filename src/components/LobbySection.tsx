import { useCallback, useEffect, useMemo, useState } from "react";

import { PLATFORM_DISPLAY, loadLibrary, type LibraryEntry, type Platform } from "../lib/library";
import { tailscaleStatus, type TailscaleStatus } from "../lib/network";
import {
  lobbyCloseRoom,
  lobbyHostRoom,
  lobbyLeaveRoom,
  lobbyRequestJoin,
  lobbyStartGame,
  lobbyState,
  onLobbyIncomingInvite,
  onLobbyState,
  type LobbyIncomingInvite,
  type RoomSnapshot,
} from "../lib/lobby";

/**
 * Lobby — host or join an in-app netplay room, then auto-launch the
 * game synchronously across every member when the host hits Start.
 *
 * Currently netplay-capable via RetroArch only (every libretro core
 * supports `-H` / `--connect=` out of the box). Other emulators show
 * an explanatory error when asked to launch into a lobby session.
 */
export function LobbySection() {
  const [room,        setRoom]        = useState<RoomSnapshot | null>(null);
  const [library,     setLibrary]     = useState<LibraryEntry[]>([]);
  const [ts,          setTs]          = useState<TailscaleStatus | null>(null);
  const [picked,      setPicked]      = useState<string>("");
  const [incoming,    setIncoming]    = useState<LobbyIncomingInvite[]>([]);
  const [busy,        setBusy]        = useState(false);
  const [error,       setError]       = useState<string | null>(null);
  const [info,        setInfo]        = useState<string | null>(null);
  const [filter,      setFilter]      = useState("");

  // ---- initial load ----------------------------------------------------
  useEffect(() => {
    lobbyState().then(setRoom).catch(() => {});
    loadLibrary().then(setLibrary).catch(() => {});
    tailscaleStatus().then(setTs).catch(() => {});
    const t = setInterval(() => {
      tailscaleStatus().then(setTs).catch(() => {});
    }, 5000);
    return () => clearInterval(t);
  }, []);

  // ---- live events -----------------------------------------------------
  useEffect(() => {
    let unS: undefined | (() => void);
    let unI: undefined | (() => void);
    onLobbyState((s) => setRoom(s)).then((u) => { unS = u; });
    onLobbyIncomingInvite((inv) => {
      setIncoming((prev) => {
        // De-dup by host_addr; the newest one replaces.
        const without = prev.filter((p) => p.host_addr !== inv.host_addr);
        return [...without, inv];
      });
    }).then((u) => { unI = u; });
    return () => { unS?.(); unI?.(); };
  }, []);

  // ---- derived ---------------------------------------------------------
  // Group library by platform so the picker doesn't overwhelm.
  const grouped = useMemo(() => {
    const filt = filter.trim().toLowerCase();
    const matches = filt
      ? library.filter((e) => e.stem.toLowerCase().includes(filt))
      : library;
    const m = new Map<Platform, LibraryEntry[]>();
    for (const e of matches) {
      const arr = m.get(e.platform) ?? [];
      arr.push(e);
      m.set(e.platform, arr);
    }
    // Within each platform, sort alphabetically.
    for (const arr of m.values()) {
      arr.sort((a, b) => a.stem.localeCompare(b.stem));
    }
    return m;
  }, [library, filter]);

  const pickedEntry = useMemo(
    () => library.find((e) => e.id === picked) ?? null,
    [library, picked],
  );
  const inRoom    = room?.role !== null && room?.role !== undefined;
  const isHost    = room?.role === "host";
  const isMember  = room?.role === "member";
  const selfIp    = ts?.self_ip ?? null;

  // ---- handlers --------------------------------------------------------
  const host = useCallback(async () => {
    if (!pickedEntry) return;
    setBusy(true); setError(null); setInfo(null);
    try {
      const r = await lobbyHostRoom(pickedEntry.platform, pickedEntry.stem);
      setRoom(r);
      setInfo("Room created. Send friends an invite (above) if they aren't on your tailnet yet — your room will pop up in their Friends tab automatically.");
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }, [pickedEntry]);

  const close = useCallback(async () => {
    setBusy(true); setError(null); setInfo(null);
    try { setRoom(await lobbyCloseRoom()); }
    catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }, []);

  const leave = useCallback(async () => {
    setBusy(true); setError(null); setInfo(null);
    try { setRoom(await lobbyLeaveRoom()); }
    catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }, []);

  const start = useCallback(async () => {
    if (!selfIp) {
      setError("Your tailnet IP isn't visible yet. Make sure the mesh is signed in (Friends → Sign in).");
      return;
    }
    setBusy(true); setError(null); setInfo(null);
    try {
      const r = await lobbyStartGame(selfIp);
      setInfo(`Launched as host (run ${r.run_id.slice(0, 8)}). Members will auto-launch their copies.`);
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }, [selfIp]);

  const acceptInvite = useCallback(async (inv: LobbyIncomingInvite) => {
    setBusy(true); setError(null); setInfo(null);
    try {
      await lobbyRequestJoin(inv.host_addr);
      setIncoming((prev) => prev.filter((p) => p.host_addr !== inv.host_addr));
      setInfo(`Requested to join ${inv.host_name}'s room…`);
    } catch (e) { setError(String(e)); }
    finally { setBusy(false); }
  }, []);

  const dismissInvite = useCallback((addr: string) => {
    setIncoming((prev) => prev.filter((p) => p.host_addr !== addr));
  }, []);

  // ---- render ----------------------------------------------------------
  return (
    <section className="rounded-md border border-abyss-border bg-abyss-panel/40">
      <header className="flex items-center gap-3 border-b border-abyss-border px-4 py-3">
        <h3 className="text-base font-bold text-abyss-fg">🎮 Game lobby</h3>
        <span className="text-[12px] text-abyss-fg-muted">pick a game · friends auto-join · launches together</span>
        {inRoom && (
          <span className={`ml-auto inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] font-mono uppercase tracking-widest ${
            isHost
              ? "border-abyss-accent/40 bg-abyss-accent/10 text-abyss-accent"
              : "border-abyss-success/40 bg-abyss-success/10 text-abyss-success"
          }`}>
            {isHost ? "hosting" : "joined"}
          </span>
        )}
      </header>

      <div className="p-4">
        {/* Incoming invites — always visible at top when present */}
        {incoming.length > 0 && !isMember && (
          <ul className="mb-3 space-y-2">
            {incoming.map((inv) => (
              <li key={inv.host_addr} className="flex items-center gap-3 rounded-md border border-abyss-accent/40 bg-abyss-accent/5 px-3 py-2 text-xs">
                <span className="text-abyss-accent">●</span>
                <div className="min-w-0 flex-1">
                  <p className="truncate">
                    <span className="font-semibold text-abyss-fg">{inv.host_name}</span>
                    <span className="text-abyss-fg-muted"> is hosting </span>
                    <span className="font-mono text-abyss-accent">{inv.game_name}</span>
                    <span className="text-abyss-fg-muted"> ({PLATFORM_DISPLAY[inv.platform]})</span>
                  </p>
                  <p className="font-mono text-[10px] text-abyss-fg-dim">
                    {inv.host_addr} · {inv.members.length} member{inv.members.length !== 1 ? "s" : ""}
                  </p>
                </div>
                <button type="button" onClick={() => acceptInvite(inv)} className={smallPrimaryBtn}>Join</button>
                <button type="button" onClick={() => dismissInvite(inv.host_addr)} className={smallGhostBtn}>×</button>
              </li>
            ))}
          </ul>
        )}

        {/* In-room view */}
        {inRoom ? (
          <div className="space-y-3">
            <div className="rounded-md border border-abyss-border bg-abyss-panel-2/60 p-3">
              <p className="text-[10px] uppercase tracking-widest text-abyss-fg-dim">
                Current room
              </p>
              <p className="mt-0.5 text-sm font-semibold text-abyss-fg">
                {room?.game_name ?? "—"}{" "}
                <span className="text-[11px] font-normal text-abyss-fg-muted">
                  ({room?.platform ? PLATFORM_DISPLAY[room.platform] : "—"})
                </span>
              </p>
              <p className="mt-1 text-[11px] text-abyss-fg-muted">
                Host: <span className="font-mono">{room?.host_name ?? room?.host_addr}</span>
                {!isHost && room?.host_addr && (
                  <span className="text-abyss-fg-dim"> · {room.host_addr}</span>
                )}
              </p>
              {isHost && (
                <p className="mt-1 text-[11px] text-abyss-fg-muted">
                  Members ({room?.members.length ?? 0}):{" "}
                  {(room?.members ?? []).length === 0 ? (
                    <span className="text-abyss-fg-dim">waiting…</span>
                  ) : (
                    (room?.members ?? []).map((m, i) => (
                      <span key={m.addr} className="text-abyss-fg">
                        {i > 0 ? ", " : ""}
                        {m.display_name ?? m.addr}
                      </span>
                    ))
                  )}
                </p>
              )}
            </div>

            <div className="flex flex-wrap items-center gap-3">
              {isHost ? (
                <>
                  <button type="button" onClick={start} disabled={busy || !selfIp} className={hugeBtn}>
                    ▶ Start the game for everyone
                  </button>
                  <button type="button" onClick={close} disabled={busy} className={dangerBtn}>
                    Close room
                  </button>
                </>
              ) : (
                <button type="button" onClick={leave} disabled={busy} className={dangerBtn}>
                  Leave room
                </button>
              )}
              {!selfIp && (
                <span className="text-[12px] text-abyss-warning">
                  ⚠ no tailnet IP — sign in via Friends first
                </span>
              )}
            </div>
          </div>
        ) : (
          /* Host-a-room picker */
          <div className="space-y-3">
            <input
              type="text"
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              placeholder="Filter games…"
              className={inputCls}
            />
            {library.length === 0 ? (
              <p className="text-xs text-abyss-fg-dim">
                Your library is empty. Scan a games folder under <em>Library</em> first.
              </p>
            ) : (
              <select
                value={picked}
                onChange={(e) => setPicked(e.target.value)}
                className="h-9 w-full rounded-md border border-abyss-border bg-abyss-panel-2 px-2 text-sm text-abyss-fg focus:border-abyss-accent/60 focus:outline-none"
              >
                <option value="">— pick a game to host —</option>
                {Array.from(grouped.entries()).map(([platform, entries]) => (
                  <optgroup key={platform} label={PLATFORM_DISPLAY[platform]}>
                    {entries.map((e) => (
                      <option key={e.id} value={e.id}>{e.stem}</option>
                    ))}
                  </optgroup>
                ))}
              </select>
            )}

            <div className="flex flex-wrap items-center gap-3">
              <button
                type="button"
                onClick={host}
                disabled={busy || !pickedEntry}
                className={hugeBtn}
              >
                🎮 Host this game
              </button>
              {pickedEntry && (
                <span className="text-[12px] text-abyss-fg-muted">
                  Your friends will get a "Join Bob's game" popup. One click and we launch on every PC at once.
                </span>
              )}
            </div>

            <p className="rounded-sm border border-abyss-border bg-abyss-panel-2/40 px-3 py-2 text-[12px] text-abyss-fg-muted">
              <span className="text-abyss-accent">ℹ</span> Works with classic systems (NES, SNES, Genesis,
              GBA, N64, PS1 and more — anything that runs in RetroArch). Newer consoles like Wii or PS2
              still need their emulator's own netplay menu — see <em>Real netplay</em> below for the IPs.
            </p>
          </div>
        )}

        {error && (
          <p className="mt-3 rounded-sm border border-abyss-danger/30 bg-abyss-danger/10 px-3 py-2 text-xs text-abyss-danger">
            {error}
          </p>
        )}
        {info && !error && (
          <p className="mt-3 rounded-sm border border-abyss-success/30 bg-abyss-success/10 px-3 py-2 text-xs text-abyss-success">
            {info}
          </p>
        )}
      </div>
    </section>
  );
}

const inputCls = `
  h-10 w-full rounded-md border-2 border-abyss-border bg-abyss-panel-2 px-3
  text-sm text-abyss-fg placeholder:text-abyss-fg-dim
  focus:border-abyss-accent/60 focus:outline-none
`;
const hugeBtn = `
  h-12 rounded-lg border-2 border-abyss-accent/60 bg-abyss-accent/15 px-6
  text-base font-bold text-abyss-accent transition-all hover:bg-abyss-accent/25 hover:scale-[1.02]
  disabled:cursor-not-allowed disabled:opacity-50 disabled:hover:scale-100
`;
const dangerBtn = `
  h-10 rounded-md border-2 border-abyss-danger/40 bg-abyss-danger/10 px-3
  text-sm font-bold text-abyss-danger transition-all hover:bg-abyss-danger/20
  disabled:cursor-not-allowed disabled:opacity-50
`;
const smallPrimaryBtn = `
  h-7 rounded-md border border-abyss-accent/60 bg-abyss-accent/10 px-2
  text-[11px] font-medium text-abyss-accent hover:bg-abyss-accent/20
`;
const smallGhostBtn = `
  h-7 w-7 rounded-md border border-abyss-border bg-transparent text-[12px]
  text-abyss-fg-muted hover:border-abyss-danger/40 hover:text-abyss-danger
`;
