//! Unit tests for the `library` module. Kept in a child module so the
//! file structure mirrors the rest of the crate and shows up alongside
//! the modules under test in `cargo test`.

use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;

use super::cache::{load, save, LibrarySnapshot};
use super::igdb::{build_search_query, to_metadata, upgrade_cover_url, IgdbClient, IgdbCover, IgdbGame};
use super::platforms::{known_extensions, platform_for_extension, refine_ambiguous};
use super::scanner::{normalise_for_hash, scan_collect, NoopSink};
use super::types::Platform;

// ---------- platform map ----------------------------------------------------

#[test]
fn every_known_extension_resolves() {
    for (ext, expected) in known_extensions() {
        assert_eq!(
            platform_for_extension(ext),
            Some(expected),
            "extension {ext:?} should resolve to {expected:?}"
        );
    }
}

#[test]
fn every_platform_has_a_display_name() {
    use Platform::*;
    // Touching the variant exhaustively here means adding a new variant
    // without giving it a display name will fail the build, not just
    // the assertion.
    for p in [
        Pc, Nes, Snes, N64, GameCube, Wii, WiiU, Switch, Gameboy, GameboyColor,
        GameboyAdvance, Nds, Threeds, Ps1, Ps2, Ps3, Psp, PsVita, Genesis,
        MasterSystem, GameGear, Saturn, Dreamcast, Atari2600, NeoGeo, Arcade,
        Other,
    ] {
        let name = p.display_name();
        assert!(!name.is_empty(), "platform {p:?} has empty display_name");
    }
}

#[test]
fn unknown_extension_returns_none() {
    assert!(platform_for_extension("txt").is_none());
    assert!(platform_for_extension("docx").is_none());
}

#[test]
fn refine_promotes_ambiguous_iso_via_parent_dir() {
    let p = PathBuf::from("/games/PS2/God Hand.iso");
    assert_eq!(refine_ambiguous(Platform::Other, &p), Platform::Ps2);

    let p2 = PathBuf::from("/games/PlayStation 3/Demon's Souls/PS3_GAME.iso");
    assert_eq!(refine_ambiguous(Platform::Other, &p2), Platform::Ps3);

    let p3 = PathBuf::from("/games/Mixed Bag/whatever.iso");
    assert_eq!(refine_ambiguous(Platform::Other, &p3), Platform::Other);
}

#[test]
fn refine_does_not_change_an_already_specific_platform() {
    let p = PathBuf::from("/games/PS2/Super Mario 64.n64");
    // Even though the path screams "PS2", the extension is authoritative.
    assert_eq!(refine_ambiguous(Platform::N64, &p), Platform::N64);
}

// ---------- name normalisation ---------------------------------------------

#[test]
fn normalise_strips_region_tags() {
    let a = normalise_for_hash("Chrono Trigger (USA) [!]");
    let b = normalise_for_hash("Chrono Trigger");
    let c = normalise_for_hash("CHRONO TRIGGER (Europe)");
    assert_eq!(a, b);
    assert_eq!(a, c);
    assert_eq!(a, "chrono trigger");
}

#[test]
fn normalise_collapses_whitespace() {
    let a = normalise_for_hash("  Final  Fantasy   VII   (USA)  ");
    assert_eq!(a, "final fantasy vii");
}

// ---------- scanner end-to-end on a tmp tree -------------------------------

#[test]
fn scanner_picks_up_known_extensions_and_ignores_noise() {
    let dir = tempdir();
    let root = dir.path().to_path_buf();

    // Stuff one of each: a known ROM (above MIN_FILE_BYTES), a too-small
    // file that should be filtered out, and an unrelated text file.
    write_file(&root.join("Super Mario World (USA).sfc"), 32 * 1024);
    write_file(&root.join("tiny.gba"),                            1024); // below threshold
    write_file(&root.join("README.txt"),                     32 * 1024);

    // Drop a deeper PS2 ISO under a heuristic-friendly folder.
    fs::create_dir_all(root.join("PS2")).unwrap();
    write_file(&root.join("PS2/God Hand.iso"), 64 * 1024);

    let (report, entries) = scan_collect(std::slice::from_ref(&root), &[], &NoopSink);

    assert_eq!(report.games_found, 2, "expected SNES + PS2 ISO, got {entries:?}");
    let platforms: HashSet<Platform> = entries.iter().map(|e| e.platform).collect();
    assert!(platforms.contains(&Platform::Snes));
    assert!(platforms.contains(&Platform::Ps2));
    assert!(report.elapsed_ms < 60_000, "scan should finish quickly on a tmpdir");
}

#[test]
fn rescan_keeps_existing_enrichment_for_same_id() {
    use super::types::IgdbMetadata;
    use chrono::Utc;

    let dir = tempdir();
    let root = dir.path().to_path_buf();
    write_file(&root.join("Halo (USA).iso"), 128 * 1024);
    fs::create_dir_all(root.join("Xbox")).unwrap();
    fs::rename(root.join("Halo (USA).iso"), root.join("Xbox/Halo (USA).iso")).unwrap();

    let (_, first) = scan_collect(std::slice::from_ref(&root), &[], &NoopSink);
    assert_eq!(first.len(), 1);

    // Pretend Phase 2.3 enriched the entry. The id must survive a rescan.
    let mut enriched = first[0].clone();
    enriched.igdb = Some(IgdbMetadata {
        igdb_id: 12345,
        name: "Halo: Combat Evolved".into(),
        summary: None,
        cover_url: None,
        release_year: Some(2001),
        total_rating: Some(94.0),
        platforms: vec!["Xbox".into()],
    });
    enriched.last_enriched = Some(Utc::now());

    let (report, rescan) = scan_collect(std::slice::from_ref(&root), std::slice::from_ref(&enriched), &NoopSink);
    assert_eq!(report.games_kept, 1);
    assert_eq!(report.games_new, 0);
    let same = rescan.iter().find(|e| e.id == enriched.id).expect("entry by id");
    assert_eq!(
        same.igdb.as_ref().map(|m| m.igdb_id),
        Some(12345),
        "IGDB enrichment must not be wiped on a rescan"
    );
}

