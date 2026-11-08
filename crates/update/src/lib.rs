//! Velopack updates, What's new window data, announcements, feedback contacts.

mod changelog;
mod error;
mod feedback;
mod updates;

pub use changelog::{Group, Release, news};
pub use error::UpdateError;
pub use feedback::{Author, Contact, Feedback, feedback};
pub use updates::{CHECK_EVERY, SNOOZE, Update, Updater, ask, until_check};

/// Handles the Velopack installer hooks (`--veloapp-install`, `--veloapp-uninstall`, …): during
/// a hook the process does its part and exits here. Must be the first call of `main` of the
/// installed exe (decision 013).
pub fn handle_installer_hooks() {
    velopack::VelopackApp::build().run();
}
