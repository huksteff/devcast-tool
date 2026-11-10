//! The schedule of checks and questions; Velopack itself is checked on an installation
//! (`devcast app update`).

use super::*;

#[test]
fn checks_every_six_hours_and_at_once_the_first_time() {
    assert_eq!(until_check(1_000, None, None), 0);
    assert_eq!(until_check(1_000, Some(1_000), None), CHECK_EVERY);
    assert_eq!(until_check(1_000 + CHECK_EVERY - 5, Some(1_000), None), 5);
    assert_eq!(until_check(1_000 + CHECK_EVERY + 5, Some(1_000), None), 0);
    assert_eq!(until_check(1_000, Some(5_000), None), 0, "clock set back");
    assert_eq!(until_check(1_000, None, Some(1_000)), RETRY_AFTER);
    assert_eq!(until_check(1_000 + RETRY_AFTER, None, Some(1_000)), 0);
    assert_eq!(
        until_check(2_000, Some(1_000), Some(1_990)),
        CHECK_EVERY - 1_000,
        "the longer wait"
    );
}

#[test]
fn asks_unless_skipped_or_put_off() {
    assert!(ask("1.2.0", None, None, 10));
    assert!(!ask("1.2.0", Some("1.2.0"), None, 10));
    assert!(
        ask("1.3.0", Some("1.2.0"), None, 10),
        "a newer one is asked about"
    );
    assert!(!ask("1.2.0", None, Some(10 + SNOOZE), 10));
    assert!(ask("1.2.0", None, Some(10 + SNOOZE), 10 + SNOOZE));
}

#[test]
fn betas_have_their_own_channel() {
    assert_eq!(channel(false), "win");
    assert_eq!(channel(true), "beta");
}

#[test]
fn a_build_of_the_repository_has_no_updater() {
    let updater = Updater::installed("https://github.com/huksteff/devcast-tool", false);
    assert!(matches!(updater, Ok(None)));
}
