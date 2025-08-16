// SPDX-License-Identifier: GPL-3.0-only
use std::time::Duration;

/// Connection retry strategy configuration
#[derive(Debug, Clone)]
pub struct RetryStrategy {
    /// Current retry attempt number
    pub attempt: u32,
    /// Initial delay between retries
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Whether to use exponential backoff
    pub use_exponential_backoff: bool,
}

impl Default for RetryStrategy {
    fn default() -> Self {
        Self {
            attempt: 0,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(15), // Cap at 15 seconds between attempts
            use_exponential_backoff: true,
        }
    }
}

impl RetryStrategy {
    /// Create a retry strategy for initial daemon connection
    pub fn for_initial_connection() -> Self {
        Self {
            attempt: 0,
            initial_delay: Duration::from_millis(500), // Start with quick retries
            max_delay: Duration::from_secs(15),        // Cap at 15 seconds
            use_exponential_backoff: true,
        }
    }

    /// Calculate the next retry delay
    pub fn next_delay(&self) -> Duration {
        if !self.use_exponential_backoff {
            return self.initial_delay;
        }

        // Use exponential backoff with jitter
        #[allow(clippy::cast_possible_truncation)]
        let base_delay = self.initial_delay.as_millis() as u64;
        let exponential_delay = base_delay.saturating_mul(2_u64.saturating_pow(self.attempt));
        #[allow(clippy::cast_possible_truncation)]
        let capped_delay = exponential_delay.min(self.max_delay.as_millis() as u64);

        // Add some jitter (±10%) to prevent thundering herd
        let jitter_range = capped_delay / 10;
        #[allow(clippy::cast_possible_truncation)]
        let jitter = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64)
            % (jitter_range * 2);
        let final_delay = capped_delay + jitter - jitter_range;

        Duration::from_millis(final_delay)
    }

    /// Increment the attempt counter and check if we should continue retrying
    /// Always returns true - retries forever with exponential backoff up to `max_delay`
    pub fn should_retry(&mut self) -> bool {
        self.attempt += 1;
        true // Always retry - never give up
    }

    /// Reset the retry strategy
    pub fn reset(&mut self) {
        self.attempt = 0;
    }
}
