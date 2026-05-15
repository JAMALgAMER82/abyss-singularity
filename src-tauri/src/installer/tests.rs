use std::collections::HashSet;

use super::manifests;
use crate::library::types::Platform;

#[test]
fn all_manifests_have_unique_ids() {
    let mut seen: HashSet<&str> = HashSet::new();
    for m in manifests::all().iter() {
        assert!(seen.insert(&m.id), "duplicate manifest id: {}", m.id);
    }
}

#[test]
fn every_manifest_id_matches_a_built_in_recipe() {
    // The auto-assign flow assumes the installer id and the
    // orchestration recipe id agree — verify here so a typo can't
    // ship silently broken.
    let recipe_ids: HashSet<String> = crate::orchestration::recipes::builtin_recipes()
        .into_iter()
        .map(|r| r.id)
        .collect();
    for m in manifests::all() {
        assert!(
            recipe_ids.contains(&m.id),
            "manifest {} has no matching orchestration recipe", m.id,
        );
    }
}

#[test]
fn retroarch_covers_the_major_retro_platforms() {
    let m = manifests::find_by_id("retroarch").expect("retroarch manifest");
    for required in [
        Platform::Nes, Platform::Snes, Platform::N64, Platform::GameboyAdvance,
        Platform::Genesis, Platform::Ps1, Platform::Psp,
    ] {
        assert!(m.platforms.contains(&required), "RetroArch missing {required:?}");
    }
}

#[test]
fn urls_are_https_or_gh_latest() {
    // `gh-latest://owner/repo/asset-substring` is resolved at install time
    // by the GitHub releases API and ultimately fetches an https asset.
    for m in manifests::all() {
        assert!(
            m.url.starts_with("https://") || m.url.starts_with("gh-latest://"),
            "{} download URL is not https or gh-latest: {}", m.id, m.url,
        );
    }
}

#[test]
fn embeddable_flags_are_set_intentionally() {
    // RetroArch / mGBA / PPSSPP / DeSmuME / Snes9x / Stella are simple
    // Win32 UIs we've confirmed embed cleanly via SetParent. Qt-based
    // emulators (PCSX2 v2, RPCS3, Simple64 [project64 id], DuckStation
    // [pcsx-redux id]) and Dolphin/Cemu fall back to minimise-and-restore.
    for m in manifests::all() {
        match m.id.as_str() {
            "retroarch" | "mgba" | "ppsspp" | "desmume" | "snes9x" | "stella" => assert!(m.embeddable),
            "pcsx2" | "rpcs3" | "dolphin" | "cemu" | "pcsx-redux" | "project64" | "flycast" => assert!(!m.embeddable),
            other => panic!("unknown manifest id {other} — update this test"),
        }
    }
}
