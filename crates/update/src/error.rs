#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error(
        "контакты обратной связи не читаются: {0}. Исправь crates/update/feedback.json и собери программу заново"
    )]
    Feedback(serde_json::Error),
}
