import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";

/**
 * Quick-reference user guide embedded directly in Settings.
 *
 * Mirrors the most-needed bits of USER_MANUAL.md (in the repo root) so
 * users don't have to leave the app for "how do I add a friend?". The
 * "Open full manual on GitHub ↗" button at the top is the canonical
 * source — keep this component in sync when the manual changes.
 */
export function HelpSection() {
  const [openId, setOpenId] = useState<string | null>("multiplayer");

  return (
    <section className="rounded-md border border-abyss-border bg-abyss-panel/40 p-5">
      <header className="mb-4 flex items-start justify-between gap-3">
        <div>
          <h3 className="text-base font-semibold text-abyss-fg">📖 User guide</h3>
          <p className="mt-1 text-xs leading-relaxed text-abyss-fg-muted">
            Quick reference. For the full manual (BIOS legalities, full troubleshooting,
            file paths, privacy), open the GitHub page.
          </p>
        </div>
        <button
          type="button"
          onClick={() =>
            openUrl("https://github.com/JAMALgAMER82/abyss-singularity/blob/main/USER_MANUAL.md").catch(() => {})
          }
          className="h-9 shrink-0 rounded-md border border-abyss-accent/60 bg-abyss-accent/10 px-4 text-sm font-medium text-abyss-accent hover:bg-abyss-accent/20"
        >
          Open full manual ↗
        </button>
      </header>

      <div className="space-y-2">
        {TOPICS.map((t) => (
          <details
            key={t.id}
            open={openId === t.id}
            onClick={(e) => {
              // React controlled <details> needs preventDefault + manual toggle.
              e.preventDefault();
              setOpenId((cur) => (cur === t.id ? null : t.id));
            }}
            className="rounded-md border border-abyss-border bg-abyss-panel-2/40"
          >
            <summary className="cursor-pointer list-none px-3 py-2 text-sm font-semibold text-abyss-fg select-none [&::-webkit-details-marker]:hidden">
              <span className="mr-2 inline-block text-abyss-accent">{openId === t.id ? "▾" : "▸"}</span>
              {t.icon} {t.title}
            </summary>
            <div className="border-t border-abyss-border bg-abyss-panel/40 px-4 py-3 text-[12px] leading-relaxed text-abyss-fg-muted space-y-2">
              {t.body}
            </div>
          </details>
        ))}
      </div>
    </section>
  );
}

interface Topic { id: string; icon: string; title: string; body: React.ReactNode; }