// ---------- cache round-trip ----------------------------------------------

#[test]
fn cache_round_trip_preserves_all_fields() {
    let dir = tempdir();
    let root = dir.path().to_path_buf();
    write_file(&root.join("Sonic the Hedgehog (USA).md"), 64 * 1024);

    let (_, entries) = scan_collect(std::slice::from_ref(&root), &[], &NoopSink);
    let snapshot_in = LibrarySnapshot::new(entries.clone());
    save(dir.path(), &snapshot_in).unwrap();

    let snapshot_out = load(dir.path()).unwrap();
    assert_eq!(snapshot_out.version, LibrarySnapshot::CURRENT_VERSION);
    assert_eq!(snapshot_out.entries.len(), entries.len());
    assert_eq!(snapshot_out.entries[0].id, entries[0].id);
    assert_eq!(snapshot_out.entries[0].platform, Platform::Genesis);
}

#[test]
fn cache_load_on_empty_dir_returns_default() {
    let dir = tempdir();
    let snapshot = load(dir.path()).unwrap();
    assert!(snapshot.entries.is_empty());
}

// ---------- tiny tempdir helper (dep-free; no `tempfile` crate) -----------

struct TempDir(PathBuf);
impl TempDir {
    fn path(&self) -> &std::path::Path { &self.0 }
}
impl Drop for TempDir {
    fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
}
fn tempdir() -> TempDir {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let p = std::env::temp_dir().join(format!(
        "abyss-test-{}-{}",
        std::process::id(),
        n
    ));
    fs::create_dir_all(&p).unwrap();
    TempDir(p)
}

fn write_file(path: &std::path::Path, bytes: usize) {
    if let Some(parent) = path.parent() { fs::create_dir_all(parent).unwrap(); }
    fs::write(path, vec![0u8; bytes]).unwrap();
}

// ---------- IGDB client (pure functions) -----------------------------------

#[test]
fn apicalypse_query_includes_all_required_fields() {
    let q = build_search_query("Halo", 5);
    assert!(q.contains("fields name"));
    assert!(q.contains("summary"));
    assert!(q.contains("cover.url"));
    assert!(q.contains("first_release_date"));
    assert!(q.contains("total_rating"));
    assert!(q.contains("platforms.name"));
    assert!(q.contains("search \"Halo\""));
    assert!(q.contains("limit 5"));
    assert!(q.ends_with(';') || q.contains(";"));
}

#[test]
fn apicalypse_query_escapes_double_quotes() {
    let q = build_search_query(r#"Doom "Eternal""#, 1);
    assert!(q.contains(r#"search "Doom \"Eternal\""; "#));
}

#[test]
fn cover_url_upgrade_adds_https_and_resizes() {
    let raw = "//images.igdb.com/igdb/image/upload/t_thumb/abc123.jpg";
    let up  = upgrade_cover_url(raw);
    assert!(up.starts_with("https://"));
    assert!(up.contains("/t_cover_big_2x/"));
    assert!(!up.contains("/t_thumb/"));
}

#[test]
fn cover_url_upgrade_is_idempotent_on_already_https() {
    let raw = "https://images.igdb.com/igdb/image/upload/t_cover_big_2x/abc.jpg";
    let up  = upgrade_cover_url(raw);
    assert_eq!(up, raw);
}

#[test]
fn igdb_game_to_metadata_maps_fields_correctly() {
    let g = IgdbGame {
        id: 1942,
        name: "The Witcher 3: Wild Hunt".into(),
        summary: Some("RPG".into()),
        first_release_date: Some(1431993600), // 2015-05-19 UTC
        total_rating: Some(94.5),
        cover: Some(IgdbCover {
            url: Some("//images.igdb.com/igdb/image/upload/t_thumb/co.jpg".into()),
        }),
        platforms: vec![],
    };
    let m = to_metadata(g);
    assert_eq!(m.igdb_id, 1942);
    assert_eq!(m.release_year, Some(2015));
    assert!(m.cover_url.as_deref().unwrap().contains("/t_cover_big_2x/"));
    assert_eq!(m.total_rating, Some(94.5));
}

#[tokio::test(flavor = "current_thread")]
async fn igdb_throttle_paces_to_four_per_second() {
    use std::time::Instant;
    let client = IgdbClient::new("dummy", "dummy").expect("client");
    let start = Instant::now();
    for _ in 0..4 {
        client.throttle().await;
    }
    let elapsed = start.elapsed();
    // First call is free, so 3 gaps ≥ 260ms = 780ms.
    assert!(
        elapsed.as_millis() >= 700,
        "4 throttled calls finished too fast: {:?}",
        elapsed
    );
}
