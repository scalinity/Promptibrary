//! Per-provider rate limiter for the extraction pipeline.
//!
//! Combines a `tokio::sync::Semaphore` (concurrency cap) with a per-provider
//! token-bucket (interval + burst) per spec §6 *Rate limiting and caching*.
//!
//! Usage:
//! ```ignore
//! let _permit = limiter.acquire(Provider::Anthropic).await;
//! // make the call; permit drops when the guard goes out of scope.
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{Mutex, OwnedSemaphorePermit, Semaphore};
use tokio::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Provider {
    Anthropic,
    Youtube,
    XTwitter,
    Article,
}

#[derive(Debug, Clone, Copy)]
pub struct RateLimitPolicy {
    pub max_concurrent: u32,
    pub min_interval: Duration,
    pub burst: u32,
}

impl Provider {
    pub fn policy(self) -> RateLimitPolicy {
        match self {
            Self::Anthropic => RateLimitPolicy {
                max_concurrent: 1,
                min_interval: Duration::from_millis(1000),
                burst: 2,
            },
            Self::Youtube => RateLimitPolicy {
                max_concurrent: 1,
                min_interval: Duration::from_millis(1500),
                burst: 1,
            },
            Self::XTwitter => RateLimitPolicy {
                max_concurrent: 1,
                min_interval: Duration::from_millis(1000),
                burst: 3,
            },
            Self::Article => RateLimitPolicy {
                max_concurrent: 4,
                min_interval: Duration::from_millis(250),
                burst: 8,
            },
        }
    }
}

struct ProviderState {
    semaphore: Arc<Semaphore>,
    bucket: Mutex<TokenBucket>,
}

struct TokenBucket {
    capacity: u32,
    tokens: f64,
    refill_per_second: f64,
    last_refill: Instant,
}

impl TokenBucket {
    fn new(burst: u32, interval: Duration) -> Self {
        let refill_per_second = 1.0 / interval.as_secs_f64();
        Self {
            capacity: burst,
            tokens: burst as f64,
            refill_per_second,
            last_refill: Instant::now(),
        }
    }

    /// Take one token, returning the duration the caller must sleep before
    /// the token is theirs (zero if it was already available).
    fn take_one(&mut self) -> Duration {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_per_second).min(self.capacity as f64);
        self.last_refill = now;

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            Duration::ZERO
        } else {
            let missing = 1.0 - self.tokens;
            let secs = missing / self.refill_per_second;
            self.tokens = 0.0;
            self.last_refill = now + Duration::from_secs_f64(secs);
            Duration::from_secs_f64(secs)
        }
    }
}

pub struct RateLimiter {
    providers: HashMap<Provider, ProviderState>,
}

impl RateLimiter {
    pub fn new() -> Self {
        let mut providers = HashMap::new();
        for &p in &[
            Provider::Anthropic,
            Provider::Youtube,
            Provider::XTwitter,
            Provider::Article,
        ] {
            let policy = p.policy();
            providers.insert(
                p,
                ProviderState {
                    semaphore: Arc::new(Semaphore::new(policy.max_concurrent as usize)),
                    bucket: Mutex::new(TokenBucket::new(policy.burst, policy.min_interval)),
                },
            );
        }
        Self { providers }
    }

    /// Acquire a permit for `provider`. Token-bucket first (cheap interval
    /// wait without holding a semaphore), then concurrency cap. Returns an
    /// owned permit; drop it once the call completes.
    pub async fn acquire(&self, provider: Provider) -> OwnedSemaphorePermit {
        let state = self
            .providers
            .get(&provider)
            .expect("provider registered at construction time");
        let wait = {
            let mut bucket = state.bucket.lock().await;
            bucket.take_one()
        };
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
        state
            .semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("semaphore is never closed")
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policies_match_spec() {
        let p = Provider::Anthropic.policy();
        assert_eq!(p.max_concurrent, 1);
        assert_eq!(p.min_interval, Duration::from_millis(1000));
        assert_eq!(p.burst, 2);

        let p = Provider::Youtube.policy();
        assert_eq!(p.max_concurrent, 1);
        assert_eq!(p.min_interval, Duration::from_millis(1500));
        assert_eq!(p.burst, 1);

        let p = Provider::XTwitter.policy();
        assert_eq!(p.max_concurrent, 1);
        assert_eq!(p.min_interval, Duration::from_millis(1000));
        assert_eq!(p.burst, 3);

        let p = Provider::Article.policy();
        assert_eq!(p.max_concurrent, 4);
        assert_eq!(p.min_interval, Duration::from_millis(250));
        assert_eq!(p.burst, 8);
    }

    #[test]
    fn token_bucket_initial_burst_immediate() {
        let mut b = TokenBucket::new(2, Duration::from_millis(1000));
        assert_eq!(b.take_one(), Duration::ZERO);
        assert_eq!(b.take_one(), Duration::ZERO);
        // Third take should require a wait close to 1s (modulo nanos
        // accumulated between the calls above).
        let waited = b.take_one();
        assert!(
            waited > Duration::from_millis(900) && waited <= Duration::from_millis(1000),
            "expected ~1000ms wait, got {waited:?}"
        );
    }

    #[test]
    fn token_bucket_caps_at_burst() {
        let mut b = TokenBucket::new(2, Duration::from_millis(1000));
        // Simulate a long idle period — refill rate ~1/sec, sleeping for
        // 10s should top out at the burst capacity, not 10 tokens.
        b.last_refill = Instant::now() - Duration::from_secs(10);
        assert_eq!(b.take_one(), Duration::ZERO);
        assert_eq!(b.take_one(), Duration::ZERO);
        // Third take has to wait.
        assert!(b.take_one() > Duration::from_millis(500));
    }

    #[tokio::test]
    async fn acquire_respects_concurrency_cap() {
        // Article limiter has max_concurrent=4. Hold 4 permits then prove
        // a fifth acquire is still pending after ~100ms.
        let limiter = Arc::new(RateLimiter::new());
        let mut held = Vec::new();
        for _ in 0..4 {
            held.push(limiter.acquire(Provider::Article).await);
        }

        let limiter_clone = limiter.clone();
        let fifth = tokio::spawn(async move { limiter_clone.acquire(Provider::Article).await });

        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(!fifth.is_finished(), "fifth permit acquired despite cap");

        held.pop();
        let _permit = tokio::time::timeout(Duration::from_millis(800), fifth)
            .await
            .expect("fifth should acquire after a permit is released")
            .unwrap();
    }
}