const TOPICS: Topic[] = [
  {
    id: "multiplayer",
    icon: "🎮",
    title: "How do I play with a friend?",
    body: (
      <>
        <p><b>The 60-second version:</b></p>
        <ol className="list-decimal pl-5 space-y-1">
          <li>Make sure you're both running Abyss and have the same game ROM.</li>
          <li>On the <em>Friends</em> tab → click <em>📨 Invite a friend</em>, follow the 3 numbered steps, send the code to your friend.</li>
          <li>They paste the code into <em>"Friend sent you a code?"</em> on their Friends tab.</li>
          <li>Wait ~10 s for them to appear in your peer list. Click <em>link</em> next to their row.</li>
          <li>In the Lobby panel: pick the game → <em>🎮 Host this game</em>.</li>
          <li>They get a popup banner → they click <em>▶ Join the game</em>.</li>
          <li>You click <em>▶ Start the game for everyone</em>. Both PCs launch RetroArch and the game starts.</li>
        </ol>
        <p className="text-abyss-fg-dim">
          ⚠ Works for retro games via RetroArch (NES, SNES, Genesis, GBA, N64, PS1, PSP, etc).
          Dolphin / PCSX2 / RPCS3 need their own netplay menu — use the IP shown in Friends → Real netplay.
        </p>
      </>
    ),
  },
  {
    id: "discover",
    icon: "🛰️",
    title: "What's the Discover tab?",
    body: (
      <>
        <p>Global directory of <b>everyone running Abyss right now</b> — like GameRanger.</p>
        <ul className="list-disc pl-5 space-y-1">
          <li><b>Left column</b>: online users. Click <em>+ add</em> to send a friend request.</li>
          <li><b>Middle column</b>: DM thread with whoever you pick.</li>
          <li><b>Right column</b>: global chat — visible to every Abyss user, rate-limited.</li>
        </ul>
        <p>When someone accepts your friend request <em>with mesh</em>, you also become tailnet peers — so all the Lobby stuff (above) just works between you.</p>
        <p>Want to disappear? Toggle <em>● Appearing offline</em> in the Discover header. You still receive DMs.</p>
      </>
    ),
  },
  {
    id: "first-run",
    icon: "🚀",
    title: "First launch — what happens automatically",
    body: (
      <>
        <p>You don't configure anything. Within ~1 minute of first launch:</p>
        <ul className="list-disc pl-5 space-y-1">
          <li>Your directory identity is auto-minted (UUID + your hostname as display name)</li>
          <li>You appear online on Discover</li>
          <li>Sunshine, Moonlight, and Tailscale install themselves (two UAC prompts — click Yes)</li>
          <li>Tailscale opens a browser to sign you in — pick any account (Google, Microsoft, email)</li>
          <li>All 13 emulators start downloading in the background (~600 MB, takes 5–10 min)</li>
        </ul>
        <p>Then click your <em>Library</em> tab → <em>Add a games folder</em> → pick where your ROMs live → Abyss scans and shows them as cards.</p>
      </>
    ),
  },
  {
    id: "bios",
    icon: "💾",
    title: "BIOS files — what each console needs",
    body: (
      <>
        <p><b>Works out of the box (no BIOS):</b> NES, SNES, Genesis, N64, GBA, Game Boy, DS, Master System, Atari 2600, PSP, GameCube, Wii.</p>
        <p><b>Need you to provide BIOS</b> (we can't legally ship them):</p>
        <ul className="list-disc pl-5 space-y-1">
          <li><b>PS1</b>: <code className="text-abyss-accent">scph1001.bin</code> (or similar), exactly 524 288 bytes. Drop in <code>%LocalAppData%\DuckStation\bios\</code>.</li>
          <li><b>PS2</b>: <code className="text-abyss-accent">scph10000.bin</code> / <code>scph39001.bin</code> etc., exactly 4 MB. Drop in <code>Documents\PCSX2\bios\</code>.</li>
          <li><b>PS3</b>: One-click install via <em>Settings → Emulators → Install PS3 firmware</em> (Sony's free public download).</li>
          <li><b>Dreamcast</b>: <code className="text-abyss-accent">dc_boot.bin</code> + <code>dc_flash.bin</code>. Drop in <code>%APPDATA%\flycast\data\</code>.</li>
        </ul>
        <p>After dropping a BIOS anywhere reasonable (Downloads / Desktop / Documents / RetroArch / DuckStation / PCSX2), run <em>Settings → Diagnose &amp; Repair → Run</em> → Abyss locates + copies to every emulator that needs it.</p>
      </>
    ),
  },
  {
    id: "trouble",
    icon: "⚠️",
    title: "Something's not working",
    body: (
      <>
        <p><b>Click Play → nothing happens?</b></p>
        <ol className="list-decimal pl-5 space-y-1">
          <li>A red banner should pop up on the Library tab. Read it.</li>
          <li>"No emulator installed" → click <em>Install all emulators</em>.</li>
          <li>Anything else → click <em>Run Repair</em>. Idempotent — always safe.</li>
        </ol>
        <p><b>Game launches and dies in a split second?</b></p>
        <p>A yellow crash banner appears with the exact command Abyss ran + the last error from the emulator. Click <em>Copy details</em> and send to your host.</p>
        <p><b>Streaming says "host rejected the credentials"?</b></p>
        <p>Stream tab → click the yellow <em>Set Sunshine creds</em> button. One UAC prompt → fixed.</p>
        <p><b>Need general help?</b></p>
        <p>Diagnose & Repair → Run → at the bottom click <em>Copy to clipboard</em> → paste to your host.</p>
      </>
    ),
  },
  {
    id: "streaming",
    icon: "📺",
    title: "Streaming (Sunshine + Moonlight)",
    body: (
      <>
        <p>Different from netplay. <b>Sunshine + Moonlight = like Steam Remote Play.</b> One PC runs the game; the other watches the screen and sends gamepad input back.</p>
        <p>Use it when: a friend wants to play a game they don't own, or one of you has a beefy PC and the other has a Chromebook.</p>
        <ol className="list-decimal pl-5 space-y-1">
          <li>Host: Stream tab → <em>Start host</em>. (Sunshine begins capturing.)</li>
          <li>Friend: Friends tab → click <em>stream</em> next to your row.</li>
          <li>Their Moonlight pairs + opens streaming your desktop. No PINs to exchange.</li>
        </ol>
      </>
    ),
  },
  {
    id: "uninstall",
    icon: "🗑️",
    title: "Uninstall / wipe everything",
    body: (
      <>
        <p>Settings → Apps → <em>Abyss Singularity</em> → Uninstall.</p>
        <p>If you also want to remove the emulators + library cache + saves (~600 MB), delete <code className="text-abyss-accent">%APPDATA%\com.abyss.singularity\</code> manually.</p>
      </>
    ),
  },
];
