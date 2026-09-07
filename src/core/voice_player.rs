//! Voice/audio playback for voice messages.
//!
//! GStreamer build: `playbin` over a temp file (bytes in → file out, cleaned
//! on stop/drop). The UI polls [`VoicePlayer::pump`] on its timer for EOS and
//! reads [`VoicePlayer::position`]/[`VoicePlayer::duration`] for progress.
//! Without the `gstreamer` feature every method is an honest `Err`/noop stub.

#[cfg(feature = "gstreamer")]
use std::path::PathBuf;
use std::time::Duration;

/// Generate a mono S16LE WAV (for tests): `secs` seconds of 440Hz sine.
#[cfg(test)]
pub(crate) fn synth_wav_bytes(sample_rate: u32, secs: f32) -> Vec<u8> {
    let n = (sample_rate as f32 * secs) as usize;
    let mut data = Vec::with_capacity(44 + n * 2);
    let block_align: u16 = 2;
    let byte_rate = sample_rate * block_align as u32;
    let data_len = (n * 2) as u32;
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&(36 + data_len).to_le_bytes());
    data.extend_from_slice(b"WAVEfmt ");
    data.extend_from_slice(&16u32.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes());
    data.extend_from_slice(&sample_rate.to_le_bytes());
    data.extend_from_slice(&byte_rate.to_le_bytes());
    data.extend_from_slice(&block_align.to_le_bytes());
    data.extend_from_slice(&16u16.to_le_bytes());
    data.extend_from_slice(b"data");
    data.extend_from_slice(&data_len.to_le_bytes());
    for i in 0..n {
        let t = i as f32 / sample_rate as f32;
        let s = (f32::sin(t * 440.0 * 2.0 * std::f32::consts::PI) * 20000.0) as i16;
        data.extend_from_slice(&s.to_le_bytes());
    }
    data
}

#[cfg(feature = "gstreamer")]
mod imp {
    use super::*;
    use gstreamer::prelude::*;

    pub struct VoicePlayer {
        pipeline: Option<gstreamer::Element>,
        tmp_path: Option<PathBuf>,
        playing: bool,
        eos: bool,
        use_fake_sink: bool,
    }

    impl VoicePlayer {
        pub fn new() -> Self {
            Self {
                pipeline: None,
                tmp_path: None,
                playing: false,
                eos: false,
                use_fake_sink: false,
            }
        }

        /// Route audio to `fakesink` (tests: no sound card needed).
        pub fn with_fake_sink(mut self) -> Self {
            self.use_fake_sink = true;
            self
        }

        fn build(&mut self, uri: &str) -> Result<(), String> {
            self.teardown();
            gstreamer::init().map_err(|e| format!("gst init: {e}"))?;
            let playbin = gstreamer::ElementFactory::make("playbin")
                .property("uri", uri)
                .build()
                .map_err(|e| format!("playbin: {e}"))?;
            if self.use_fake_sink {
                let fake = gstreamer::ElementFactory::make("fakesink")
                    .property("sync", true)
                    .build()
                    .map_err(|e| format!("fakesink: {e}"))?;
                playbin.set_property("audio-sink", &fake);
                playbin.set_property("video-sink", &fake);
            }
            playbin
                .set_state(gstreamer::State::Playing)
                .map_err(|e| format!("play: {e:?}"))?;
            self.pipeline = Some(playbin);
            self.playing = true;
            self.eos = false;
            Ok(())
        }

        pub fn play_file(&mut self, path: &std::path::Path) -> Result<(), String> {
            let uri = format!("file://{}", path.display());
            self.build(&uri)
        }

        pub fn play_bytes(&mut self, bytes: &[u8], suffix: &str) -> Result<(), String> {
            if bytes.is_empty() {
                return Err("empty audio".to_string());
            }
            let mut path = std::env::temp_dir();
            path.push(format!(
                "ym_voice_{}_{suffix}",
                uuid::Uuid::new_v4().simple()
            ));
            std::fs::write(&path, bytes).map_err(|e| format!("temp write: {e}"))?;
            if let Err(e) = self.build(&format!("file://{}", path.display())) {
                let _ = std::fs::remove_file(&path);
                return Err(e);
            }
            if let Some(old) = self.tmp_path.replace(path) {
                let _ = std::fs::remove_file(old);
            }
            Ok(())
        }

        pub fn pause(&mut self) {
            if let Some(p) = self.pipeline.as_ref() {
                let _ = p.set_state(gstreamer::State::Paused);
            }
            self.playing = false;
        }

        pub fn resume(&mut self) -> Result<(), String> {
            match self.pipeline.as_ref() {
                Some(p) => {
                    p.set_state(gstreamer::State::Playing)
                        .map_err(|e| format!("resume: {e:?}"))?;
                    self.playing = true;
                    self.eos = false;
                    Ok(())
                }
                None => Err("nothing to resume".to_string()),
            }
        }

