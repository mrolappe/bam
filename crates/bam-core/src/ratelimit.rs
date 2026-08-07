//! Configurable token-bucket rate limiter (P4.2) over an injected [`Clock`],
//! so it is testable without sleeping. Used by the harvest worker (P4.3) to
//! throttle mirror requests to a polite rate.

use std::cell::Cell;
use std::time::{Duration, Instant};

use serde::Deserialize;

/// Documented default: 2.0 requests per second.
pub const DEFAULT_RATE: f64 = 2.0;
/// Documented default: burst of 4 immediate requests.
pub const DEFAULT_BURST: u32 = 4;

/// The `[rate_limit]` section of `bam.toml`. Absent fields — including an
/// entirely absent section — fall back to the documented defaults, not a
/// hand-written zero value.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct RateLimitConfig {
    #[serde(default = "default_rate")]
    pub rate: f64,
    #[serde(default = "default_burst")]
    pub burst: u32,
}

fn default_rate() -> f64 {
    DEFAULT_RATE
}

fn default_burst() -> u32 {
    DEFAULT_BURST
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            rate: DEFAULT_RATE,
            burst: DEFAULT_BURST,
        }
    }
}

/// A configured rate that can never let a request through.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("rate limit: rate must be positive, got {0}")]
pub struct NonPositiveRate(pub f64);

/// A source of the current time, injected so [`TokenBucket`] is testable
/// without a real clock or real sleeping.
pub trait Clock {
    fn now(&self) -> Instant;
}

/// The real clock, for production use.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> Instant {
        Instant::now()
    }
}

impl<C: Clock> Clock for &C {
    fn now(&self) -> Instant {
        (*self).now()
    }
}

/// Token bucket: `capacity` tokens (the burst), refilled continuously at
/// `rate` tokens/second, never exceeding `capacity`. `try_acquire` is the
/// only operation — non-blocking by design, so a caller (real or test) drives
/// its own waiting/sleeping rather than this type doing it internally.
pub struct TokenBucket<C: Clock> {
    clock: C,
    rate: f64,
    capacity: f64,
    tokens: Cell<f64>,
    last_refill: Cell<Instant>,
}

impl<C: Clock> TokenBucket<C> {
    pub fn new(config: &RateLimitConfig, clock: C) -> Result<Self, NonPositiveRate> {
        if config.rate <= 0.0 {
            return Err(NonPositiveRate(config.rate));
        }
        let now = clock.now();
        Ok(Self {
            clock,
            rate: config.rate,
            capacity: config.burst as f64,
            tokens: Cell::new(config.burst as f64),
            last_refill: Cell::new(now),
        })
    }

    fn refill(&self) {
        let now = self.clock.now();
        let elapsed = now.duration_since(self.last_refill.get()).as_secs_f64();
        let refilled = (self.tokens.get() + elapsed * self.rate).min(self.capacity);
        self.tokens.set(refilled);
        self.last_refill.set(now);
    }

    /// Takes one token if one is available now. Otherwise returns how long
    /// the caller must wait before a token will be available, leaving the
    /// bucket untouched — the caller decides whether and how to wait.
    pub fn try_acquire(&self) -> Result<(), Duration> {
        self.refill();
        let tokens = self.tokens.get();
        if tokens >= 1.0 {
            self.tokens.set(tokens - 1.0);
            Ok(())
        } else {
            let deficit = 1.0 - tokens;
            Err(Duration::from_secs_f64(deficit / self.rate))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct FakeClock(RefCell<Instant>);

    impl FakeClock {
        fn new() -> Self {
            Self(RefCell::new(Instant::now()))
        }

        fn advance(&self, d: Duration) {
            *self.0.borrow_mut() += d;
        }
    }

    impl Clock for FakeClock {
        fn now(&self) -> Instant {
            *self.0.borrow()
        }
    }

    /// Drives `bucket` through `n` acquisitions, advancing the fake clock by
    /// exactly the reported wait whenever one is needed. Returns total fake
    /// time elapsed.
    fn drain(bucket: &TokenBucket<&FakeClock>, clock: &FakeClock, n: usize) -> Duration {
        let start = clock.now();
        for _ in 0..n {
            while let Err(wait) = bucket.try_acquire() {
                clock.advance(wait);
            }
        }
        clock.now().duration_since(start)
    }

    #[test]
    fn observes_the_configured_rate_in_fake_time() {
        let clock = FakeClock::new();
        let config = RateLimitConfig {
            rate: 2.0,
            burst: 4,
        };
        let bucket = TokenBucket::new(&config, &clock).unwrap();

        let wall_start = Instant::now();
        let elapsed = drain(&bucket, &clock, 100);
        let wall_elapsed = wall_start.elapsed();

        // 4 requests are free (the burst); the remaining 96 arrive one every
        // 1/rate = 0.5s of fake time -> 48s.
        let expected = Duration::from_secs_f64(96.0 / 2.0);
        let diff = elapsed.as_secs_f64() - expected.as_secs_f64();
        assert!(
            diff.abs() < 0.01,
            "elapsed={elapsed:?} expected={expected:?}"
        );

        // No real sleeping happened.
        assert!(
            wall_elapsed < Duration::from_millis(200),
            "{wall_elapsed:?}"
        );
    }

    #[test]
    fn burst_allows_n_immediate_requests_then_throttles() {
        let clock = FakeClock::new();
        let config = RateLimitConfig {
            rate: 2.0,
            burst: 4,
        };
        let bucket = TokenBucket::new(&config, &clock).unwrap();

        for _ in 0..4 {
            assert!(bucket.try_acquire().is_ok());
        }
        let wait = bucket.try_acquire().unwrap_err();
        assert!(wait > Duration::ZERO);
    }

    #[test]
    fn absent_config_yields_the_documented_defaults() {
        let config: RateLimitConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(config, RateLimitConfig::default());
        assert_eq!(config.rate, DEFAULT_RATE);
        assert_eq!(config.burst, DEFAULT_BURST);
    }

    #[test]
    fn a_configured_rate_of_zero_is_rejected_at_load() {
        let clock = FakeClock::new();
        let config = RateLimitConfig {
            rate: 0.0,
            burst: 4,
        };
        match TokenBucket::new(&config, &clock) {
            Err(err) => assert_eq!(err, NonPositiveRate(0.0)),
            Ok(_) => panic!("expected a rejected rate of 0"),
        }
    }
}
