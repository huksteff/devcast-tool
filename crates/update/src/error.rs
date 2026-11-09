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
    #[error(
        "не удалось проверить или скачать обновление: {0}. Проверь интернет и повтори «Проверить обновления»"
    )]
    Network(String),
    #[error(
        "обновление не установлено: {0}. Скачай установщик со страницы выпусков и поставь поверх"
    )]
    Velopack(String),
}
