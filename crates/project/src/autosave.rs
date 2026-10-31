//! When to write `session.json` and whether there is unsaved work. Pure state: the caller passes
//! the current time and does the writing, so this is testable without clocks or files.
//!
//! Debounce: the session is written `delay` after the last change, but at most `max_wait` after
//! the first unsaved one, so continuous editing still gets saved.

use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct Autosave {
    delay: Duration,
    max_wait: Duration,
    /// First and last change not yet written to `session.json`.
    pending: Option<(Instant, Instant)>,
    /// The working state differs from `project.json`.
    unsaved: bool,
    /// Writing failed: no new try before this moment (the caller shows the error each try).
    retry: Option<Instant>,
}

impl Autosave {
    pub const DEFAULT_DELAY: Duration = Duration::from_secs(2);
    pub const DEFAULT_MAX_WAIT: Duration = Duration::from_secs(30);

    pub fn new(delay: Duration, max_wait: Duration) -> Self {
        Self {
            delay,
            max_wait,
            pending: None,
            unsaved: false,
            retry: None,
        }
    }

    /// Opening a project that has a `session.json` starts in the unsaved state.
    pub fn restored(mut self, unsaved: bool) -> Self {
        self.unsaved = unsaved;
        self
    }

    /// The project changed (a command, undo or redo).
    pub fn changed(&mut self, now: Instant) {
        self.unsaved = true;
        let first = self.pending.map_or(now, |(first, _)| first);
        self.pending = Some((first, now));
    }

    /// Whether `session.json` should be written now.
    pub fn due(&self, now: Instant) -> bool {
        if self.retry.is_some_and(|retry| now < retry) {
            return false;
        }
        self.pending.is_some_and(|(first, last)| {
            now.saturating_duration_since(last) >= self.delay
                || now.saturating_duration_since(first) >= self.max_wait
        })
    }

    /// How long until [`Self::due`] becomes true, to schedule a repaint; `None` when nothing waits.
    pub fn wait(&self, now: Instant) -> Option<Duration> {
        self.pending.map(|(first, last)| {
            let by_delay = (last + self.delay).saturating_duration_since(now);
            let by_max = (first + self.max_wait).saturating_duration_since(now);
            let by_retry = self
                .retry
                .map_or(Duration::ZERO, |r| r.saturating_duration_since(now));
            by_delay.min(by_max).max(by_retry)
        })
    }

    /// Writing `session.json` failed at `now`: the next try waits `max_wait`, so a broken disk
    /// is not retried and reported on every frame.
    pub fn failed(&mut self, now: Instant) {
        self.retry = Some(now + self.max_wait);
    }

    pub fn session_written(&mut self) {
        self.pending = None;
        self.retry = None;
    }

    /// Ctrl+S or «Не сохранять»: the working state equals `project.json` again.
    pub fn saved(&mut self) {
        self.pending = None;
        self.retry = None;
        self.unsaved = false;
    }

    pub fn has_unsaved(&self) -> bool {
        self.unsaved
    }
}

impl Default for Autosave {
    fn default() -> Self {
        Self::new(Self::DEFAULT_DELAY, Self::DEFAULT_MAX_WAIT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn secs(s: u64) -> Duration {
        Duration::from_secs(s)
    }

    #[test]
    fn waits_for_a_pause_after_the_last_change() {
        let t0 = Instant::now();
        let mut a = Autosave::new(secs(2), secs(30));
        assert!(!a.due(t0) && a.wait(t0).is_none() && !a.has_unsaved());
        a.changed(t0);
        a.changed(t0 + secs(1));
        assert!(!a.due(t0 + secs(2)));
        assert_eq!(a.wait(t0 + secs(2)), Some(secs(1)));
        assert!(a.due(t0 + secs(3)));
        a.session_written();
        assert!(!a.due(t0 + secs(10)) && a.has_unsaved());
    }

    #[test]
    fn continuous_editing_is_saved_after_max_wait() {
        let t0 = Instant::now();
        let mut a = Autosave::new(secs(2), secs(5));
        for s in 0..6 {
            a.changed(t0 + secs(s));
        }
        assert!(a.due(t0 + secs(5)));
        assert_eq!(a.wait(t0 + secs(5)), Some(Duration::ZERO));
    }

    #[test]
    fn a_failed_write_waits_max_wait_before_the_next_try() {
        let t0 = Instant::now();
        let mut a = Autosave::new(secs(2), secs(30));
        a.changed(t0);
        assert!(a.due(t0 + secs(2)));
        a.failed(t0 + secs(2));
        assert!(!a.due(t0 + secs(3)) && !a.due(t0 + secs(31)));
        assert_eq!(a.wait(t0 + secs(3)), Some(secs(29)));
        assert!(a.due(t0 + secs(32)));
        a.session_written();
        a.changed(t0 + secs(40));
        assert!(a.due(t0 + secs(42)));
    }

    #[test]
    fn save_clears_unsaved_and_restore_sets_it() {
        let t0 = Instant::now();
        let mut a = Autosave::default();
        a.changed(t0);
        a.saved();
        assert!(!a.has_unsaved() && !a.due(t0 + secs(60)));
        assert!(Autosave::default().restored(true).has_unsaved());
    }
}
