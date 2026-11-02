//! The «Проекты ▾» menu of the header (7.28, mockup 6): the projects opened last (the app keeps
//! them in `settings.json`, newest first; the open one is marked, a folder that is gone is left
//! out) with their pictures from the library's thread, then «Новый проект», «Открыть…»,
//! «Сохранить» and «Сохранить как…». «Сохранить как…» asks a name and copies the project next to
//! it with its recordings (`ProjectDir::save_as`) on its own thread while the window waits; the
//! copy then opens instead (`Leave::Copied`).

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use dc_ui_kit::fonts::{self, Weight};
use dc_ui_kit::icons::Icon;
use dc_ui_kit::strings::projects as text;
use dc_ui_kit::tokens::{ACCENT_HI, BORDER, FS_S, R1, S1, S2, SURFACE_2, TEXT, TEXT_2, TEXT_3};
use dc_ui_kit::widgets::{Answer, Button, Kind, panel_frame};
use eframe::egui::{self, Color32, Rect, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use super::closing::Leave;
use super::{Action, EditorWindow, keys};
use crate::keymap::EditorKey;

/// The projects the menu lists at most (v1, SCOPE).
pub const RECENT_MAX: usize = 16;
/// Width of the menu, its tiles in a row and the height of a tile's name, points (mockup).
const MENU_WIDTH: f32 = 400.0;
const COLUMNS: usize = 3;
const NAME_LINE: f32 = 18.0;
/// Width of the «Сохранить как» question, points.
const DIALOG_WIDTH: f32 = 340.0;

/// The menu's list and «Сохранить как…».
#[derive(Debug, Default)]
pub struct Projects {
    /// Folders of the projects opened last, newest first: the app gives them each frame.
    pub recent: Vec<PathBuf>,
    /// «Сохранить как…» is open with this name.
    pub save_as: Option<String>,
    /// The copy being made: its folder or why it failed.
    copying: Option<Receiver<Result<PathBuf, String>>>,
}

impl Projects {
    /// The window waits for a dialog of the projects: shortcuts wait too.
    pub fn busy(&self) -> bool {
        self.save_as.is_some() || self.copying.is_some()
    }

    /// Waits until the copy is made; the next frame takes it.
    #[cfg(test)]
    pub fn wait_copy(&mut self) {
        if let Some(copying) = self.copying.take() {
            let (done, waited) = channel();
            let copied = copying.recv().unwrap_or_else(|e| Err(e.to_string()));
            done.send(copied).unwrap();
            self.copying = Some(waited);
        }
    }
}

/// «Проекты ▾» and its menu.
pub fn menu(ui: &mut Ui, editor: &mut EditorWindow) {
    let button = Button::new(dc_ui_kit::strings::editor::PROJECTS)
        .kind(Kind::Ghost)
        .small()
        .icon(Icon::Project)
        .menu();
    let response = ui.add(button);
    if response.clicked() {
        let recent = editor.projects.recent.clone();
        editor.library.want_pictures(recent.iter());
    }
    egui::Popup::menu(&response)
        .width(MENU_WIDTH)
        .show(|ui| list(ui, editor));
}

fn list(ui: &mut Ui, editor: &mut EditorWindow) {
    ui.set_width(MENU_WIDTH);
    ui.label(egui::RichText::new(text::RECENT).font(fonts::text(FS_S, Weight::Semibold)));
    let open = editor.doc.dir().root().to_path_buf();
    let recent: Vec<PathBuf> = editor
        .projects
        .recent
        .iter()
        .filter(|root| root.is_dir())
        .cloned()
        .collect();
    if recent.is_empty() {
        ui.label(egui::RichText::new(text::NO_RECENT).color(TEXT_2));
    }
    let width = (MENU_WIDTH - S2 * (COLUMNS as f32 - 1.0)) / COLUMNS as f32;
    for row in recent.chunks(COLUMNS) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = S2;
            for root in row {
                let current = *root == open;
                if tile(ui, editor, root, width, current).clicked() && !current {
                    editor.leave(Leave::Open(root.clone()));
                    ui.close();
                }
            }
        });
    }
    ui.separator();
    let items = [
        (text::NEW, Some(Icon::Plus), EditorKey::NewProject),
        (text::OPEN, Some(Icon::Folder), EditorKey::OpenProject),
        (text::SAVE, None, EditorKey::Save),
        (text::SAVE_AS, None, EditorKey::SaveAs),
    ];
    for (label, icon, key) in items {
        let mut item = Button::new(label).kind(Kind::Ghost).small();
        if let Some(icon) = icon {
            item = item.icon(icon);
        }
        let enabled = key != EditorKey::Save || editor.doc.unsaved();
        let hint = keys::hint(label, key, &editor.keys);
        if ui.add_enabled(enabled, item).on_hover_text(hint).clicked() {
            run(editor, key);
            ui.close();
        }
    }
}

