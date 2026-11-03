//! Velopack updates, What's new window data, announcements, feedback contacts.

mod changelog;
mod error;
mod feedback;

pub use changelog::{Group, Release, news};
pub use error::UpdateError;
pub use feedback::{Contact, Feedback, feedback};

/// Handles the Velopack installer hooks (`--veloapp-install`, `--veloapp-uninstall`, …): during
/// a hook the process does its part and exits here. Must be the first call of `main` of the
/// installed exe (decision 013). Checks for updates stay off while the repository is private.
pub fn handle_installer_hooks() {
    velopack::VelopackApp::build().run();
}
