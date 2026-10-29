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
}
