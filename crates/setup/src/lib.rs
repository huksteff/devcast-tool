//! `DevCast-Setup.exe`: Velopack installs only without questions (decision 013), so this small
//! program asks where — «Установить» into the usual `%LOCALAPPDATA%\DevCast`, or «Другая
//! папка…» — and runs the Velopack installer it carries with `--installto` (owner's decision
//! 2026-11-04). Updates go to the same folder. System dialogs of Windows, no window of its own.

use std::path::{Path, PathBuf};
use std::process::Command;

use dc_ui_kit::strings::setup as text;
use rfd::{MessageButtons, MessageDialog, MessageDialogResult, MessageLevel};

/// The Velopack installer `cargo xtask package` built into this program; empty in other builds.
const VELOPACK_SETUP: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/velopack-setup.exe"));

/// The folder of the app inside the folder the user picks.
const APP_FOLDER: &str = "DevCast";

/// What the user answered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    Install(PathBuf),
    Cancel,
}

/// Asks where, installs there; `Ok(false)` — the user cancelled.
pub fn run() -> Result<bool, String> {
    if VELOPACK_SETUP.is_empty() {
        return Err(text::NOT_PACKED.to_owned());
    }
    let Choice::Install(folder) = ask(&default_folder())? else {
        return Ok(false);
    };
    install(&folder)?;
    Ok(true)
}

/// Shows `error` in a system dialog.
pub fn tell(error: &str) {
    MessageDialog::new()
        .set_level(MessageLevel::Error)
        .set_title(text::TITLE)
        .set_description(error)
        .set_buttons(MessageButtons::Ok)
        .show();
}

/// Where Velopack installs by itself: `%LOCALAPPDATA%\DevCast`.
fn default_folder() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join(APP_FOLDER)
}

/// «Установить в …?» until the user installs or cancels; a closed folder picker asks again.
fn ask(default: &Path) -> Result<Choice, String> {
    loop {
        let answer = MessageDialog::new()
            .set_level(MessageLevel::Info)
            .set_title(text::TITLE)
            .set_description(text::question(default))
            .set_buttons(MessageButtons::YesNoCancelCustom(
                text::INSTALL.to_owned(),
                text::OTHER_FOLDER.to_owned(),
                text::CANCEL.to_owned(),
            ))
            .show();
        match answer {
            MessageDialogResult::Custom(pick) if pick == text::INSTALL => {
                return Ok(Choice::Install(default.to_path_buf()));
            }
            MessageDialogResult::Custom(pick) if pick == text::OTHER_FOLDER => {
                let picked = rfd::FileDialog::new()
                    .set_title(text::PICK_FOLDER)
                    .pick_folder();
                if let Some(picked) = picked {
                    return Ok(Choice::Install(app_folder(&picked)));
                }
            }
            _ => return Ok(Choice::Cancel),
        }
    }
}

/// The app's own folder in the picked one: `D:\Программы` → `D:\Программы\DevCast`; a picked
/// `…\DevCast` stays as it is.
pub fn app_folder(picked: &Path) -> PathBuf {
    let named = picked
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case(APP_FOLDER));
    if named {
        picked.to_path_buf()
    } else {
        picked.join(APP_FOLDER)
    }
}

/// The arguments of the Velopack installer for `folder`.
pub fn setup_args(folder: &Path) -> Vec<std::ffi::OsString> {
    vec!["--installto".into(), folder.as_os_str().to_owned()]
}

/// Writes the carried installer to a temp folder and runs it for `folder`; it shows its own
/// progress and starts DevCast when done.
fn install(folder: &Path) -> Result<(), String> {
    let failed = |e: &dyn std::fmt::Display| text::failed(e);
    let temp = std::env::temp_dir().join(format!("devcast-setup-{}", std::process::id()));
    std::fs::create_dir_all(&temp).map_err(|e| failed(&e))?;
    let setup = temp.join("DevCast-win-Setup.exe");
    std::fs::write(&setup, VELOPACK_SETUP).map_err(|e| failed(&e))?;
    let status = Command::new(&setup).args(setup_args(folder)).status();
    // The copy is only a carrier: what the user keeps is the installed folder.
    let _ = std::fs::remove_dir_all(&temp);
    match status {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => Err(failed(&status)),
        Err(error) => Err(failed(&error)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_app_gets_its_own_folder_in_the_picked_one() {
        let picked = Path::new(r"D:\Мои программы");
        assert_eq!(app_folder(picked), Path::new(r"D:\Мои программы\DevCast"));
        let own = Path::new(r"D:\Мои программы\devcast");
        assert_eq!(
            app_folder(own),
            own,
            "a folder named DevCast is the app's own"
        );
        let args = setup_args(Path::new(r"D:\Мои программы\DevCast"));
        assert_eq!(args[0], "--installto");
        assert_eq!(args[1], r"D:\Мои программы\DevCast");
    }

    #[test]
    fn the_usual_folder_is_the_one_velopack_installs_to() {
        assert!(default_folder().ends_with("DevCast"));
    }
}
