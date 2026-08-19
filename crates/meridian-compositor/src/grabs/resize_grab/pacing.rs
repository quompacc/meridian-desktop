use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub(super) struct ConfigurePacingStats {
    pub offers: u64,
    pub emitted: u64,
    pub duplicates: u64,
    pub coalesced: u64,
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

    pub fn offer(&mut self, target: T, now: Instant) -> Option<T> {
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
        let due = self.last_emit_at.map_or(true, |last| {
            now.saturating_duration_since(last) >= self.interval
        });
        due.then(|| self.emit_pending(now)).flatten()
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

        assert_eq!(pacer.offer(10, start), Some(10));
        assert_eq!(pacer.offer(10, start + Duration::from_millis(1)), None);
        assert_eq!(pacer.stats().emitted, 1);
        assert_eq!(pacer.stats().duplicates, 1);
    }

    #[test]
    fn coalesces_to_newest_target_until_interval_elapses() {
        let start = Instant::now();
        let mut pacer = ConfigurePacer::new(Duration::from_millis(17));

        assert_eq!(pacer.offer(10, start), Some(10));
        assert_eq!(pacer.offer(11, start + Duration::from_millis(2)), None);
        assert_eq!(pacer.offer(12, start + Duration::from_millis(4)), None);
        assert_eq!(pacer.offer(13, start + Duration::from_millis(17)), Some(13));
        assert_eq!(pacer.stats().emitted, 2);
        assert_eq!(pacer.stats().coalesced, 2);
    }

    #[test]
    fn flush_delivers_last_pending_target_once() {
        let start = Instant::now();
        let mut pacer = ConfigurePacer::new(Duration::from_millis(17));

        assert_eq!(pacer.offer(10, start), Some(10));
        assert_eq!(pacer.offer(20, start + Duration::from_millis(2)), None);
        assert_eq!(pacer.flush(start + Duration::from_millis(3)), Some(20));
        assert_eq!(pacer.flush(start + Duration::from_millis(4)), None);
    }
}
