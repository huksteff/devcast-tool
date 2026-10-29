//! The one-line answer of `captions style`.

use dc_render::Rgba;

use crate::style::CaptionsSettings;

/// `субтитры: размер 30→36, строк 1→2, анимация fade→pop`.
pub(crate) fn changes(old: &CaptionsSettings, new: &CaptionsSettings) -> String {
    let number = |name: &str, a: f64, b: f64| (a != b).then(|| format!("{name} {a}→{b}"));
    let color = |name: &str, a: Rgba, b: Rgba| (a != b).then(|| format!("{name} {a}→{b}"));
    let switch = |name: &str, a: bool, b: bool| {
        (a != b).then(|| format!("{name} {}", if b { "вкл" } else { "выкл" }))
    };
    let parts = [
        (old.show != new.show).then(|| {
            if new.show {
                "показаны"
            } else {
                "скрыты"
            }
            .to_owned()
        }),
        (old.font != new.font).then(|| format!("шрифт {}→{}", old.font, new.font)),
        number("размер", old.size, new.size),
        color("цвет", old.color, new.color),
        color("подсветка", old.highlight, new.highlight),
        color("несказанные", old.inactive, new.inactive),
        switch("тень", old.shadow, new.shadow),
        (old.rows != new.rows).then(|| format!("строк {}→{}", old.rows, new.rows)),
        number("отступ снизу (%)", old.bottom, new.bottom),
        number("ширина (%)", old.width, new.width),
        number("радиус", old.radius, new.radius),
        number("плашка", old.background, new.background),
        (old.animation != new.animation)
            .then(|| format!("анимация {}→{}", old.animation.name(), new.animation.name())),
    ];
    let parts: Vec<_> = parts.into_iter().flatten().collect();
    format!("субтитры: {}", parts.join(", "))
}
