//! Best-effort console BIOS / firmware locator.
//!
//! Legal stance: Abyss never bundles or downloads copyrighted console
//! firmware (Sony PS1/PS2 BIOS, Sega Dreamcast BIOS, Nintendo keys).
//! Distributing those files would expose the project to DMCA takedown
//! and the user to copyright liability. But if the user has dumped or
//! obtained a BIOS for *another* emulator on this same PC (RetroArch's
//! `system/` folder, an old PCSX2 install, their Downloads folder, …),
//! we can re-use it without distributing anything. That's pure file
//! discovery + local copy — zero network, zero distribution.
//!
//! Approach:
//! 1. Walk a curated list of "places people stash BIOS files" up to a
//!    shallow depth — never the whole disk.
//! 2. Identify candidates by filename **and** filesize signature (a
//!    real `scph1001.bin` is exactly 524 288 bytes; a junk file with
//!    the same name isn't).
//! 3. Copy the match into each emulator's expected BIOS folder.
//!
//! Result: if any PS1 BIOS dump exists anywhere reasonable on this PC,
//! DuckStation finds it on next launch with zero user action.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::Result;

/// A specific BIOS file fingerprint: lowercase filename + exact byte
/// size. Both must match or we don't copy — protects against junk
/// files that happen to share a name.
#[derive(Debug, Clone, Copy)]
struct BiosFingerprint {
    filename: &'static str,
    size:     u64,
}

/// One BIOS slot we want to fill. `targets` are the destination dirs
/// (relative paths under `$USERPROFILE` or `$LOCALAPPDATA` or the
/// Abyss emulator dir) we copy a found match into.
#[derive(Debug, Clone)]
struct BiosSlot {
    label:        &'static str,
    fingerprints: &'static [BiosFingerprint],
    /// Where this emulator looks for its BIOS — first-existing wins
    /// for read; we write into ALL of them so any path works.
    targets:      Vec<PathBuf>,
}