        pub fn stop(&mut self) {
            self.teardown();
        }

        fn teardown(&mut self) {
            if let Some(p) = self.pipeline.take() {
                let _ = p.set_state(gstreamer::State::Null);
            }
            if let Some(path) = self.tmp_path.take() {
                let _ = std::fs::remove_file(path);
            }
            self.playing = false;
        }

        pub fn is_playing(&self) -> bool {
            self.playing && !self.eos
        }

        pub fn eos_reached(&self) -> bool {
            self.eos
        }

        pub fn position(&self) -> Option<Duration> {
            self.pipeline
                .as_ref()?
                .query_position::<gstreamer::ClockTime>()
                .map(|t| Duration::from_nanos(t.nseconds()))
        }

        pub fn duration(&self) -> Option<Duration> {
            self.pipeline
                .as_ref()?
                .query_duration::<gstreamer::ClockTime>()
                .map(|t| Duration::from_nanos(t.nseconds()))
        }

        /// Poll the bus (non-blocking): EOS stops playback, errors are logged.
        /// Returns true on EOS.
        pub fn pump(&mut self) -> bool {
            let bus = match self.pipeline.as_ref().and_then(|p| p.bus()) {
                Some(b) => b,
                None => return false,
            };
            let mut eos = false;
            while let Some(msg) = bus.timed_pop_filtered(
                gstreamer::ClockTime::ZERO,
                &[gstreamer::MessageType::Eos, gstreamer::MessageType::Error],
            ) {
                match msg.view() {
                    gstreamer::MessageView::Eos(..) => {
                        eos = true;
                    }
                    gstreamer::MessageView::Error(e) => {
                        log::warn!("voice playback error: {} ({:?})", e.error(), e.debug());
                        eos = true;
                    }
                    _ => {}
                }
            }
            if eos {
                self.eos = true;
                self.playing = false;
                if let Some(p) = self.pipeline.as_ref() {
                    let _ = p.set_state(gstreamer::State::Ready);
                }
            }
            eos
        }
    }

    impl Drop for VoicePlayer {
        fn drop(&mut self) {
            self.teardown();
        }
    }
}

#[cfg(not(feature = "gstreamer"))]
mod imp {
    use super::*;

    pub struct VoicePlayer {
        _private: (),
    }

    impl VoicePlayer {
        pub fn new() -> Self {
            Self { _private: () }
        }

        pub fn play_file(&mut self, _path: &std::path::Path) -> Result<(), String> {
            Err("voice playback needs the gstreamer build".to_string())
        }

        pub fn play_bytes(&mut self, _bytes: &[u8], _suffix: &str) -> Result<(), String> {
            Err("voice playback needs the gstreamer build".to_string())
        }

        pub fn pause(&mut self) {}

        pub fn resume(&mut self) -> Result<(), String> {
            Err("voice playback needs the gstreamer build".to_string())
        }

        pub fn stop(&mut self) {}

        pub fn is_playing(&self) -> bool {
            false
        }

        pub fn eos_reached(&self) -> bool {
            false
        }

        pub fn position(&self) -> Option<Duration> {
            None
        }

        pub fn duration(&self) -> Option<Duration> {
            None
        }

        pub fn pump(&mut self) -> bool {
            false
        }
    }
}

pub use imp::VoicePlayer;

#[cfg(all(test, feature = "gstreamer"))]
mod tests {
    use super::*;

    #[test]
    fn playback_runs_to_eos() {
        let wav = synth_wav_bytes(16000, 0.4);
        let mut player = VoicePlayer::new().with_fake_sink();
        player.play_bytes(&wav, "wav").expect("play");

        // Position must advance and EOS must arrive (~0.4s of audio).
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
        let mut saw_position = false;
        while !player.eos_reached() && std::time::Instant::now() < deadline {
            player.pump();
            if player
                .position()
                .map(|p| p.as_millis() > 0)
                .unwrap_or(false)
            {
                saw_position = true;
            }
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
        assert!(saw_position, "position never advanced");
        assert!(player.eos_reached(), "no EOS");

        if let Some(d) = player.duration() {
            let secs = d.as_secs_f32();
            assert!((0.2..0.8).contains(&secs), "suspicious duration: {secs}");
        }
        player.stop();
        assert!(!player.is_playing());
    }

    #[test]
    fn pause_resume() {
        let wav = synth_wav_bytes(16000, 1.0);
        let mut player = VoicePlayer::new().with_fake_sink();
        player.play_bytes(&wav, "wav").expect("play");
        assert!(player.is_playing());
        player.pause();
        assert!(!player.is_playing());
        player.resume().expect("resume");
        assert!(player.is_playing());
        player.stop();
    }

    #[test]
    fn empty_bytes_rejected() {
        let mut player = VoicePlayer::new().with_fake_sink();
        assert!(player.play_bytes(&[], "ogg").is_err());
    }
}
