use std::path::PathBuf;

use super::recipes::{builtin_recipes, expand_args};
use super::types::{EmulatorEntry, OrchestrationConfig};
use crate::library::types::{LibraryEntry, Platform};

fn sample_entry(stem: &str, ext: &str) -> LibraryEntry {
    LibraryEntry {
        id: "abc".into(),
        path: PathBuf::from(format!("C:/Games/{stem}.{ext}")),
        file_name: format!("{stem}.{ext}"),
        stem: stem.to_string(),
        extension: ext.to_string(),
        size_bytes: 1024,
        modified: chrono::Utc::now(),
        platform: Platform::Snes,
        igdb: None,
        cover_local_path: None,
        last_enriched: None,
    }
}

#[test]
fn expand_args_substitutes_all_tokens() {
    let template = vec![
        "--load".to_string(),
        "{game_path}".to_string(),
        "--label".to_string(),
        "{game_stem}".to_string(),
        "--root".to_string(),
        "{game_dir}".to_string(),
    ];
    let entry = sample_entry("Chrono Trigger (USA)", "sfc");
    let out = expand_args(&template, &entry);

    assert_eq!(out[0], "--load");
    assert!(out[1].ends_with("Chrono Trigger (USA).sfc"));
    assert_eq!(out[2], "--label");
    assert_eq!(out[3], "Chrono Trigger (USA)");
    assert_eq!(out[4], "--root");
    assert!(out[5].ends_with("Games"));
}

#[test]
fn expand_args_handles_repeated_tokens() {
    let template = vec!["{game_stem}={game_stem}".to_string()];
    let entry = sample_entry("Sonic", "md");
    let out = expand_args(&template, &entry);
    assert_eq!(out[0], "Sonic=Sonic");
}

#[test]
fn expand_args_is_a_noop_for_tokenless_args() {
    let template = vec!["-fullscreen".to_string()];
    let entry = sample_entry("Halo", "iso");
    assert_eq!(expand_args(&template, &entry), vec!["-fullscreen"]);
}

#[test]
fn builtin_recipes_cover_every_platform_at_least_once() {
    use std::collections::HashSet;
    let recipes = builtin_recipes();
    assert!(!recipes.is_empty(), "no built-in recipes!");
    let covered: HashSet<Platform> = recipes
        .iter()
        .flat_map(|r| r.platforms.iter().copied())
        .collect();
    // These are the platforms we MUST be able to launch something for.
    // Switch and 3DS are intentionally absent: Ryujinx (Switch) and Citra
    // (3DS) were shut down in late 2024 and we don't ship the legally-
    // murky forks. PS Vita is similarly user-configured. Other platforms
    // below should always have at least one option baked in.
    for required in [
        Platform::Pc, Platform::Snes, Platform::N64, Platform::GameCube,
        Platform::Wii, Platform::Ps1, Platform::Ps2,
        Platform::Ps3, Platform::GameboyAdvance, Platform::Nds,
        Platform::Dreamcast,
    ] {
        assert!(
            covered.contains(&required),
            "no built-in recipe covers {required:?}"
        );
    }
}

#[test]
fn builtin_recipe_ids_are_unique() {
    use std::collections::HashSet;
    let recipes = builtin_recipes();
    let mut seen: HashSet<&str> = HashSet::new();
    for r in &recipes {
        assert!(seen.insert(&r.id), "duplicate recipe id: {}", r.id);
    }
}

#[test]
fn orchestration_config_round_trips_through_json() {
    let cfg = OrchestrationConfig {
        emulators: vec![EmulatorEntry {
            id: "retroarch".into(),
            name: "RetroArch".into(),
            exe: PathBuf::from("C:/Program Files/RetroArch/retroarch.exe"),
            args: vec!["-L".into(), "cores/snes9x_libretro.dll".into(), "-f".into(), "{game_path}".into()],
            working_dir: None,
            env: Default::default(),
            platforms: vec![Platform::Snes, Platform::N64],
        }],
        assignments: [(Platform::Snes, "retroarch".into())].into_iter().collect(),
    };
    let json = serde_json::to_string(&cfg).unwrap();
    let back: OrchestrationConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(back.emulators, cfg.emulators);
    assert_eq!(back.assignments, cfg.assignments);
}