/// All BIOS slots Abyss knows about. Resolved at runtime against the
/// current user's profile.
///
/// Each slot lists every place an emulator that can play that platform
/// might look for its BIOS — including the libretro `system/` folder of
/// the RetroArch we install ourselves. That last one is *crucial* for
/// PS1: when RetroArch loads SwanStation / Beetle PSX / PCSX ReARMed,
/// those cores read PS1 BIOSes from `<retroarch>/system/`. Without that
/// path as a target, a friend who has BIOS in DuckStation's folder
/// gets a half-working setup — standalone DuckStation works but
/// RetroArch-via-SwanStation silently fails to launch (exit 1, ~150 ms,
/// no stderr because the core init error never surfaces).
fn slots() -> Vec<BiosSlot> {
    let user_profile = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default();
    let local_app    = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_default();
    let roaming      = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_default();

    // Where the RetroArch we install lives, derived from the Tauri identifier
    // we set in tauri.conf.json. Stays in sync because the installer extracts
    // every emulator under `<app_data>/emulators/<id>/...`.
    let abyss_data = roaming.join("com.abyss.singularity");
    let abyss_retroarch_system = abyss_data
        .join("emulators")
        .join("retroarch")
        .join("RetroArch-Win64")
        .join("system");

    vec![
        BiosSlot {
            label: "PS1 (DuckStation + RetroArch libretro cores)",
            fingerprints: &[
                // Every shipped scph PS1 BIOS revision. Each is exactly
                // 524 288 bytes — that's the PS1 boot ROM size.
                BiosFingerprint { filename: "scph1001.bin", size: 524_288 },
                BiosFingerprint { filename: "scph1002.bin", size: 524_288 },
                BiosFingerprint { filename: "scph5500.bin", size: 524_288 },
                BiosFingerprint { filename: "scph5501.bin", size: 524_288 },
                BiosFingerprint { filename: "scph5502.bin", size: 524_288 },
                BiosFingerprint { filename: "scph7001.bin", size: 524_288 },
                BiosFingerprint { filename: "scph7002.bin", size: 524_288 },
                BiosFingerprint { filename: "scph7003.bin", size: 524_288 },
                BiosFingerprint { filename: "scph7501.bin", size: 524_288 },
                BiosFingerprint { filename: "scph7502.bin", size: 524_288 },
                BiosFingerprint { filename: "scph7503.bin", size: 524_288 },
                BiosFingerprint { filename: "ps-30a.bin",   size: 524_288 },
            ],
            targets: vec![
                local_app.join("DuckStation").join("bios"),
                user_profile.join("Documents").join("DuckStation").join("bios"),
                abyss_retroarch_system.clone(),
            ],
        },
        BiosSlot {
            label: "PS2 (PCSX2)",
            fingerprints: &[
                // PCSX2 BIOS files are exactly 4 MB.
                BiosFingerprint { filename: "scph10000.bin", size: 4_194_304 },
                BiosFingerprint { filename: "scph30004.bin", size: 4_194_304 },
                BiosFingerprint { filename: "scph39001.bin", size: 4_194_304 },
                BiosFingerprint { filename: "scph70004.bin", size: 4_194_304 },
                BiosFingerprint { filename: "scph70012.bin", size: 4_194_304 },
                BiosFingerprint { filename: "scph77001.bin", size: 4_194_304 },
                BiosFingerprint { filename: "scph90001.bin", size: 4_194_304 },
            ],
            targets: vec![
                // Standard non-portable PCSX2 v2 config dir on Windows.
                user_profile.join("Documents").join("PCSX2").join("bios"),
                // Portable-mode location: a bios/ folder next to the exe
                // we extracted. Defensive — PCSX2 only uses this when a
                // portable.txt exists, but covering it costs nothing and
                // helps if a future build flips the default.
                abyss_data.join("emulators").join("pcsx2").join("bios"),
            ],
        },
        BiosSlot {
            label: "Dreamcast (Flycast + libretro)",
            fingerprints: &[
                BiosFingerprint { filename: "dc_boot.bin",  size: 2_097_152 },
                BiosFingerprint { filename: "dc_flash.bin", size:   131_072 },
            ],
            targets: vec![
                roaming.join("flycast").join("data"),
                user_profile.join(".local").join("share").join("flycast"),
                // Flycast libretro looks under system/dc/ for the BIOS pair.
                abyss_retroarch_system.join("dc"),
            ],
        },
    ]
}

/// Directories worth scanning for stashed BIOS files. Shallow depths
/// only — we don't want to recurse into game-collection folders.
fn search_roots() -> Vec<(PathBuf, usize)> {
    let user_profile = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default();
    let local_app    = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_default();
    let roaming      = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_default();
    let abyss_retroarch_system = roaming
        .join("com.abyss.singularity")
        .join("emulators").join("retroarch").join("RetroArch-Win64").join("system");

    vec![
        // RetroArch system folder — the single most likely place a
        // multi-emulator user already has BIOS files. Includes the one
        // we manage ourselves, so a user who manually dropped BIOS into
        // our install dir gets it propagated.
        (abyss_retroarch_system, 2),
        (local_app.join("RetroArch").join("system"), 1),
        (user_profile.join("RetroArch").join("system"), 1),
        (user_profile.join("Documents").join("RetroArch").join("system"), 1),
        // Older emulator install dirs.
        (user_profile.join("Documents").join("PCSX2").join("bios"), 1),
        (user_profile.join("Documents").join("DuckStation").join("bios"), 1),
        (local_app.join("DuckStation").join("bios"), 1),
        (roaming.join("flycast").join("data"), 1),
        // Common user-stash spots.
        (user_profile.join("Downloads"), 3),
        (user_profile.join("Desktop"), 2),
        (user_profile.join("Documents"), 2),
    ]
}

