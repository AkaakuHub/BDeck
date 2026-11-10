use std::time::Duration;
use tokio::time::{Instant, Interval, MissedTickBehavior};

pub(super) const INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);

pub(super) fn daily() -> Interval {
    let mut interval = tokio::time::interval_at(Instant::now() + INTERVAL, INTERVAL);
    interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
    interval
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn scheduled_checks_start_after_24_hours_and_repeat_daily() {
        let start = Instant::now();
        let mut schedule = daily();
        schedule.tick().await;
        assert_eq!(Instant::now() - start, INTERVAL);
        schedule.tick().await;
        assert_eq!(Instant::now() - start, INTERVAL * 2);
    }

    #[tokio::test(start_paused = true)]
    async fn waking_after_several_days_does_not_burst_missed_checks() {
        let mut schedule = daily();
        tokio::time::advance(INTERVAL * 3 + Duration::from_secs(60)).await;
        schedule.tick().await;
        assert!(
            tokio::time::timeout(Duration::from_secs(1), schedule.tick())
                .await
                .is_err()
        );
        let resumed = Instant::now();
        schedule.tick().await;
        assert_eq!(Instant::now() - resumed, INTERVAL - Duration::from_secs(61));
    }
}