/// A recent project: its picture, its name and «открыт» on the open one.
fn tile(
    ui: &mut Ui,
    editor: &mut EditorWindow,
    root: &std::path::Path,
    width: f32,
    current: bool,
) -> egui::Response {
    let picture_height = width * 9.0 / 16.0;
    let (rect, response) =
        ui.allocate_exact_size(vec2(width, picture_height + S1 + NAME_LINE), Sense::click());
    let name = root
        .file_stem()
        .map_or_else(String::new, |s| s.to_string_lossy().into_owned());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, &name));
    let picture = Rect::from_min_size(rect.min, vec2(width, picture_height));
    let texture = editor.library.picture(ui.ctx(), root);
    let painter = ui.painter();
    painter.rect_filled(picture, R1, SURFACE_2);
    if let Some(texture) = texture {
        let size = texture.size_vec2();
        let scale = (picture.width() / size.x).min(picture.height() / size.y);
        let fitted = Rect::from_center_size(picture.center(), size * scale);
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        painter.image(texture.id(), fitted, uv, Color32::WHITE);
    }
    let (border, stroke) = match (current, response.hovered()) {
        (true, _) => (ACCENT_HI, 2.0),
        (false, true) => (TEXT_3, 1.0),
        (false, false) => (BORDER, 1.0),
    };
    painter.rect_stroke(picture, R1, Stroke::new(stroke, border), StrokeKind::Inside);
    let font = fonts::text(FS_S, Weight::Medium);
    let mut room = width;
    if current {
        let mark = painter.layout_no_wrap(text::CURRENT.to_owned(), font.clone(), ACCENT_HI);
        room -= mark.size().x + S1;
        let mark_at = pos2(picture.right() - mark.size().x, picture.bottom() + S1);
        painter.galley(mark_at, mark, ACCENT_HI);
    }
    let mut job = egui::text::LayoutJob::simple_singleline(name, font, TEXT);
    job.wrap = egui::text::TextWrapping::truncate_at_width(room);
    let label = painter.layout_job(job);
    painter.galley(pos2(rect.left(), picture.bottom() + S1), label, TEXT);
    response
}

/// The action of a projects shortcut or menu item.
pub fn run(editor: &mut EditorWindow, key: EditorKey) {
    match key {
        EditorKey::NewProject => editor.leave(Leave::New),
        EditorKey::OpenProject => {
            let folder = editor
                .doc
                .dir()
                .root()
                .parent()
                .map(std::path::Path::to_path_buf);
            let mut dialog = rfd::FileDialog::new().set_title(text::PICK_PROJECT);
            if let Some(folder) = folder {
                dialog = dialog.set_directory(folder);
            }
            if let Some(root) = dialog.pick_folder() {
                editor.leave(Leave::Open(root));
            }
        }
        EditorKey::Save => editor.ask(Action::Save),
        EditorKey::SaveAs => editor.projects.save_as = Some(text::copy_name(&editor.name)),
        _ => {}
    }
}

