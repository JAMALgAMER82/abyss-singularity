# Abyss Singularity — User Manual

Everything you need to play games with your friends. Written for normal humans,
not power users. If you can copy-paste, you can use Abyss.

---

## 1. What is Abyss?

It's one app that does **four** things:

| | What | When you'd use it |
|---|---|---|
| 🕹️ | Run any retro / older game on your PC (NES, SNES, N64, Game Boy, PS1, PS2, GameCube, Wii, Dreamcast, PSP, PS3, etc.) | Solo gaming with emulators |
| 👥 | See your friends online + chat with them | Hanging out, planning a session |
| 🎮 | Play the same game together over the internet (real netplay) | Mario Kart 64 / Smash 64 / Street Fighter II with your friend across the world |
| 📺 | Stream your screen + gamepad to a friend (like Steam Remote Play) | Letting a friend "borrow" your PC remotely |

No subscriptions. No account signup. Everything works peer-to-peer.

---

## 2. Installation (one time, ~2 minutes)

1. **Get the zip** your friend sent you (`Abyss-Singularity-Install.zip`) and extract it
2. **Double-click** `Abyss Singularity_0.2.0_x64-setup.exe`
3. Windows might say *"Windows protected your PC"* — click **More info → Run anyway** (totally normal for indie apps that haven't paid for a code-signing cert)
4. Wait ~5 seconds — it installs silently to your user folder (no UAC needed)
5. Abyss opens automatically

That's it. The app installs to `%LocalAppData%\Abyss Singularity\` so you can uninstall later from Settings → Apps without admin rights.

---

## 3. First launch — what happens automatically

The app does a lot of setup work in the background so you don't have to:

| Time after launch | What's happening |
|---|---|
| Immediately | Your "name" on the directory is auto-set to your PC's hostname. The mesh network sidecar starts. |
| ~10 s | You appear as "online" on the global directory (Discover tab). |
| ~15 s | Sunshine + Tailscale will install themselves — you'll see **two Windows UAC prompts** (those blue "Do you want to allow this app to make changes" dialogs). Click **Yes** to both. Moonlight installs silently in between. |
| ~30 s | All 13 emulators start downloading in the background (~600 MB total, takes 5–10 minutes on a normal connection). You can use the app while this runs. |
| ~1 minute | Tailscale will pop up a browser window asking you to sign in — sign in with anything (Google / Microsoft / email). This is your network identity. Free, no card needed. |

After all that, everything is set up. You only do this once per PC.

---

## 4. The five tabs

The left sidebar has five icons. From top to bottom:

### 📚 Library (Ctrl+1)
Your local games. **First time**, click "Add a games folder" and point it at the folder where your `.iso` / `.nes` / `.smc` files live. Abyss scans them, figures out which platform each one is (NES vs. SNES vs. PS1 etc.) by file extension + filename, and shows them as cards.

**Click "▶ Play"** on any game → it launches the right emulator automatically.

### 🌐 Network (Ctrl+2)
Shows your Tailscale connection status + a regional latency probe for picking the best relay region when streaming long-distance. **You rarely need to touch this** — it's mostly diagnostic.

### 📺 Stream (Ctrl+3)
Run a Sunshine server here (lets friends stream from your PC) or launch Moonlight (to stream from someone else's). See section 7.

### 👥 Friends (Ctrl+4)
The party tab. Three things in one:
1. **📨 Invite codes** — set up tailnet sharing (for the lobby below)
2. **🎮 Game lobby** — host a game, friends join, everyone launches at once
3. **Chat + transfer** — text chat per mesh peer + send game files

See section 5.

### 🛰️ Discover (Ctrl+5)
**GameRanger style.** See everyone using Abyss online. Send friend requests. Chat in the global lobby. See section 6.

### ⚙️ Settings (Ctrl+6)
Configuration. The "Help & user guide" you're reading is also accessible here.

---

## 5. Playing the same game with a friend (the Lobby)

This is the headline feature. The way it works:

### Step 1 — get your friend on your "tailnet"

Tailnet = your private peer-to-peer network. Both of you need to be on the same one for the lobby to work.

**Easiest way** (recommended):

1. **You**: open Friends tab → Invite a friend → follow the 3 numbered steps (open Tailscale link, paste a key, generate a code)
2. **You**: send the generated invite code to your friend (Discord / WhatsApp / email — any way)
3. **Friend**: opens Friends tab → "Friend sent you a code?" box → paste it → click **▶ Join my friend**
4. Their app reconfigures their tailnet automatically. ~10 seconds later they appear in your Friends peer list.

### Step 2 — open a chat channel

In the Friends tab's peer list (left column), click the **"link"** button next to your friend's row. This opens a chat session so the lobby can talk to them.

### Step 3 — host a game

1. In the Lobby panel, type to filter games → pick one from the dropdown
2. Click **🎮 Host this game** — your friend gets a banner popup: *"You're hosting Mario Kart 64 — Join"*

### Step 4 — friend joins

They click **▶ Join the game** on their banner.

### Step 5 — start

You click **▶ Start the game for everyone**.

**Both Abyss apps launch RetroArch with the right netplay flags simultaneously.** RetroArch handles the actual game-state sync. You're playing together within seconds.

### Important caveats

- **Both of you need the same ROM file.** Library matches by game name. If they don't have it, Abyss says so — you can send it via the file-transfer feature in the Friends tab.
- **RetroArch netplay only.** Works for: NES, SNES, Genesis, GBA, N64, PS1, PSP, Atari 2600, NeoGeo, MAME, Master System, Game Boy, GBC, NDS — anything that runs in a libretro core.
- **Standalone Dolphin / PCSX2 / RPCS3 / Cemu**: no auto-netplay. For these, use the **Real netplay** section at the bottom of the Stream tab — copy each other's tailnet IPs, paste into the emulator's own netplay menu.

---

## 6. Discover — see everyone online (GameRanger style)

The Discover tab is a global directory of everyone running Abyss right now.

### What you see

- **Left column**: list of online users (their handle, country, last-seen time)
- **Middle column**: when you click someone you've friended, your DM history with them
- **Right column**: global chat — visible to every Abyss user (like an IRC channel)

### Add a friend

1. Click **+ add** next to any online user
2. Their Abyss gets a notification banner
3. They click **✓ Accept + mesh** — your two Abysses pair, you become friends
4. From now on you can DM them, see when they're online, and you're on the same tailnet so the Lobby (section 5) works automatically

### Appear offline

Toggle the **● Online** button (top right of Discover) to **● Appearing offline** if you don't want to show up in the global list. You still receive friend requests + DMs.

### Global chat

Bottom-right of Discover. Be nice — rate-limited to 6 messages per minute.

---

## 7. Streaming from someone's PC

Different from netplay. **Sunshine + Moonlight = like Steam Remote Play.** One person runs the game on their PC; the other watches the screen + sends their gamepad input back. Useful when:

- You want to let a friend play a game they don't own
- One of you has a beefy PC, the other has a Chromebook
- You want couch co-op but you're not on the same couch

### Setup

Both apps install automatically on first launch (section 3). You don't configure anything.

### Stream from your PC to a friend's

1. **You** (host): Stream tab → click **Start host** (Sunshine starts capturing)
2. **Friend**: Friends tab → find your row → click **stream** button
3. Both apps pair automatically. Moonlight opens on their side with your desktop streaming in.

No PINs to read aloud. No browser dance. Just two clicks.

---

## 8. BIOS / firmware — what you need for which console

Abyss can't legally ship copyrighted BIOS files. Some emulators need one to run real games:

| Console | BIOS required? | Where to put it |
|---|---|---|
| NES, SNES, Genesis, GBA, N64, Game Boy, DS, Master System, Atari 2600 | **No** — works out of the box | — |
| PS1 (RetroArch SwanStation) | **Yes** — `scph1001.bin` (or any variant). Exactly 524,288 bytes. | Drop into `%LocalAppData%\DuckStation\bios\` — Abyss auto-copies to all places |
| PS1 (DuckStation standalone) | Same | Same |
| PS2 (PCSX2) | **Yes** — `scph10000.bin` / `scph39001.bin` / `scph77001.bin` etc. Exactly 4 MB. | `%USERPROFILE%\Documents\PCSX2\bios\` |
| PS3 (RPCS3) | **Yes** — Sony's official `PS3UPDAT.PUP` | One-click install: Settings → Emulators → "Install PS3 firmware" |
| Dreamcast (Flycast) | **Yes** — `dc_boot.bin` + `dc_flash.bin` | `%APPDATA%\flycast\data\` |
| GameCube / Wii (Dolphin) | **No** — HLE built-in | — |
| PSP (PPSSPP) | **No** — HLE built-in | — |
| Wii U (Cemu) | Sometimes (for online + some games) | Cemu has an in-app firmware installer |

Abyss has an **auto-BIOS finder** that scans common locations (Downloads, Documents, RetroArch system folder, DuckStation/PCSX2/Flycast folders) for matching files by exact filename + filesize. Drop your BIOS anywhere in those folders → Settings → Diagnose → Run → Abyss copies it everywhere needed.

---

## 9. Troubleshooting

### "Click Play, nothing happens"

A red banner should appear at the top of the Library tab:

- **"No emulator installed"** → click **Install all emulators** (~600 MB). Wait for the download bar.
- **"Run Repair"** suggestion → click it. Idempotent — safe to spam.
- **"Show technical details"** → expand for the exact error string. Send it to your Abyss host (the person who shared the app with you).

### Game launches then crashes in <1 second

A yellow banner appears: **"[Game] launched but died after N ms"**. Expand the panels:

- **Command Abyss ran** — the exact CLI invocation. If you can run the emulator manually and it works, compare your manual command to this one and spot the difference.
- **Last stderr lines** — usually has the real reason ("BIOS not found", "core .dll missing", etc.).
- **Copy details** button → paste the whole report to your Abyss host for help.

### Friend can't see me in Discover

Possible causes:
1. They haven't set up the directory yet → Settings → Directory → paste the same Worker URL their host gave them
2. You're on **Appear offline** → toggle off in Discover header
3. Worker is down → check `https://abyss-directory.<your-host>.workers.dev/health` in a browser → should return `{"ok":true}`

### Streaming says "host rejected"

Sunshine doesn't have its admin credentials set on the host side. Host: Stream tab → click the yellow **"Set Sunshine creds"** button. One UAC prompt → fixed.

### Auto-installer didn't fire on first launch

You may have an existing partial install. Just run **Settings → Diagnose & Repair → Run** — it triggers the same install paths manually and auto-repairs config.

### Diagnostic report for your Abyss host

If anything is weird, share a diagnostic. **Settings → Diagnose & Repair → Run** → at the bottom click **Copy to clipboard** → paste it to your host. It includes every subsystem's status without leaking any personal data.

---

## 10. Where things live on disk

| Path | What |
|---|---|
| `%LocalAppData%\Abyss Singularity\` | The app itself + uninstaller |
| `%APPDATA%\com.abyss.singularity\` | Settings, library cache, **emulators**, RetroArch cores |
| `%APPDATA%\com.abyss.singularity\emulators\` | All 13 emulators we install (~600 MB) |
| `%LocalAppData%\com.abyss.singularity\logs\` | App logs (the "diagnostic report" reads from here) |
| `%LocalAppData%\AbyssSingularity\tailscale\` | Tailscale mesh state — wipe this to force a fresh sign-in |
| `%LocalAppData%\DuckStation\bios\` | DuckStation BIOS folder (also where Abyss looks for PS1 BIOS) |
| `%USERPROFILE%\Documents\PCSX2\bios\` | PCSX2 BIOS folder |

To completely uninstall: Settings → Apps → Abyss Singularity → Uninstall. Then optionally delete `%APPDATA%\com.abyss.singularity\` to remove emulators / library cache / saves.

---

## 11. Privacy & what data is shared

- **Your tailnet** (Tailscale) is between you and the friends who redeemed your invite code. Nothing flows through anyone else's servers — it's true peer-to-peer.
- **The directory** (Discover tab) only stores: your randomly-generated UUID, the display name you picked, your country (if you set one), your app version, and a "last seen" timestamp. No emails, no IPs.
- **Game files / saves** stay on your PC. Abyss doesn't upload them anywhere. The file-transfer feature is peer-to-peer over your tailnet.
- **Chat messages** in the Friends tab go peer-to-peer over your tailnet. DMs + global chat in the Discover tab go through the Worker (your host's Cloudflare account).
- **BIOS files** never leave your PC. Abyss only copies them between folders on the same machine.

---

## 12. Getting help

- Most issues fix themselves with **Settings → Diagnose & Repair → Run**.
- Send your host a diagnostic report (Copy to clipboard button in Diagnose).
- Project repo: https://github.com/JAMALgAMER82/abyss-singularity
- Issues / bug reports: https://github.com/JAMALgAMER82/abyss-singularity/issues

---

*Last updated: 2026-05-15 — v0.2.0*
