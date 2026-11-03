//! The one-line answer of `captions style`.

use dc_core::Changes;

use crate::style::CaptionsSettings;

/// `субтитры: размер 30→36, строк 1→2, анимация fade→pop`.
pub(crate) fn changes(old: &CaptionsSettings, new: &CaptionsSettings) -> String {
    let mut changes = Changes::default();
    changes
        .part(old.show != new.show, || {
            if new.show {
                "показаны"
            } else {
                "скрыты"
            }
            .to_owned()
        })
        .value("шрифт", &old.font, &new.font)
        .value("размер", old.size, new.size)
        .value("цвет", old.color, new.color)
        .value("подсветка", old.highlight, new.highlight)
        .value("несказанные", old.inactive, new.inactive)
        .switch("тень", old.shadow, new.shadow)
        .value("строк", old.rows, new.rows)
        .value("отступ снизу (%)", old.bottom, new.bottom)
        .value("ширина (%)", old.width, new.width)
        .value("радиус", old.radius, new.radius)
        .value("плашка", old.background, new.background)
        .value("анимация", old.animation.name(), new.animation.name())
        .value("порог тишины", old.silence.name(), new.silence.name());
    format!("субтитры: {}", changes.join())
}
