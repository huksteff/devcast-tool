//! The crop and output format dialog of the editor.

/// Before the format choice.
pub const FORMAT: &str = "Формат видео";
pub const HINT: &str =
    "Тяните рамку или края · Shift — сохранить пропорцию · двойной клик — весь кадр";
pub const NOTE: &str =
    "Формат задаёт кадр всего видео. Клипы другой формы вписываются с чёрными полями.";
/// «Весь кадр»: the crop frame takes the whole source frame.
pub const RESET: &str = "Сбросить";
pub const RESET_HINT: &str = "Весь кадр";
pub const CANCEL: &str = "Отмена";
pub const APPLY: &str = "Применить";
/// The numeric fields of the crop frame, px of the source.
pub const X: &str = "X";
pub const Y: &str = "Y";
pub const WIDTH: &str = "Ширина";
pub const HEIGHT: &str = "Высота";
/// The picture of the clip and its crop frame.
pub const FRAME: &str = "Кадр клипа";
/// The render thread found no video of the clip at the moment shown.
pub const NO_PICTURE: &str =
    "Нет кадра клипа в этот момент: закрой окно и поставь указатель на клип";

/// «Кроп и формат — Запись 30.10 14:05».
pub fn title(clip: &str) -> String {
    format!("Кроп и формат — {clip}")
}

/// «1640 × 1036 px · 1,58 : 1»: the output frame and its aspect, two decimals.
pub fn output(width: u32, height: u32) -> String {
    let ratio = f64::from(width) / f64::from(height.max(1));
    let ratio = format!("{ratio:.2}").replace('.', ",");
    format!("{width} × {height} px · {ratio} : 1")
}
