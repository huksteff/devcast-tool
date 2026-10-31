//! Every literal of `src/strings.rs` and of `src/strings/` (keys, inspector, captions,
//! annotations, sound) is drawn with our fonts: kittest fails on a letter the fonts lack, so a new
//! string with such a letter fails here.

/// The text inside the double quotes of each line that has some.
fn literals(source: &str) -> Vec<String> {
    source
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .flat_map(|line| line.split('"').skip(1).step_by(2).map(str::to_owned))
        .filter(|text| !text.is_empty())
        .collect()
}

#[test]
fn every_string_has_glyphs() {
    let mut texts = literals(include_str!("../src/strings.rs"));
    texts.extend(literals(include_str!("../src/strings/keys.rs")));
    texts.extend(literals(include_str!("../src/strings/inspector.rs")));
    texts.extend(literals(include_str!("../src/strings/captions.rs")));
    texts.extend(literals(include_str!("../src/strings/annotations.rs")));
    texts.extend(literals(include_str!("../src/strings/sound.rs")));
    assert!(texts.iter().any(|t| t == "Горячие клавиши"));
    assert!(texts.len() > 80, "strings found: {}", texts.len());
    assert!(texts.iter().any(|t| t == "Отменить запись?"));
    let mut harness = dc_ui_kit::testing::harness()
        .with_size(egui::vec2(1200.0, 2400.0))
        .build_ui(move |ui| {
            for text in &texts {
                ui.label(text.as_str());
            }
        });
    harness.run();
    harness
        .render()
        .unwrap_or_else(|e| panic!("strings are not drawn: {e}"));
}
