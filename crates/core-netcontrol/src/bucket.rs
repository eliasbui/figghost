//! A token bucket: how many bytes an app may move now, at a rate that the rules and the priorities set.

use std::time::{Duration, Instant};

/// The bucket holds at most this many seconds of its rate, so that a pause is not followed by a burst.
const BURST_SECONDS: f64 = 0.25;
/// The smallest burst, so that a slow rate still moves whole packets.
const MIN_BURST: f64 = 16.0 * 1024.0;

#[derive(Debug, Clone)]
pub struct Bucket {
    /// Bytes a second; `None` is no limit.
    rate: Option<f64>,
    tokens: f64,
    at: Instant,
}

impl Bucket {
    pub fn new(now: Instant) -> Self {
        Self {
            rate: None,
            tokens: 0.0,
            at: now,
        }
    }

    pub fn rate(&self) -> Option<f64> {
        self.rate
    }

    pub fn set_rate(&mut self, rate: Option<f64>, now: Instant) {
        self.refill(now);
        let rate = rate
            .filter(|r| r.is_finite() && *r > 0.0)
            .map(|r| r.max(1024.0));
        if rate != self.rate {
            self.rate = rate;
            self.tokens = self.tokens.min(burst(rate));
        }
    }

    fn refill(&mut self, now: Instant) {
        let elapsed = now.saturating_duration_since(self.at).as_secs_f64();
        self.at = now;
        if let Some(rate) = self.rate {
            self.tokens = (self.tokens + rate * elapsed).min(burst(Some(rate)));
        }
    }

    /// Takes up to `want` bytes: how many may move now, and if none, how long to wait before asking again.
    pub fn take(&mut self, want: usize, now: Instant) -> Result<usize, Duration> {
        self.refill(now);
        let Some(rate) = self.rate else {
            return Ok(want);
        };
        if self.tokens >= 1.0 {
            let n = (self.tokens.floor() as usize).min(want).max(1);
            self.tokens -= n as f64;
            return Ok(n);
        }
        let wait = (1.0 - self.tokens) / rate;
        Err(Duration::from_secs_f64(wait.clamp(0.001, 0.25)))
    }

    /// Gives back what was taken and not used.
    pub fn refund(&mut self, n: usize) {
        if let Some(rate) = self.rate {
            self.tokens = (self.tokens + n as f64).min(burst(Some(rate)));
        }
    }
}

fn burst(rate: Option<f64>) -> f64 {
    rate.map_or(f64::INFINITY, |r| (r * BURST_SECONDS).max(MIN_BURST))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_a_rate_everything_moves() {
        let now = Instant::now();
        let mut b = Bucket::new(now);
        assert_eq!(b.take(1 << 20, now), Ok(1 << 20));
    }

    #[test]
    fn a_rate_lets_through_about_that_much_a_second() {
        let start = Instant::now();
        let mut b = Bucket::new(start);
        b.set_rate(Some(100_000.0), start);
        let mut moved = 0usize;
        let mut now = start;
        // Ten seconds in steps of ten milliseconds, asking for more than the rate each time.
        for _ in 0..1000 {
            now += Duration::from_millis(10);
            if let Ok(n) = b.take(64 * 1024, now) {
                moved += n;
            }
        }
        let per_second = moved as f64 / 10.0;
        assert!((95_000.0..=105_000.0).contains(&per_second), "{per_second}");
    }

    #[test]
    fn an_empty_bucket_says_how_long_to_wait_and_a_pause_is_no_burst() {
        let start = Instant::now();
        let mut b = Bucket::new(start);
        b.set_rate(Some(64_000.0), start);
        assert!(b.take(1000, start).is_err(), "it starts empty");
        let later = start + Duration::from_secs(60);
        let n = b.take(usize::MAX, later).unwrap();
        assert!(n as f64 <= burst(Some(64_000.0)), "{n}");
        b.set_rate(None, later);
        assert_eq!(b.take(5, later), Ok(5));
    }
}
