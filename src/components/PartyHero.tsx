import { useEffect, useState } from "react";
import { tailscaleStatus, type TailscaleStatus } from "../lib/network";
import {
  lobbyState,
  onLobbyIncomingInvite,
  onLobbyState,
  type LobbyIncomingInvite,
  type RoomSnapshot,
} from "../lib/lobby";

/**
 * Big, colorful "Play with friends" hero — the first thing on the
 * Friends tab. Designed for non-technical users: three states with
 * BIG buttons, minimal text, no jargon.
 *
 *   - Default: two side-by-side buttons ("Invite a friend" /
 *     "Host a game") + an at-a-glance status line.
 *   - Incoming invite: takes over with a giant prompt
 *     ("Bob is hosting Mario Kart 64 — Join?") + accept button.
 *   - In a room: shows current room state + Start button (host) or
 *     waiting indicator (member).
 *
 * Clicks scroll to the relevant existing panel below — we don't
 * duplicate the form, we just make discovery a 1-step gesture.
 */
export function PartyHero({
  onInviteFriend,
  onHostGame,
}: {
  onInviteFriend: () => void;
  onHostGame:     () => void;
}) {
  const [ts,         setTs]         = useState<TailscaleStatus | null>(null);
  const [room,       setRoom]       = useState<RoomSnapshot | null>(null);
  const [incoming,   setIncoming]   = useState<LobbyIncomingInvite | null>(null);

  useEffect(() => {
    tailscaleStatus().then(setTs).catch(() => {});
    lobbyState().then(setRoom).catch(() => {});
    const t = setInterval(() => {
      tailscaleStatus().then(setTs).catch(() => {});
    }, 5000);
    return () => clearInterval(t);
  }, []);

  useEffect(() => {
    let unS: undefined | (() => void);
    let unI: undefined | (() => void);
    onLobbyState((s) => setRoom(s)).then((u) => { unS = u; });
    onLobbyIncomingInvite((inv) => setIncoming(inv)).then((u) => { unI = u; });
    return () => { unS?.(); unI?.(); };
  }, []);

  const onlinePeers = (ts?.peers ?? []).filter((p) => p.online).length;
  const inRoom      = room?.role !== null && room?.role !== undefined;

  // ---------------- Incoming invite — highest priority -------------------
  if (incoming && !inRoom) {
    return (
      <section className="rounded-xl border-2 border-abyss-accent/60 bg-gradient-to-br from-abyss-accent/15 via-abyss-panel/80 to-abyss-panel/40 p-6 shadow-xl">
        <p className="text-[11px] font-mono uppercase tracking-widest text-abyss-accent">
          ● Friend invitation
        </p>
        <h2 className="mt-2 text-2xl font-bold text-abyss-fg abyss-text-glow">
          {incoming.host_name} wants you to play
        </h2>
        <p className="mt-1 text-base text-abyss-fg-muted">
          🎮 <span className="text-abyss-accent font-semibold">{incoming.game_name}</span>
        </p>
        <button
          type="button"
          onClick={onHostGame}
          className="mt-4 h-12 rounded-lg border-2 border-abyss-success/60 bg-abyss-success/15 px-8 text-base font-bold text-abyss-success transition-all hover:bg-abyss-success/25 hover:scale-[1.02]"
        >
          ▶ Join the game
        </button>
        <p className="mt-3 text-[12px] text-abyss-fg-dim">
          When you click Join, Abyss will pair with {incoming.host_name}'s PC and launch the game on yours automatically.
        </p>
      </section>
    );
  }

  // ---------------- In a room ------------------------------------------------
  if (inRoom) {
    const isHost = room?.role === "host";
    return (
      <section className="rounded-xl border-2 border-abyss-success/40 bg-gradient-to-br from-abyss-success/10 via-abyss-panel/80 to-abyss-panel/40 p-6 shadow-xl">
        <p className="text-[11px] font-mono uppercase tracking-widest text-abyss-success">
          ● {isHost ? "You're hosting" : "Joined " + (room?.host_name ?? room?.host_addr)}
        </p>
        <h2 className="mt-2 text-2xl font-bold text-abyss-fg">
          🎮 {room?.game_name}
        </h2>
        {isHost && (
          <p className="mt-1 text-sm text-abyss-fg-muted">
            {(room?.members.length ?? 0) === 0
              ? "Waiting for friends to join…"
              : `${room?.members.length} friend${room?.members.length === 1 ? "" : "s"} ready.`}
          </p>
        )}
        <button
          type="button"
          onClick={onHostGame}
          className="mt-4 h-12 rounded-lg border-2 border-abyss-accent/60 bg-abyss-accent/15 px-8 text-base font-bold text-abyss-accent transition-all hover:bg-abyss-accent/25 hover:scale-[1.02]"
        >
          {isHost ? "▶ Start the game for everyone" : "Open lobby ↓"}
        </button>
      </section>
    );
  }

  // ---------------- Default — two big action buttons -----------------------
  return (
    <section className="rounded-xl border-2 border-abyss-border bg-gradient-to-br from-abyss-panel-2/60 via-abyss-panel/60 to-abyss-panel/40 p-6 shadow-xl">
      <div className="flex items-start justify-between gap-4">
        <div>
          <h2 className="text-2xl font-bold text-abyss-fg abyss-text-glow">
            Play with your friends
          </h2>
          <p className="mt-1 text-sm text-abyss-fg-muted">
            One-click invite + co-op — no IPs, no PINs, no browser dance.
          </p>
        </div>
        <span className="shrink-0 rounded-full border border-abyss-border bg-abyss-panel-2 px-3 py-1 text-[11px] font-mono uppercase tracking-widest text-abyss-fg-muted">
          {onlinePeers > 0
            ? <><span className="text-abyss-success">● {onlinePeers}</span> online</>
            : <span className="text-abyss-fg-dim">no friends online</span>}
        </span>
      </div>

      <div className="mt-5 grid grid-cols-1 gap-3 md:grid-cols-2">
        <button
          type="button"
          onClick={onInviteFriend}
          className="group flex flex-col items-start gap-1 rounded-lg border-2 border-abyss-accent/40 bg-abyss-accent/5 p-5 text-left transition-all hover:border-abyss-accent/80 hover:bg-abyss-accent/10 hover:scale-[1.02]"
        >
          <span className="text-2xl">📨</span>
          <span className="text-base font-bold text-abyss-fg">Invite a friend</span>
          <span className="text-[12px] text-abyss-fg-muted leading-relaxed">
            Give them a code. They paste it once and you're on the same network forever.
          </span>
        </button>
        <button
          type="button"
          onClick={onHostGame}
          className="group flex flex-col items-start gap-1 rounded-lg border-2 border-abyss-success/40 bg-abyss-success/5 p-5 text-left transition-all hover:border-abyss-success/80 hover:bg-abyss-success/10 hover:scale-[1.02]"
        >
          <span className="text-2xl">🎮</span>
          <span className="text-base font-bold text-abyss-fg">Host a game</span>
          <span className="text-[12px] text-abyss-fg-muted leading-relaxed">
            Pick a game, they get a one-click "Join" prompt. Both launch together.
          </span>
        </button>
      </div>
    </section>
  );
}
