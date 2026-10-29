use std::path::PathBuf;

use dc_core::SourceTime;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GpuError {
    #[error(
        "нет ни видеокарты, ни программного адаптера (WARP в Windows, lavapipe в Linux): {0}. Обнови драйвер видеокарты"
    )]
    NoAdapter(wgpu::RequestAdapterError),
    #[error("видеокарта «{adapter}» не создала устройство: {source}. Обнови драйвер видеокарты")]
    Device {
        adapter: String,
        source: wgpu::RequestDeviceError,
    },
    #[error("ожидание видеокарты прервалось: {0}. Устройство потеряно, перезапусти программу")]
    Poll(#[from] wgpu::PollError),
    #[error("кадр не прочитался из видеопамяти: {0}. Устройство потеряно, перезапусти программу")]
    Map(#[from] wgpu::BufferAsyncError),
    #[error("кадр не прочитался из видеопамяти: {0}. Устройство потеряно, перезапусти программу")]
    MapRange(#[from] wgpu::MapRangeError),
}

/// A scene that cannot be drawn with what the caller gave the compositor.
#[derive(Debug, Error)]
pub enum RenderError {
    #[error(
        "кадр «{media}» на {time} не загружен в рендер перед рисованием сцены. Это ошибка программы: сообщи разработчику"
    )]
    NoFrame { media: String, time: SourceTime },
    #[error(
        "кадр NV12 {width}×{height} не той длины: Y {y} байт, UV {uv} байт. Это ошибка программы: сообщи разработчику"
    )]
    Nv12 {
        width: u32,
        height: u32,
        y: usize,
        uv: usize,
    },
    #[error("шрифты из «{}» не загрузились: {source}. Проверь, что папка есть и файлы .ttf/.otf целые", path.display())]
    Fonts {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error(
        "шрифт «{family}» не загружен. Выбери встроенный шрифт (Inter, Montserrat, Roboto, PT Serif, JetBrains Mono) или положи файл шрифта в assets/ проекта"
    )]
    NoFont { family: String },
    #[error("текст не подготовился к рисованию: {0}. Уменьши размер или количество текста в кадре")]
    Text(glyphon::PrepareError),
    #[error("текст не нарисовался: {0}. Это ошибка программы: сообщи разработчику")]
    TextDraw(glyphon::RenderError),
}
