use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub(super) struct ConfigurePacingStats {
    pub offers: u64,
    pub emitted: u64,
    pub duplicates: u64,
    pub coalesced: u64,
    pub client_blocked: u64,
    pub timeout_emitted: u64,
}

/// Keeps interactive resize configure traffic at or below the output refresh
/// rate while retaining the newest target for the next opportunity.
pub(super) struct ConfigurePacer<T> {
    interval: Duration,
    last_emitted: Option<T>,
    pending: Option<T>,
    last_emit_at: Option<Instant>,
    stats: ConfigurePacingStats,
}

const MAX_IN_FLIGHT_INTERVALS: u32 = 4;

impl<T: Copy + Eq> ConfigurePacer<T> {
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            last_emitted: None,
            pending: None,
            last_emit_at: None,
            stats: ConfigurePacingStats::default(),
        }
    }

    pub fn offer(&mut self, target: T, now: Instant, client_ready: bool) -> Option<T> {
        self.stats.offers += 1;
        if self.pending == Some(target)
            || (self.pending.is_none() && self.last_emitted == Some(target))
        {
            self.stats.duplicates += 1;
            return None;
        }

        if self.pending.replace(target).is_some() {
            self.stats.coalesced += 1;
        }
        let Some(last_emit_at) = self.last_emit_at else {
            return self.emit_pending(now);
        };
        let elapsed = now.saturating_duration_since(last_emit_at);
        if elapsed < self.interval {
            return None;
        }
        if client_ready {
            return self.emit_pending(now);
        }
        self.stats.client_blocked += 1;
        if elapsed >= self.interval.saturating_mul(MAX_IN_FLIGHT_INTERVALS) {
            self.stats.timeout_emitted += 1;
            return self.emit_pending(now);
        }
        None
    }

    pub fn flush(&mut self, now: Instant) -> Option<T> {
        self.emit_pending(now)
    }

    pub fn interval(&self) -> Duration {
        self.interval
    }

    pub fn stats(&self) -> ConfigurePacingStats {
        self.stats
    }

    fn emit_pending(&mut self, now: Instant) -> Option<T> {
        let target = self.pending.take()?;
        self.last_emitted = Some(target);
        self.last_emit_at = Some(now);
        self.stats.emitted += 1;
        Some(target)
    }
}

#[cfg(test)]
mod tests {
    use super::ConfigurePacer;
    use std::time::{Duration, Instant};

    #[test]
    fn emits_first_target_immediately_and_drops_duplicates() {
        let start = Instant::now();
        let mut pacer = ConfigurePacer::new(Duration::from_millis(17));

        assert_eq!(pacer.offer(10, start, false), Some(10));
        assert_eq!(
            pacer.offer(10, start + Duration::from_millis(1), false),
            None
        );
        assert_eq!(pacer.stats().emitted, 1);
        assert_eq!(pacer.stats().duplicates, 1);
    }

    #[test]
    fn coalesces_to_newest_target_until_interval_elapses() {
        let start = Instant::now();
        let mut pacer = ConfigurePacer::new(Duration::from_millis(17));

        assert_eq!(pacer.offer(10, start, false), Some(10));
        assert_eq!(
            pacer.offer(11, start + Duration::from_millis(2), false),
            None
        );
        assert_eq!(
            pacer.offer(12, start + Duration::from_millis(4), false),
            None
        );
        assert_eq!(
            pacer.offer(13, start + Duration::from_millis(17), true),
            Some(13)
        );
        assert_eq!(pacer.stats().emitted, 2);
        assert_eq!(pacer.stats().coalesced, 2);
    }

    #[test]
    fn flush_delivers_last_pending_target_once() {
        let start = Instant::now();
        let mut pacer = ConfigurePacer::new(Duration::from_millis(17));

        assert_eq!(pacer.offer(10, start, false), Some(10));
        assert_eq!(
            pacer.offer(20, start + Duration::from_millis(2), false),
            None
        );
        assert_eq!(pacer.flush(start + Duration::from_millis(3)), Some(20));
        assert_eq!(pacer.flush(start + Duration::from_millis(4)), None);
    }

    #[test]
    fn waits_for_client_commit_but_has_a_bounded_timeout() {
        let start = Instant::now();
        let interval = Duration::from_millis(10);
        let mut pacer = ConfigurePacer::new(interval);

        assert_eq!(pacer.offer(10, start, false), Some(10));
        assert_eq!(pacer.offer(20, start + interval, false), None);
        assert_eq!(pacer.offer(30, start + interval * 3, false), None);
        assert_eq!(pacer.offer(40, start + interval * 4, false), Some(40));
        assert_eq!(pacer.stats().client_blocked, 3);
        assert_eq!(pacer.stats().timeout_emitted, 1);
    }
}
