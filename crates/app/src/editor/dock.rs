//! The height of the docked timeline. It is the user's: only dragging the panel's edge changes
//! and saves it. A window made smaller squeezes the panel for the time being, and the height
//! comes back with the room (before, the squeezed height stayed for good).

use eframe::egui::{IdSalt, PanelState, Ui};

/// The id of the timeline panel in the editor window.
pub const PANEL: &str = "editor-timeline";

pub struct Dock {
    /// The remembered height, fitted to the window of this frame.
    pub wanted: f32,
    /// The pointer is held or was just let go in the window: the panel may be dragged.
    by_hand: bool,
}

impl Dock {
    /// Before the panel is shown: when the room for it changed (`most`, the tallest it may
    /// be — the window was resized), the panel takes `wanted` again.
    pub fn new(ui: &Ui, wanted: f32, most: f32) -> Self {
        let by_hand = ui.input(|i| i.pointer.any_down() || i.pointer.any_released());
        let id = ui.scope_id().with_salt(IdSalt::new(PANEL));
        let room = id.with("room");
        let was = ui.ctx().data_mut(|d| d.get_temp::<f32>(room));
        ui.ctx().data_mut(|d| d.insert_temp(room, most));
        if was.is_some_and(|was| (was - most).abs() >= 0.5)
            && let Some(mut state) = PanelState::load(ui.ctx(), id)
        {
            state.outer_rect.set_height(wanted);
            ui.ctx().data_mut(|d| d.insert_persisted(id, state));
        }
        Self { wanted, by_hand }
    }

    /// The height to remember: the `shown` one when the user dragged, else the `kept` one.
    pub fn height(&self, shown: f32, kept: f32) -> f32 {
        match self.by_hand {
            true => shown,
            false => kept,
        }
    }
}
