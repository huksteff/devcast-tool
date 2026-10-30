use std::time::{Duration, Instant};

use super::{Countdown, clock};

#[test]
fn countdown_goes_down_once_a_second() {
    let start = Instant::now();
    let mut countdown = Countdown::new(3, start);
    assert!(!countdown.tick(start + Duration::from_millis(999)));
    assert_eq!(countdown.left, 3);
    assert!(!countdown.tick(start + Duration::from_millis(1000)));
    assert_eq!(countdown.left, 2);
    assert!(countdown.tick(start + Duration::from_millis(3500)));
    assert_eq!(countdown.left, 0);
}

#[test]
fn timer_shows_hours_only_past_an_hour() {
    assert_eq!(clock(0), "00:00");
    assert_eq!(clock(767_400_000), "12:47");
    assert_eq!(clock(3_600_000_000 + 63_000_000), "1:01:03");
}
