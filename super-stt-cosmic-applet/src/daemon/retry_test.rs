// SPDX-License-Identifier: GPL-3.0-only
#[cfg(test)]
mod tests {
    use super::super::retry::RetryStrategy;
    use std::time::Duration;

    #[test]
    fn test_initial_connection_strategy() {
        let strategy = RetryStrategy::for_initial_connection();
        assert_eq!(strategy.attempt, 0);
        assert_eq!(strategy.initial_delay, Duration::from_millis(500));
        assert_eq!(strategy.max_delay, Duration::from_secs(15));
        assert!(strategy.use_exponential_backoff);
    }

    #[test]
    fn test_exponential_backoff() {
        let mut strategy = RetryStrategy::for_initial_connection();

        // First attempt should have short delay
        let first_delay = strategy.next_delay();
        assert!(first_delay >= Duration::from_millis(450)); // With jitter
        assert!(first_delay <= Duration::from_millis(550));

        // Increment attempts and check delays increase
        strategy.should_retry();
        let second_delay = strategy.next_delay();
        assert!(second_delay > first_delay);

        // After many attempts, should cap at max_delay
        for _ in 0..10 {
            strategy.should_retry();
        }
        let late_delay = strategy.next_delay();
        // Max delay is 15s, with up to 20% jitter = max 18s, be generous with 20s
        assert!(
            late_delay <= Duration::from_secs(20),
            "Delay was {:?}",
            late_delay
        );
    }

    #[test]
    fn test_infinite_retries() {
        let mut strategy = RetryStrategy::for_initial_connection();

        // Should allow retries forever - test a reasonable number
        for _ in 0..100 {
            assert!(strategy.should_retry());
        }

        // Should still allow retries after many attempts
        assert!(strategy.should_retry());
    }

    #[test]
    fn test_reset() {
        let mut strategy = RetryStrategy::for_initial_connection();

        // Increment attempts
        for _ in 0..5 {
            strategy.should_retry();
        }
        assert_eq!(strategy.attempt, 5);

        // Reset should clear attempts
        strategy.reset();
        assert_eq!(strategy.attempt, 0);
        assert!(strategy.should_retry());
    }
}
