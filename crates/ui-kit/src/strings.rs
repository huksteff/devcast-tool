//! Every Russian string of the interface (SCOPE: Russian only). Windows and widgets take their
//! text from here; texts of CLI commands and errors live next to them (docs/QUESTIONS.md).

/// Widgets of the kit.
pub mod kit {
    pub const COPY: &str = "Копировать";
    pub const CLOSE: &str = "Закрыть";
    pub const ADD_COLOR: &str = "Добавить цвет";
    pub const REMOVE_COLOR: &str = "Убрать цвет";
    pub const COLOR: &str = "Цвет";
    pub const HEX_HINT: &str = "#RRGGBB";
    pub const COPIED: &str = "Скопировано";
}
