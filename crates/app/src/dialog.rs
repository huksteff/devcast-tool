//! System file dialogs of the app.

/// A file dialog owned by the window the user works in: it opens over that window (the editor
/// on the second screen), not in the corner of the main screen.
pub fn file() -> rfd::FileDialog {
    let dialog = rfd::FileDialog::new();
    match dc_capture::win::owner::foreground() {
        Some(owner) => dialog.set_parent(&owner),
        None => dialog,
    }
}
