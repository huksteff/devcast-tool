//! The sound panels of the inspector: the «Звук» tab, the sound of a picked clip, an extra
//! sound clip.

/// Subtitle of the «Звук» tab.
pub const DEFAULTS_FOR_ALL: &str = "по умолчанию для всех клипов";
pub const MIC: &str = "Микрофон";
pub const SYSTEM: &str = "Системный звук";
pub const VOLUME: &str = "Громкость";
pub const GAIN: &str = "Усиление";
pub const NORMALIZE: &str = "Нормализация";
/// The target of the normalization after its switch.
pub const LUFS: &str = "−16 LUFS";
pub const DENOISE: &str = "Подавить шум";
pub const UNCHANGED_NOTE: &str =
    "Нормализация и подавление шума не меняют файлы записи — только звук проекта.";
pub const NO_SOUND: &str = "В записях проекта нет звука микрофона и системы.";
pub const EXTRA_NOTE: &str =
    "Громкость дополнительного звука — у каждого клипа: выберите его на дорожке «Аудио».";

/// «Заглушить «Микрофон»»: the mute button in the head of a track's group.
pub fn mute(track: &str) -> String {
    format!("Заглушить «{track}»")
}

/// «Включить звук «Микрофон»»: the same button while the track is muted.
pub fn unmute(track: &str) -> String {
    format!("Включить звук «{track}»")
}

pub const CLIP_SOUND: &str = "Звук клипа";
pub const WHOLE_CLIP: &str = "Весь клип";
pub const SOUND_ON: &str = "Звук";
pub const SOUND_OFF: &str = "Без звука";
/// The switch that silences one track of the clip.
pub const SILENT: &str = "Без звука";
pub const FOLLOWS_NOTE: &str = "Пока параметр не менялся, клип берёт значение дорожки.";
pub const IMPORTED_NOTE: &str = "Дорожки импортированного видео (audio-1, audio-2 …) настраиваются командой devcast audio channel.";

/// «Микрофон · как у дорожки»: a track of the clip that follows the defaults.
pub fn follows(track: &str) -> String {
    format!("{track} · как у дорожки")
}

/// «Микрофон · свои: громкость, усиление»: a track of the clip with levels of its own.
pub fn own(track: &str, fields: &[&str]) -> String {
    format!("{track} · свои: {}", fields.join(", "))
}

/// The fields of [`own`].
pub const OWN_VOLUME: &str = "громкость";
pub const OWN_GAIN: &str = "усиление";
pub const OWN_MUTE: &str = "mute";
pub const OWN_NORMALIZE: &str = "нормализация";
pub const OWN_DENOISE: &str = "шумоподавление";

pub const FILE: &str = "Файл";
pub const PLACE: &str = "Место";
pub const START: &str = "Начало";
pub const OFFSET: &str = "Сдвиг в файле";
pub const LENGTH: &str = "Длина";
pub const EXTRA_PLACE_NOTE: &str =
    "Звук идёт по времени готового видео: вырезы и скорость его не сдвигают.";
