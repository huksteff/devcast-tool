//! MP4 export through FFmpeg: H.264 video from NV12 frames (hardware or CPU encoder),
//! AAC audio, encoder self-test (gray frames → decode → check they are not zeroed).

use ffmpeg::ffi;
use ffmpeg_next as ffmpeg;

pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Encoder-specific options for "good quality, reasonably fast".
fn options(name: &str) -> ffmpeg::Dictionary<'static> {
    let mut d = ffmpeg::Dictionary::new();
    match name {
        "h264_nvenc" => {
            d.set("preset", "p4");
            d.set("rc", "vbr");
            d.set("cq", "21");
        }
        "h264_qsv" => d.set("preset", "medium"),
        "h264_amf" => d.set("quality", "balanced"),
        "h264_mf" => d.set("rate_control", "quality"),
        _ => {
            d.set("preset", "veryfast");
            d.set("crf", "20");
        }
    }
    d
}

pub fn open_video(name: &str, w: u32, h: u32, fps: i32, global_header: bool) -> Result<ffmpeg::encoder::Video> {
    let codec = ffmpeg::encoder::find_by_name(name).ok_or_else(|| format!("no encoder {name}"))?;
    let mut enc = ffmpeg::codec::context::Context::new_with_codec(codec).encoder().video()?;
    enc.set_width(w);
    enc.set_height(h);
    enc.set_format(ffmpeg::format::Pixel::NV12);
    enc.set_time_base((1, fps));
    enc.set_gop(fps as u32);
    enc.set_max_b_frames(0);
    enc.set_bit_rate(0);
    // SAFETY: plain field writes on an encoder context that is not opened yet.
    unsafe {
        (*enc.as_mut_ptr()).framerate = ffi::AVRational { num: fps, den: 1 };
        if global_header {
            (*enc.as_mut_ptr()).flags |= ffi::AV_CODEC_FLAG_GLOBAL_HEADER as i32;
        }
    }
    Ok(enc.open_as_with(codec, options(name))?)
}

/// Encodes a few mid-gray frames, decodes them with the software decoder and checks the luma.
/// Some drivers open a hardware session fine and then emit zeroed frames.
pub fn self_test(name: &str) -> Result<f64> {
    let (w, h) = (640, 360);
    let mut enc = open_video(name, w, h, 30, false)?;
    let mut packets = Vec::new();
    for i in 0..10 {
        let mut frame = ffmpeg::frame::Video::new(ffmpeg::format::Pixel::NV12, w, h);
        frame.data_mut(0).fill(128);
        frame.data_mut(1).fill(128);
        frame.set_pts(Some(i));
        enc.send_frame(&frame)?;
        drain(&mut enc, &mut packets)?;
    }
    enc.send_eof()?;
    drain(&mut enc, &mut packets)?;
    let h264 = ffmpeg::decoder::find(ffmpeg::codec::Id::H264).ok_or("no h264 decoder")?;
    let mut dec = ffmpeg::codec::context::Context::new_with_codec(h264).decoder().video()?;
    let mut sum = 0.0;
    let mut count = 0usize;
    let mut decoded = ffmpeg::frame::Video::empty();
    for packet in &packets {
        dec.send_packet(packet)?;
        while dec.receive_frame(&mut decoded).is_ok() {
            let y = decoded.data(0);
            sum += y.iter().take(w as usize * 16).map(|&v| f64::from(v)).sum::<f64>();
            count += w as usize * 16;
        }
    }
    let mean = if count == 0 { 0.0 } else { sum / count as f64 };
    if (100.0..=156.0).contains(&mean) { Ok(mean) } else { Err(format!("decoded luma {mean:.1}, expected ~128").into()) }
}

fn drain(enc: &mut ffmpeg::encoder::Video, out: &mut Vec<ffmpeg::Packet>) -> Result<()> {
    let mut packet = ffmpeg::Packet::empty();
    while enc.receive_packet(&mut packet).is_ok() {
        out.push(packet.clone());
    }
    Ok(())
}

