//! MP4 export (decision 006): scene → `Compositor` → NV12 on the GPU → ring readback → H.264
//! (the first codec that passes its self-test) and the mixed sound → AAC, both into one file.

use std::ops::ControlFlow;
use std::path::Path;

use dc_render::{Compositor, Gpu, Nv12Readback, Planes};

use crate::audio::AudioEncoder;
use crate::encoder::{VideoEncoder, VideoSettings};
use crate::error::EncodeError;
use crate::frame::Nv12Frame;
use crate::mp4::{Mp4Layout, Mp4Writer};
use crate::packet::Packet;
use crate::select::open_for_export;
use crate::still::Sources;

use super::{
    Exported, Mp4Settings, PartFile, Progress, ProgressFn, Timeline, frame_count, frame_time,
};

const VIDEO: usize = 0;
const SOUND: usize = 1;

/// Exports `timeline` into `out` at `settings`. `progress` hears of every frame and may
/// cancel; a cancelled or failed export leaves no file at `out`.
pub fn export_mp4(
    gpu: &Gpu,
    compositor: &mut Compositor,
    sources: &mut Sources<'_>,
    timeline: &mut Timeline<'_>,
    settings: &Mp4Settings,
    out: &Path,
    progress: &mut ProgressFn<'_>,
) -> Result<Exported, EncodeError> {
    settings.check(timeline.source_fps)?;
    let size = settings.resolution.size(timeline.canvas);
    let fps = settings.fps;
    let video = VideoSettings {
        width: size.width,
        height: size.height,
        fps,
    };
    let encoder = open_for_export(video, settings.quality)?;
    let codec = encoder.codec();
    let part = PartFile::new(out);
    let mut muxer = Muxer::new(encoder, video, timeline, part.path())?;
    let target = gpu.target(size);
    let mut readback = Nv12Readback::new(gpu, &target);
    let total = frame_count(timeline.duration, fps);
    let rate = u64::from(timeline.audio.rate);
    let samples = (timeline.duration.micros().max(0) as u64 * rate).div_ceil(1_000_000);
    let mut sound = Vec::new();
    let mut played = 0;
    for n in 0..total {
        let scene = (timeline.scene)(frame_time(n, fps));
        sources.feed(gpu, compositor, &scene)?;
        compositor.draw(gpu, &scene, &target)?;
        readback.push(gpu, &mut |planes| muxer.video(planes))?;
        let until = ((n + 1) * rate / u64::from(fps)).min(samples);
        sound.resize(
            (until - played) as usize * usize::from(timeline.audio.channels),
            0.0,
        );
        (timeline.sound)(played as i64, &mut sound).map_err(EncodeError::Sound)?;
        muxer.sound(&sound)?;
        played = until;
        if progress(Progress { done: n + 1, total }) == ControlFlow::Break(()) {
            return Err(EncodeError::Cancelled);
        }
    }
    readback.finish(gpu, &mut |planes| muxer.video(planes))?;
    muxer.finish()?;
    Ok(Exported {
        path: out.to_path_buf(),
        size,
        fps,
        frames: total,
        duration: timeline.duration,
        codec: Some(codec),
        bytes: part.commit()?,
    })
}

/// Both encoders and the file.
struct Muxer {
    writer: Mp4Writer,
    video: VideoEncoder,
    frame: Nv12Frame,
    sound: AudioEncoder,
    packets: Vec<Packet>,
    /// Frames encoded so far: the slot of the next one.
    encoded: i64,
}

impl Muxer {
    fn new(
        video: VideoEncoder,
        settings: VideoSettings,
        timeline: &Timeline<'_>,
        path: &Path,
    ) -> Result<Self, EncodeError> {
        let sound = AudioEncoder::open_aac(timeline.audio)?;
        let writer = Mp4Writer::with_streams(
            path,
            &[&video.parameters(), &sound.parameters()],
            Mp4Layout::Plain,
        )?;
        Ok(Self {
            writer,
            frame: Nv12Frame::new(settings.width, settings.height)?,
            video,
            sound,
            packets: Vec::new(),
            encoded: 0,
        })
    }

    fn video(&mut self, planes: Planes<'_>) -> Result<(), EncodeError> {
        self.frame.copy_from(planes.y, planes.uv, planes.pitch);
        self.video
            .encode(&mut self.frame, self.encoded, &mut self.packets)?;
        self.encoded += 1;
        self.write(VIDEO)
    }

    fn sound(&mut self, samples: &[f32]) -> Result<(), EncodeError> {
        self.sound.encode(samples, &mut self.packets)?;
        self.write(SOUND)
    }

    fn finish(mut self) -> Result<(), EncodeError> {
        self.video.finish(&mut self.packets)?;
        self.write(VIDEO)?;
        self.sound.finish(&mut self.packets)?;
        self.write(SOUND)?;
        self.writer.finish()
    }

    /// Writes the packets `stream`'s encoder gave; a video frame lasts one slot.
    fn write(&mut self, stream: usize) -> Result<(), EncodeError> {
        for mut packet in self.packets.drain(..) {
            if stream == VIDEO {
                packet.set_duration(1);
            }
            self.writer.write_to(stream, packet)?;
        }
        Ok(())
    }
}
