use std::time::Instant;

/// Tracks token timing for a single streaming response.
///
/// All time values are injected externally — this module never calls `Instant::now()`.
/// This design enables fully deterministic unit tests.
#[derive(Debug, Clone)]
pub struct TokenSession {
    start: Instant,
    first_token: Option<Instant>,
    token_count: u64,
    last_token: Option<Instant>,
}

impl TokenSession {
    /// Create a new session starting at the given instant.
    pub fn new(start: Instant) -> Self {
        Self {
            start,
            first_token: None,
            token_count: 0,
            last_token: None,
        }
    }

    /// Record a token arrival at the given instant.
    pub fn record_token(&mut self, now: Instant) {
        if self.first_token.is_none() {
            self.first_token = Some(now);
        }
        self.token_count += 1;
        self.last_token = Some(now);
    }

    /// Tokens per second, calculated as count / elapsed seconds.
    /// Returns 0.0 if no tokens recorded or zero elapsed time.
    pub fn tps(&self, now: Instant) -> f64 {
        if self.token_count == 0 {
            return 0.0;
        }
        let elapsed = now.duration_since(self.start).as_secs_f64();
        if elapsed <= 0.0 {
            return 0.0;
        }
        self.token_count as f64 / elapsed
    }

    /// Time to first token in milliseconds.
    /// Returns `None` if no tokens have been recorded.
    pub fn ttft(&self) -> Option<f64> {
        self.first_token
            .map(|ft| ft.duration_since(self.start).as_secs_f64() * 1000.0)
    }

    /// Number of tokens recorded so far.
    pub fn token_count(&self) -> u64 {
        self.token_count
    }

    /// Elapsed time in seconds from start to `now`.
    pub fn elapsed(&self, now: Instant) -> f64 {
        now.duration_since(self.start).as_secs_f64()
    }
}

/// Kahan summation for improved floating-point accumulation precision.
///
/// Standard sequential summation loses precision when adding many small values
/// to a large running total. Kahan's algorithm tracks the lost low-order bits
/// in a compensation variable.
pub fn kahan_sum(values: &[f64]) -> f64 {
    let mut sum = 0.0_f64;
    let mut compensation = 0.0_f64;
    for &val in values {
        let y = val - compensation;
        let t = sum + y;
        compensation = (t - sum) - y;
        sum = t;
    }
    sum
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn instant_plus(base: Instant, millis: u64) -> Instant {
        base + Duration::from_millis(millis)
    }

    #[test]
    fn test_basic_tps() {
        let start = Instant::now();
        let mut session = TokenSession::new(start);

        // Record 100 tokens over 5 seconds
        for i in 1..=100 {
            let t = instant_plus(start, i * 50); // spread over 5000ms
            session.record_token(t);
        }

        let end = instant_plus(start, 5000);
        let tps = session.tps(end);
        assert!((tps - 20.0).abs() < 0.01, "Expected ~20 TPS, got {tps}");
    }

    #[test]
    fn test_ttft() {
        let start = Instant::now();
        let mut session = TokenSession::new(start);

        let first = instant_plus(start, 150);
        session.record_token(first);

        let ttft = session.ttft().unwrap();
        assert!(
            (ttft - 150.0).abs() < 0.1,
            "Expected ~150ms TTFT, got {ttft}"
        );
    }

    #[test]
    fn test_zero_tokens() {
        let start = Instant::now();
        let session = TokenSession::new(start);
        let later = instant_plus(start, 1000);
        assert_eq!(session.tps(later), 0.0);
        assert!(session.ttft().is_none());
    }

    #[test]
    fn test_single_token() {
        let start = Instant::now();
        let mut session = TokenSession::new(start);
        let t = instant_plus(start, 500);
        session.record_token(t);

        assert_eq!(session.token_count(), 1);
        let tps = session.tps(instant_plus(start, 1000));
        assert!((tps - 1.0).abs() < 0.01, "Expected ~1.0 TPS, got {tps}");
    }

    #[test]
    fn test_many_tokens() {
        let start = Instant::now();
        let mut session = TokenSession::new(start);

        for i in 1..=1000 {
            session.record_token(instant_plus(start, i));
        }
        assert_eq!(session.token_count(), 1000);

        let tps = session.tps(instant_plus(start, 1000));
        assert!(
            (tps - 1000.0).abs() < 1.0,
            "Expected ~1000 TPS, got {tps}"
        );
    }

    #[test]
    fn test_kahan_sum_precision() {
        // Classic case: sum of many small values where naive sum loses precision
        let values: Vec<f64> = (0..10_000).map(|_| 0.1).collect();
        let result = kahan_sum(&values);
        assert!(
            (result - 1000.0).abs() < 1e-10,
            "Kahan sum should be ~1000.0, got {result}"
        );
    }

    #[test]
    fn test_kahan_sum_empty() {
        assert_eq!(kahan_sum(&[]), 0.0);
    }

    #[test]
    fn test_elapsed() {
        let start = Instant::now();
        let session = TokenSession::new(start);
        let later = instant_plus(start, 2500);
        let elapsed = session.elapsed(later);
        assert!(
            (elapsed - 2.5).abs() < 0.01,
            "Expected ~2.5s elapsed, got {elapsed}"
        );
    }

    #[test]
    fn test_token_count() {
        let start = Instant::now();
        let mut session = TokenSession::new(start);
        assert_eq!(session.token_count(), 0);
        session.record_token(instant_plus(start, 100));
        session.record_token(instant_plus(start, 200));
        session.record_token(instant_plus(start, 300));
        assert_eq!(session.token_count(), 3);
    }

    #[test]
    fn test_first_token_not_overwritten() {
        let start = Instant::now();
        let mut session = TokenSession::new(start);
        session.record_token(instant_plus(start, 100));
        session.record_token(instant_plus(start, 200));

        let ttft = session.ttft().unwrap();
        assert!(
            (ttft - 100.0).abs() < 0.1,
            "TTFT should remain at first token time"
        );
    }
}
