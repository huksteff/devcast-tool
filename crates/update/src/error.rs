#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error(
        "контакты обратной связи не читаются: {0}. Исправь crates/update/feedback.json и собери программу заново"
    )]
    Feedback(serde_json::Error),
    #[error(
        "версия «{0}» не вида 1.2.3: поправь заголовок раздела CHANGELOG.md или version в Cargo.toml"
    )]
    Version(String),
}