/// MP4 writer with one H.264 video and one AAC stereo audio stream.
pub struct Mp4 {
    out: ffmpeg::format::context::Output,
    video: ffmpeg::encoder::Video,
    audio: ffmpeg::encoder::Audio,
    video_tb: ffmpeg::Rational,
    audio_tb: ffmpeg::Rational,
    audio_pts: i64,
    sample_rate: i32,
}

impl Mp4 {
    pub fn create(path: &str, encoder: &str, w: u32, h: u32, fps: i32) -> Result<Self> {
        let mut out = ffmpeg::format::output(path)?;
        let global = out.format().flags().contains(ffmpeg::format::Flags::GLOBAL_HEADER);
        let video = open_video(encoder, w, h, fps, global)?;
        let aac = ffmpeg::encoder::find(ffmpeg::codec::Id::AAC).ok_or("no aac")?;
        let mut a = ffmpeg::codec::context::Context::new_with_codec(aac).encoder().audio()?;
        let sample_rate = 48_000;
        a.set_rate(sample_rate);
        a.set_format(ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Planar));
        a.set_channel_layout(ffmpeg::ChannelLayout::default(2));
        a.set_bit_rate(192_000);
        a.set_time_base((1, sample_rate));
        if global {
            // SAFETY: field write on an unopened encoder context.
            unsafe { (*a.as_mut_ptr()).flags |= ffi::AV_CODEC_FLAG_GLOBAL_HEADER as i32 };
        }
        let audio = a.open_as(aac)?;
        out.add_stream(ffmpeg::encoder::find_by_name(encoder))?.set_parameters(&video);
        out.add_stream(aac)?.set_parameters(&audio);
        out.write_header()?;
        let video_tb = out.stream(0).ok_or("no stream 0")?.time_base();
        let audio_tb = out.stream(1).ok_or("no stream 1")?.time_base();
        Ok(Self { out, video, audio, video_tb, audio_tb, audio_pts: 0, sample_rate })
    }

    pub fn video_frame(&mut self, frame: &ffmpeg::frame::Video) -> Result<()> {
        self.video.send_frame(frame)?;
        self.write_video()
    }

    /// Appends audio up to `until` seconds: a 440 Hz sine (left) / 660 Hz (right).
    pub fn audio_until(&mut self, until: f64) -> Result<()> {
        let n = self.audio.frame_size() as usize;
        while (self.audio_pts as f64) / f64::from(self.sample_rate) < until {
            let mut frame = ffmpeg::frame::Audio::new(self.audio.format(), n, self.audio.channel_layout());
            for (ch, freq) in [(0usize, 440.0f64), (1, 660.0)] {
                let plane = frame.plane_mut::<f32>(ch);
                for (i, s) in plane.iter_mut().enumerate() {
                    let t = (self.audio_pts + i as i64) as f64 / f64::from(self.sample_rate);
                    *s = (0.2 * (std::f64::consts::TAU * freq * t).sin()) as f32;
                }
            }
            frame.set_rate(self.sample_rate as u32);
            frame.set_pts(Some(self.audio_pts));
            self.audio_pts += n as i64;
            self.audio.send_frame(&frame)?;
            self.write_audio()?;
        }
        Ok(())
    }

    fn write_video(&mut self) -> Result<()> {
        let mut packet = ffmpeg::Packet::empty();
        while self.video.receive_packet(&mut packet).is_ok() {
            packet.set_stream(0);
            packet.rescale_ts(self.video.time_base(), self.video_tb);
            packet.write_interleaved(&mut self.out)?;
        }
        Ok(())
    }

    fn write_audio(&mut self) -> Result<()> {
        let mut packet = ffmpeg::Packet::empty();
        while self.audio.receive_packet(&mut packet).is_ok() {
            packet.set_stream(1);
            packet.rescale_ts(self.audio.time_base(), self.audio_tb);
            packet.write_interleaved(&mut self.out)?;
        }
        Ok(())
    }

    pub fn finish(mut self) -> Result<()> {
        self.video.send_eof()?;
        self.write_video()?;
        self.audio.send_eof()?;
        self.write_audio()?;
        self.out.write_trailer()?;
        Ok(())
    }
}
