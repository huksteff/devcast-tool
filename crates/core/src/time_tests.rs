use proptest::prelude::*;

use super::*;

fn s(text: &str) -> SourceTime {
    text.parse().unwrap()
}

#[test]
fn parses_all_formats() {
    assert_eq!(s("12.5"), SourceTime::from_micros(12_500_000));
    assert_eq!(s("1:02.25"), SourceTime::from_micros(62_250_000));
    assert_eq!(s("1:02:03"), SourceTime::from_micros(3_723_000_000));
    assert_eq!(s(" 0 "), SourceTime::ZERO);
    assert_eq!(s("-1.5"), SourceTime::from_micros(-1_500_000));
}

#[test]
fn rejects_garbage_with_hint() {
    for bad in [
        "", "abc", "1:", ":5", "1:60", "1:61:00", "1:2:3:4", "NaN", "inf", "-", "1.5.2",
    ] {
        let error = bad.parse::<SourceTime>().unwrap_err();
        assert!(
            error.to_string().contains("пиши секундами"),
            "{bad}: {error}"
        );
    }
}

#[test]
fn displays_minutes_and_hours() {
    assert_eq!(SourceTime::from_micros(62_250_000).to_string(), "1:02.250");
    assert_eq!(
        SourceTime::from_micros(3_723_000_999).to_string(),
        "1:02:03.000"
    );
    assert_eq!(SourceTime::from_micros(-1_500_000).to_string(), "-0:01.500");
}

#[test]
fn arithmetic_saturates_instead_of_panicking() {
    let max = SourceTime::from_micros(i64::MAX);
    assert_eq!(max + TimeDelta::from_micros(1), max);
    assert_eq!(max.checked_add(TimeDelta::from_micros(1)), None);
    let min = SourceTime::from_micros(i64::MIN);
    assert_eq!(min - TimeDelta::from_micros(1), min);
}

#[test]
fn non_finite_seconds_become_zero() {
    assert_eq!(SourceTime::from_secs_f64(f64::NAN), SourceTime::ZERO);
    assert_eq!(TimeDelta::from_secs_f64(f64::INFINITY), TimeDelta::ZERO);
}

#[test]
fn serializes_as_plain_micros() {
    let json = serde_json::to_string(&SourceTime::from_micros(1500)).unwrap();
    assert_eq!(json, "1500");
    let range = Range::new(s("1"), s("2")).unwrap();
    assert_eq!(
        serde_json::to_string(&range).unwrap(),
        r#"{"start":1000000,"end":2000000}"#
    );
}

#[test]
fn inverted_range_is_rejected_on_load() {
    let error = serde_json::from_str::<Range<Source>>(r#"{"start":5,"end":1}"#).unwrap_err();
    assert!(error.to_string().contains("раньше начала"), "{error}");
    assert!(Range::new(s("2"), s("1")).is_err());
}

#[test]
fn range_contains_is_half_open() {
    let r = Range::new(s("1"), s("2")).unwrap();
    assert!(r.contains(s("1")));
    assert!(!r.contains(s("2")));
    assert_eq!(r.duration(), TimeDelta::from_millis(1000));
}

#[test]
fn intersect_needs_real_overlap() {
    let a = Range::new(s("0"), s("10")).unwrap();
    let b = Range::new(s("5"), s("15")).unwrap();
    let touching = Range::new(s("10"), s("20")).unwrap();
    assert_eq!(a.intersect(&b), Some(Range::new(s("5"), s("10")).unwrap()));
    assert_eq!(a.intersect(&touching), None);
}

proptest! {
    #[test]
    fn display_then_parse_round_trips_milliseconds(ms in -10_000_000_000i64..10_000_000_000) {
        let t = SourceTime::from_micros(ms * 1000);
        prop_assert_eq!(t.to_string().parse::<SourceTime>().unwrap(), t);
    }

    #[test]
    fn add_then_sub_restores(t in -1_000_000_000_000i64..1_000_000_000_000, d in -1_000_000_000i64..1_000_000_000) {
        let time = OutputTime::from_micros(t);
        let delta = TimeDelta::from_micros(d);
        prop_assert_eq!((time + delta) - delta, time);
        prop_assert_eq!((time + delta).since(time), delta);
    }

    #[test]
    fn intersection_is_inside_both(a in 0i64..1000, b in 0i64..1000, c in 0i64..1000, d in 0i64..1000) {
        let r1 = Range::new(SourceTime::from_micros(a.min(b)), SourceTime::from_micros(a.max(b))).unwrap();
        let r2 = Range::new(SourceTime::from_micros(c.min(d)), SourceTime::from_micros(c.max(d))).unwrap();
        if let Some(i) = r1.intersect(&r2) {
            prop_assert!(i.start() >= r1.start() && i.end() <= r1.end());
            prop_assert!(i.start() >= r2.start() && i.end() <= r2.end());
            prop_assert!(!i.is_empty());
        }
    }
}