/// «Сохранить как»: the name, then the copy on its own thread; the window waits for it.
/// Returns why the copy could not be made.
pub fn save_as(ui: &Ui, editor: &mut EditorWindow) -> Option<String> {
    if let Some(copying) = &editor.projects.copying {
        let done = match copying.try_recv() {
            Ok(done) => done,
            Err(TryRecvError::Empty) => {
                waiting(ui);
                return None;
            }
            Err(TryRecvError::Disconnected) => Err(text::COPYING.to_owned()),
        };
        editor.projects.copying = None;
        return match done {
            Ok(root) => {
                editor.leaving = Some(Leave::Copied(root));
                None
            }
            Err(error) => Some(error),
        };
    }
    let name = editor.projects.save_as.as_mut()?;
    let mut answer = None;
    egui::Modal::new(egui::Id::unique("editor-save-as"))
        .frame(panel_frame())
        .show(ui.ctx(), |ui| {
            ui.set_width(DIALOG_WIDTH);
            answer = name_question(ui, name);
        });
    let answer = answer?;
    let name = editor.projects.save_as.take().unwrap_or_default();
    if answer == Answer::Cancel || name.trim().is_empty() {
        return None;
    }
    start_copy(editor, &name).err()
}

/// Title, text, the name field and «Отмена · Сохранить»; Enter saves, Esc cancels.
fn name_question(ui: &mut Ui, name: &mut String) -> Option<Answer> {
    ui.label(
        egui::RichText::new(text::SAVE_AS_TITLE)
            .font(fonts::text(dc_ui_kit::tokens::FS_M, Weight::Semibold)),
    );
    ui.add_space(S1);
    ui.label(egui::RichText::new(text::SAVE_AS_TEXT).color(TEXT_2));
    ui.add_space(S2);
    let field = egui::TextEdit::singleline(name)
        .desired_width(f32::INFINITY)
        .hint_text(text::NAME);
    let field = ui.add(field);
    field.request_focus();
    ui.add_space(S2);
    let mut answer = None;
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(Button::new(text::SAVE).kind(Kind::Primary).small())
                .clicked()
            {
                answer = Some(Answer::Confirm);
            }
            if ui.add(Button::new(text::CANCEL).small()).clicked() {
                answer = Some(Answer::Cancel);
            }
        });
    });
    ui.input(|i| {
        if i.key_pressed(egui::Key::Escape) {
            answer = Some(Answer::Cancel);
        } else if i.key_pressed(egui::Key::Enter) {
            answer = Some(Answer::Confirm);
        }
    });
    answer
}

/// Starts copying the project's working state to `name` next to it.
fn start_copy(editor: &mut EditorWindow, name: &str) -> Result<(), String> {
    let dir = editor.doc.dir().clone();
    let target = dir.renamed_root(name).map_err(|e| e.to_string())?;
    let project = editor.doc.project().clone();
    let (done, copying) = channel();
    std::thread::Builder::new()
        .name("devcast-save-as".to_owned())
        .spawn(move || {
            let copied = dir
                .save_as(&project, target)
                .map(|copy| copy.root().to_path_buf())
                .map_err(|e| e.to_string());
            // The window is gone when the send fails: the copy stays as it is.
            let _ = done.send(copied);
        })
        .map_err(|e| e.to_string())?;
    editor.projects.copying = Some(copying);
    Ok(())
}

/// «Копирую проект…» with a spinner while the copy is made; the spinner repaints the window
/// until the copy is there.
fn waiting(ui: &Ui) {
    egui::Modal::new(egui::Id::unique("editor-copying"))
        .frame(panel_frame())
        .show(ui.ctx(), |ui| {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(egui::RichText::new(text::COPYING).color(TEXT));
            });
        });
}

#[cfg(test)]
#[path = "projects_tests.rs"]
mod tests;