/// Run the auto-finder. Returns a map of slot label → list of files we
/// copied (paths in the target emulator dirs). An empty map means we
/// didn't find anything — the user has to provide BIOS files manually.
pub fn auto_install_all() -> Result<HashMap<String, Vec<PathBuf>>> {
    let slot_list = slots();
    let roots     = search_roots();

    // For each fingerprint we care about, scan the search roots for a
    // matching file. First match wins per slot — we don't need every
    // BIOS revision, any one will do.
    let mut results: HashMap<String, Vec<PathBuf>> = HashMap::new();

    for slot in &slot_list {
        // Source priority: any target that already has a matching BIOS
        // (correct filename + exact size), then any of the search roots.
        // Critical fix: we used to skip the entire slot when *any one*
        // target already had the BIOS, which meant a user with BIOS in
        // DuckStation's folder got nothing propagated to RetroArch's
        // system/ — the friend's "Pepsiman exits in 163 ms" symptom.
        let target_with_bios = slot.targets.iter().find_map(|t| {
            slot.fingerprints.iter().find_map(|fp| {
                let p = t.join(fp.filename);
                let meta = std::fs::metadata(&p).ok()?;
                (meta.is_file() && meta.len() == fp.size).then_some(p)
            })
        });
        let found = match target_with_bios.or_else(|| find_one_of(&roots, slot.fingerprints)) {
            Some(p) => p,
            None    => {
                log::info!("bios_finder: no BIOS anywhere for {}", slot.label);
                continue;
            }
        };

        log::info!("bios_finder: source {} BIOS at {}", slot.label, found.display());
        let mut copied_to: Vec<PathBuf> = Vec::new();
        for target_dir in &slot.targets {
            if let Err(e) = std::fs::create_dir_all(target_dir) {
                log::warn!("bios_finder: mkdir {}: {e}", target_dir.display());
                continue;
            }
            let target_path = target_dir.join(found.file_name().unwrap_or_default());
            if target_path.exists() { continue; }
            match std::fs::copy(&found, &target_path) {
                Ok(_) => {
                    log::info!("bios_finder: copied {} -> {}", found.display(), target_path.display());
                    copied_to.push(target_path);
                }
                Err(e) => log::warn!("bios_finder: copy to {} failed: {e}", target_path.display()),
            }
        }
        if !copied_to.is_empty() {
            results.insert(slot.label.to_string(), copied_to);
        }
    }
    Ok(results)
}

