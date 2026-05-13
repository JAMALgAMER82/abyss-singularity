use super::latency::{recommend, recommend_pair};
use super::regions::default_targets;
use super::types::{LatencyProfile, ProbeResult};

fn ok(id: &str, ms: u32) -> ProbeResult {
    ProbeResult {
        id:         id.into(),
        label:      id.into(),
        continent:  "Test".into(),
        host:       "test.local".into(),
        port:       443,
        latency_ms: Some(ms),
        error:      None,
    }
}
fn fail(id: &str) -> ProbeResult {
    ProbeResult {
        id:         id.into(),
        label:      id.into(),
        continent:  "Test".into(),
        host:       "test.local".into(),
        port:       443,
        latency_ms: None,
        error:      Some("timeout".into()),
    }
}

#[test]
fn recommend_picks_lowest_reachable_region() {
    let r = recommend(&[ok("a", 120), ok("b", 30), fail("c"), ok("d", 90)]);
    assert!(r.is_some());
    let rec = r.unwrap();
    assert_eq!(rec.id, "b");
    assert_eq!(rec.latency_ms, 30);
}

#[test]
fn recommend_returns_none_when_nothing_is_reachable() {
    assert!(recommend(&[fail("a"), fail("b")]).is_none());
}

#[test]
fn pair_recommender_minimises_worst_case() {
    let targets = default_targets();
    let any_two: Vec<&str> = targets.iter().take(2).map(|t| t.id.as_str()).collect();
    let id_a = any_two[0].to_string();
    let id_b = any_two[1].to_string();

    let p1 = LatencyProfile {
        measurements: [(id_a.clone(), 30), (id_b.clone(), 200)].into_iter().collect(),
    };
    let p2 = LatencyProfile {
        // Player 2 is mirror-image: bad for region A, good for region B.
        measurements: [(id_a.clone(), 250), (id_b.clone(), 80)].into_iter().collect(),
    };

    let rec = recommend_pair(&p1, &p2, &targets).expect("recommendation");
    // worst(A) = max(30, 250) = 250
    // worst(B) = max(200, 80) = 200  ← winner.
    assert_eq!(rec.id, id_b);
    assert_eq!(rec.latency_ms, 200);
}

#[test]
fn pair_recommender_ignores_regions_only_one_player_measured() {
    let targets = default_targets();
    let id = targets.first().expect("at least one default target").id.clone();
    let p1 = LatencyProfile { measurements: [(id.clone(), 20)].into_iter().collect() };
    let p2 = LatencyProfile { measurements: Default::default() };
    assert!(recommend_pair(&p1, &p2, &targets).is_none());
}

#[test]
fn default_targets_span_at_least_four_continents() {
    use std::collections::HashSet;
    let targets = default_targets();
    let cs: HashSet<&str> = targets.iter().map(|t| t.continent.as_str()).collect();
    assert!(cs.len() >= 4, "expected ≥4 continents, got {cs:?}");
}

// (Status JSON projection now happens in `tailscale::status()` against the
// mesh sidecar's structured response — no more CLI-output parser to test.)
