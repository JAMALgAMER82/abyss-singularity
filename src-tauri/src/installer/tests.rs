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
fn urls_are_https_only() {
    for m in manifests::all() {
        assert!(
            m.url.starts_with("https://"),
            "{} download URL is not https: {}", m.id, m.url,
        );
    }
}

#[test]
fn embeddable_flags_are_set_intentionally() {
    // RetroArch / mGBA / PPSSPP / DeSmuME are the simple Win32 UIs we've
    // confirmed embed cleanly via SetParent. The modern Qt-based ones
    // (PCSX2 v2, RPCS3) and Dolphin are explicitly false so the UI knows
    // to fall back to the minimise-and-restore launch flow.
    for m in manifests::all() {
        match m.id.as_str() {
            "retroarch" | "mgba" | "ppsspp" | "desmume" | "snes9x" | "project64" | "stella" => assert!(m.embeddable),
            "pcsx2" | "rpcs3" | "dolphin" | "cemu" | "pcsx-redux"                            => assert!(!m.embeddable),
            other => panic!("unknown manifest id {other} — update this test"),
        }
    }
}
