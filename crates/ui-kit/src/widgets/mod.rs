//! Widgets of the mockup. Each draws itself from the tokens and has an accessible name, so
//! kittest and egui-mcp find it by its caption.

mod anchors;
mod button;
mod color;
mod dialog;
mod dropdown;
mod icon_button;
mod meter;
mod section;
mod segmented;
mod slider;
mod text_field;
mod tiles;
mod toast;
mod toggle;

pub use anchors::anchors;
pub use button::{Button, Kind};
pub use color::{GRADIENT_COLORS, color_button, gradient_editor, palette};
pub use dialog::{Answer, Confirm, ErrorAction, ErrorPanel, panel_frame};
pub use dropdown::{dropdown, dropdown_some};
pub use icon_button::IconButton;
pub use meter::LevelBar;
pub use section::Section;
pub use segmented::segmented;
pub use slider::Slider;
pub use text_field::{search_field, text_area, text_field};
pub use tiles::tiles;
pub use toast::{ToastKind, Toasts};
pub use toggle::Toggle;
