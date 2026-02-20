use approx::assert_relative_eq;
use ollama_scope::metrics::{kahan_sum, TokenSession};
use proptest::prelude::*;
use std::time::{Duration, Instant};

fn instant_plus(base: Instant, millis: u64) -> Instant {
    base + Duration::from_millis(millis)
}

/// Build a TokenSession with `count` tokens spread over `duration_ms`.
fn build_session(count: u64, duration_ms: u64) -> (TokenSession, Instant, Instant) {
    let start = Instant::now();
    let mut session = TokenSession::new(start);
    if count > 0 && duration_ms > 0 {
        let interval = duration_ms / count;
        for i in 1..=count {
            session.record_token(instant_plus(start, i * interval));
        }
    }
    let end = instant_plus(start, duration_ms);
    (session, start, end)
}

#[test]
fn test_typical_metrics_snapshot() {
    let (session, _start, end) = build_session(50, 2500);
    let tps = session.tps(end);
    let ttft = session.ttft().unwrap();
    let count = session.token_count();

    let snapshot = format!(
        "token_count: {count}\ntps: {tps:.2}\nttft_ms: {ttft:.2}\nelapsed_s: {:.2}",
        session.elapsed(end)
    );

    insta::assert_yaml_snapshot!(snapshot);
}

#[test]
fn test_approx_float_comparison() {
    let (session, _start, end) = build_session(100, 5000);
    let tps = session.tps(end);
    assert_relative_eq!(tps, 20.0, max_relative = 1e-10);
}

#[test]
fn test_kahan_vs_naive_large_sum() {
    let values: Vec<f64> = (0..100_000).map(|_| 1e-10).collect();
    let kahan = kahan_sum(&values);
    let naive: f64 = values.iter().sum();
    // Kahan should be closer to the true value
    let expected = 100_000.0 * 1e-10;
    assert!(
        (kahan - expected).abs() <= (naive - expected).abs(),
        "Kahan should be at least as accurate as naive sum"
    );
}

proptest! {
    #[test]
    fn prop_tps_non_negative(count in 0u64..1000, duration_ms in 1u64..10000) {
        let (session, _start, end) = build_session(count, duration_ms);
        let tps = session.tps(end);
        prop_assert!(tps >= 0.0, "TPS must be non-negative, got {tps}");
    }

    #[test]
    fn prop_tps_finite(count in 0u64..1000, duration_ms in 1u64..10000) {
        let (session, _start, end) = build_session(count, duration_ms);
        let tps = session.tps(end);
        prop_assert!(tps.is_finite(), "TPS must be finite, got {tps}");
    }

    #[test]
    fn prop_kahan_sum_finite(values in proptest::collection::vec(-1e10f64..1e10, 0..100)) {
        let result = kahan_sum(&values);
        prop_assert!(result.is_finite(), "Kahan sum must be finite, got {result}");
    }
}