/// Walk every search root looking for ANY file whose lowercase basename
/// matches one of the fingerprints AND whose size matches exactly.
fn find_one_of(
    roots:        &[(PathBuf, usize)],
    fingerprints: &[BiosFingerprint],
) -> Option<PathBuf> {
    for (root, max_depth) in roots {
        if !root.exists() { continue }
        let walker = walkdir::WalkDir::new(root)
            .max_depth(*max_depth)
            .follow_links(false)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file());

        for entry in walker {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else { continue };
            let lower = name.to_ascii_lowercase();
            // Skip files whose names don't match any fingerprint — using
            // `?` here would abort the whole search on the first unrelated
            // file walkdir surfaces (e.g. a readme.txt next to the BIOS).
            let Some(matching) = fingerprints.iter().find(|fp| fp.filename == lower) else { continue };
            let Ok(meta) = path.metadata() else { continue };
            if meta.len() == matching.size {
                return Some(path.to_path_buf());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    // Per-test directory under the OS temp dir. We can't depend on the
    // `tempfile` crate (it's not in Cargo.toml), so cook our own with a
    // process-unique counter to avoid collisions in parallel test runs.
    fn temp_root(label: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let pid = std::process::id();
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("abyss-bios-test-{label}-{pid}-{n}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    struct DirGuard(PathBuf);
    impl Drop for DirGuard {
        fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
    }

    /// One PS1 BIOS fingerprint, useful across every test below.
    const PS1: BiosFingerprint = BiosFingerprint { filename: "scph1001.bin", size: 524_288 };

    #[test]
    fn matches_by_filename_and_exact_size() {
        let root = temp_root("hit");
        let _g = DirGuard(root.clone());
        let bios = root.join("scph1001.bin");
        fs::write(&bios, vec![0u8; PS1.size as usize]).unwrap();

        let found = find_one_of(&[(root.clone(), 1)], &[PS1]).expect("should find BIOS");
        assert_eq!(found, bios);
    }

    #[test]
    fn rejects_filename_match_with_wrong_size() {
        // A file named like a BIOS but the wrong size must NOT match —
        // this is what protects against truncated downloads / junk dumps.
        let root = temp_root("size");
        let _g = DirGuard(root.clone());
        fs::write(root.join("scph1001.bin"), vec![0u8; 1024]).unwrap();

        assert!(find_one_of(&[(root.clone(), 1)], &[PS1]).is_none());
    }

    #[test]
    fn ignores_unrelated_files_in_root() {
        // Regression test for the `?`-instead-of-`continue` bug that
        // aborted the whole walk on the first file whose name didn't
        // match any fingerprint. We arrange the noise files so they
        // sort alphabetically *before* the real BIOS, maximising the
        // chance the walker surfaces them first.
        let root = temp_root("noise");
        let _g = DirGuard(root.clone());
        fs::write(root.join("a-readme.txt"), b"hello").unwrap();
        fs::write(root.join("b-other.dat"), vec![0u8; 4096]).unwrap();
        let bios = root.join("scph1001.bin");
        fs::write(&bios, vec![0u8; PS1.size as usize]).unwrap();

        let found = find_one_of(&[(root.clone(), 1)], &[PS1]).expect("walk must skip non-matches");
        assert_eq!(found, bios);
    }

    #[test]
    fn filename_match_is_case_insensitive() {
        let root = temp_root("case");
        let _g = DirGuard(root.clone());
        let bios = root.join("SCPH1001.BIN");
        fs::write(&bios, vec![0u8; PS1.size as usize]).unwrap();

        assert!(find_one_of(&[(root.clone(), 1)], &[PS1]).is_some());
    }

    #[test]
    fn honours_max_depth() {
        let root = temp_root("depth");
        let _g = DirGuard(root.clone());
        let nested = root.join("a").join("b").join("c");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("scph1001.bin"), vec![0u8; PS1.size as usize]).unwrap();

        // Depth 5 reaches a/b/c/file (root + 3 dirs + file = 4 hops, well under 5).
        assert!(find_one_of(&[(root.clone(), 5)], &[PS1]).is_some());
        // Depth 1 means only direct children of root — file is too deep.
        assert!(find_one_of(&[(root.clone(), 1)], &[PS1]).is_none());
    }

    #[test]
    fn skips_nonexistent_root_silently() {
        let nope = std::env::temp_dir().join("abyss-bios-test-does-not-exist-pls");
        let _ = fs::remove_dir_all(&nope);
        // Must not panic; just returns None.
        assert!(find_one_of(&[(nope, 3)], &[PS1]).is_none());
    }

    #[test]
    fn matches_any_fingerprint_in_slot() {
        // We feed a 5500 BIOS but the slot lists every scph revision —
        // any one match should suffice.
        let root = temp_root("alt");
        let _g = DirGuard(root.clone());
        fs::write(root.join("scph5500.bin"), vec![0u8; 524_288]).unwrap();

        let fps = [
            BiosFingerprint { filename: "scph1001.bin", size: 524_288 },
            BiosFingerprint { filename: "scph5500.bin", size: 524_288 },
        ];
        assert!(find_one_of(&[(root.clone(), 1)], &fps).is_some());
    }
}

/// Manual BIOS install: the user picked a file via the OS dialog and we
/// drop it into every emulator slot whose fingerprints match. Returns
/// the slot label we placed it under, or `None` if the file's name /
/// size don't match anything Abyss recognises.
pub fn install_picked_file(picked: &Path) -> Result<Option<String>> {
    let Some(name) = picked.file_name().and_then(|n| n.to_str()) else {
        return Ok(None);
    };
    let lower = name.to_ascii_lowercase();
    let size  = picked.metadata().map(|m| m.len()).unwrap_or(0);

    for slot in slots() {
        let Some(fp) = slot.fingerprints.iter().find(|fp| fp.filename == lower) else { continue };
        if fp.size != size { continue }
        // Match — copy to all targets.
        for target_dir in &slot.targets {
            std::fs::create_dir_all(target_dir).ok();
            let target_path = target_dir.join(name);
            std::fs::copy(picked, &target_path).ok();
        }
        return Ok(Some(slot.label.to_string()));
    }
    Ok(None)
}
