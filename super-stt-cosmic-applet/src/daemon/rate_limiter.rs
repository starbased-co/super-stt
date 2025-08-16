// SPDX-License-Identifier: GPL-3.0-only
use std::time::{Duration, Instant};

/// A token bucket rate limiter for UDP packet processing
///
/// This implements a standard token bucket algorithm to prevent flooding `DoS` attacks
/// by limiting the rate at which UDP packets can be processed.
pub struct TokenBucketRateLimiter {
    /// Maximum number of tokens the bucket can hold
    capacity: u32,
    /// Current number of tokens in the bucket
    tokens: u32,
    /// Rate at which tokens are replenished (tokens per second)
    refill_rate: u32,
    /// Last time tokens were replenished
    last_refill: Instant,
}

impl TokenBucketRateLimiter {
    /// Create a new rate limiter
    ///
    /// # Arguments
    /// * `capacity` - Maximum tokens the bucket can hold (burst size)
    /// * `refill_rate` - Tokens added per second (sustained rate)
    pub fn new(capacity: u32, refill_rate: u32) -> Self {
        Self {
            capacity,
            tokens: capacity, // Start with full bucket
            refill_rate,
            last_refill: Instant::now(),
        }
    }

    /// Create a rate limiter suitable for audio data processing
    ///
    /// Allows bursts of up to 50 packets with sustained rate of 100 packets/second
    /// This should handle normal audio streaming while preventing flooding
    pub fn for_audio_processing() -> Self {
        Self::new(50, 100)
    }

    /// Try to consume a token for packet processing
    ///
    /// Returns true if a token was available and consumed, false if rate limited
    pub fn try_consume(&mut self) -> bool {
        self.refill_tokens();

        if self.tokens > 0 {
            self.tokens -= 1;
            true
        } else {
            false
        }
    }

    /// Refill tokens based on elapsed time
    fn refill_tokens(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill);

        // Calculate tokens to add based on elapsed time using integer math
        // tokens_to_add = (elapsed_nanos * refill_rate) / 1e9
        let elapsed_nanos = elapsed.as_nanos();
        let tokens_to_add_u128 =
            (elapsed_nanos.saturating_mul(u128::from(self.refill_rate))) / 1_000_000_000u128;
        let tokens_to_add =
            u32::try_from(tokens_to_add_u128.min(u128::from(u32::MAX))).unwrap_or(u32::MAX);

        if tokens_to_add > 0 {
            self.tokens = (self.tokens + tokens_to_add).min(self.capacity);
            self.last_refill = now;
        }
    }

    /// Get the time until the next token becomes available
    ///
    /// Returns None if tokens are immediately available
    pub fn time_until_next_token(&mut self) -> Option<Duration> {
        self.refill_tokens();

        if self.tokens > 0 {
            None
        } else {
            // Calculate when the next token will be available
            let time_per_token = Duration::from_secs_f64(1.0 / f64::from(self.refill_rate));
            Some(time_per_token)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_basic_token_consumption() {
        let mut limiter = TokenBucketRateLimiter::new(5, 10);

        // Should be able to consume up to capacity
        for _ in 0..5 {
            assert!(limiter.try_consume());
        }

        // Should be rate limited after capacity
        assert!(!limiter.try_consume());
    }

    #[test]
    fn test_token_refill() {
        let mut limiter = TokenBucketRateLimiter::new(2, 10);

        // Consume all tokens
        assert!(limiter.try_consume());
        assert!(limiter.try_consume());
        assert!(!limiter.try_consume());

        // Wait for refill (10 tokens/sec = 1 token per 100ms)
        thread::sleep(Duration::from_millis(150));

        // Should have one token available
        assert!(limiter.try_consume());
        assert!(!limiter.try_consume());
    }

    #[test]
    fn test_audio_processing_defaults() {
        let mut limiter = TokenBucketRateLimiter::for_audio_processing();

        // Should handle burst of 50 packets
        for _ in 0..50 {
            assert!(limiter.try_consume());
        }

        // Should be rate limited after burst
        assert!(!limiter.try_consume());
    }
}
