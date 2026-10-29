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
