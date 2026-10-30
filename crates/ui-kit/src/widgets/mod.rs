//! Widgets of the mockup. Each draws itself from the tokens and has an accessible name, so
//! kittest and egui-mcp find it by its caption.

mod button;
mod dropdown;
mod icon_button;
mod segmented;
mod slider;
mod text_field;
mod toggle;

pub use button::{Button, Kind};
pub use dropdown::dropdown;
pub use icon_button::IconButton;
pub use segmented::segmented;
pub use slider::Slider;
pub use text_field::text_field;
pub use toggle::Toggle;
