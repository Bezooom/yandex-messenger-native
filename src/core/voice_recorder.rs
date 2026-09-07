#![allow(dead_code)]

use std::cell::RefCell;

use crate::config;
#[cfg(feature = "gstreamer")]
use gstreamer::prelude::*;

// ============================================================
// GStreamer pipeline (see launch_string): capture → tee → Opus/OGG appsink
// for bytes + S16LE appsink tap for the waveform meter.
// ============================================================

/// Audio capture source. `Test` needs no microphone (unit tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoiceSource {
    Auto,
    Test,
}

/// Recording state
#[derive(Debug, Clone, PartialEq)]
enum RecordState {
    Idle,
    Recording,
    Stopped,
}

/// VoiceRecorder — captures Opus/OGG via GStreamer (or stub without it).
///
/// Pipeline (gstreamer):
/// `{src} ! audioconvert ! audioresample ! tee name=t ! queue ! opusenc !
/// oggmux ! appsink name=enc; t. ! queue ! audioconvert ! audioresample !
/// audio/x-raw,format=S16LE,channels=1,rate=16000 ! appsink name=pcm`
///
/// Samples are pulled explicitly with [`VoiceRecorder::pump`] (no signal
/// threads): encoded bytes accumulate, PCM chunks feed the waveform meter.
pub struct VoiceRecorder {
    source: VoiceSource,
    state: RefCell<RecordState>,
    /// Total duration of the recording in seconds
    pub(crate) duration: RefCell<f64>,
    /// Collected Opus/OGG bytes (empty without gstreamer).
    pub(crate) audio_data: RefCell<Vec<u8>>,
    /// Waveform amplitude samples (0.0–1.0) collected during recording
    pub(crate) waveform: RefCell<Vec<f32>>,
    /// Start time for duration tracking
    start_time: RefCell<Option<std::time::Instant>>,
    /// Last waveform sample time (throttle ~20Hz).
    last_wave: RefCell<Option<std::time::Instant>>,
    /// GStreamer pipeline (Optional — set after start())
    #[cfg(feature = "gstreamer")]
    pub(crate) pipeline: RefCell<Option<gstreamer::Pipeline>>,
    #[cfg(not(feature = "gstreamer"))]
    pub(crate) pipeline: RefCell<Option<String>>,
    /// Named appsinks for polling.
    #[cfg(feature = "gstreamer")]
    enc_sink: RefCell<Option<gstreamer_app::AppSink>>,
    #[cfg(feature = "gstreamer")]
    pcm_sink: RefCell<Option<gstreamer_app::AppSink>>,
}

impl VoiceRecorder {
    /// Create a new VoiceRecorder instance (microphone).
    pub fn new() -> Self {
        Self::with_source(VoiceSource::Auto)
    }

    pub fn with_source(source: VoiceSource) -> Self {
        Self {
            source,
            state: RefCell::new(RecordState::Idle),
            duration: RefCell::new(0.0),
            audio_data: RefCell::new(Vec::new()),
            waveform: RefCell::new(Vec::new()),
            start_time: RefCell::new(None),
            last_wave: RefCell::new(None),
            pipeline: RefCell::new(None),
            #[cfg(feature = "gstreamer")]
            enc_sink: RefCell::new(None),
            #[cfg(feature = "gstreamer")]
            pcm_sink: RefCell::new(None),
        }
    }

    fn launch_string(&self) -> String {
        let src = match self.source {
            VoiceSource::Auto => "autoaudiosrc",
            VoiceSource::Test => "audiotestsrc is-live=true wave=sine freq=440",
        };
        format!(
            "{src} ! audioconvert ! audioresample ! tee name=t \
             t. ! queue ! opusenc bitrate=64000 ! oggmux ! appsink name=enc sync=false max-buffers=0 drop=false emit-signals=false \
             t. ! queue ! audioconvert ! audioresample ! audio/x-raw,format=S16LE,channels=1,rate=16000 ! appsink name=pcm sync=false max-buffers=0 drop=false emit-signals=false"
        )
    }

    /// Start recording audio using GStreamer (or stub mode).
    pub fn start(&self) -> Result<(), String> {
        if *self.state.borrow() != RecordState::Idle {
            return Err("Already recording".to_string());
        }

        #[cfg(feature = "gstreamer")]
        {
            // Initialize GStreamer
            gstreamer::init().map_err(|e| format!("GStreamer init failed: {}", e))?;

            // Create pipeline
            let element = gstreamer::parse::launch(&self.launch_string())
                .map_err(|e| format!("Failed to parse pipeline: {}", e))?;

            let pipeline = element
                .downcast::<gstreamer::Pipeline>()
                .map_err(|_| "Failed to cast to Pipeline".to_string())?;

            // Grab the appsinks for polling (no signal threads).
            let enc = pipeline
                .by_name("enc")
                .and_then(|e| e.downcast::<gstreamer_app::AppSink>().ok())
                .ok_or_else(|| "no enc appsink".to_string())?;
            let pcm = pipeline
                .by_name("pcm")
                .and_then(|e| e.downcast::<gstreamer_app::AppSink>().ok())
                .ok_or_else(|| "no pcm appsink".to_string())?;

            // Start the pipeline
            pipeline
                .set_state(gstreamer::State::Playing)
                .map_err(|e| format!("Failed to start pipeline: {}", e))?;

            *self.pipeline.borrow_mut() = Some(pipeline);
            *self.enc_sink.borrow_mut() = Some(enc);
            *self.pcm_sink.borrow_mut() = Some(pcm);
        }

        *self.state.borrow_mut() = RecordState::Recording;
        *self.start_time.borrow_mut() = Some(std::time::Instant::now());
        *self.last_wave.borrow_mut() = None;
        *self.duration.borrow_mut() = 0.0;
        *self.audio_data.borrow_mut() = Vec::new();
        *self.waveform.borrow_mut() = Vec::new();

        log::info!(
            "Voice recording started (gstreamer={})",
            cfg!(feature = "gstreamer")
        );
        Ok(())
    }

    /// Drain pending samples without blocking. Call periodically (UI timer or
    /// test loop) while recording; returns `(new_bytes, new_waveform_samples)`.
    pub fn pump(&self) -> (usize, usize) {
        if *self.state.borrow() != RecordState::Recording {
            return (0, 0);
        }
        #[cfg(feature = "gstreamer")]
        {
            let mut bytes = 0usize;
            let mut waves = 0usize;
            if let Some(enc) = self.enc_sink.borrow().as_ref() {
                while let Some(sample) = enc.try_pull_sample(gstreamer::ClockTime::ZERO) {
                    if let Some(buffer) = sample.buffer() {
                        if let Ok(map) = buffer.map_readable() {
                            bytes += map.as_slice().len();
                            self.audio_data
                                .borrow_mut()
                                .extend_from_slice(map.as_slice());
                        }
                    }
                }
            }
            if let Some(pcm) = self.pcm_sink.borrow().as_ref() {
                while let Some(sample) = pcm.try_pull_sample(gstreamer::ClockTime::ZERO) {
                    if let Some(buffer) = sample.buffer() {
                        if let Ok(map) = buffer.map_readable() {
                            let peak = pcm_peak(map.as_slice());
                            let now = std::time::Instant::now();
                            let due = self
                                .last_wave
                                .borrow()
                                .map(|t| now.duration_since(t).as_millis() >= 50)
                                .unwrap_or(true);
                            if due {
                                *self.last_wave.borrow_mut() = Some(now);
                                self.add_waveform_sample(peak);
                                waves += 1;
                            }
                        }
                    }
                }
            }
            (bytes, waves)
        }
        #[cfg(not(feature = "gstreamer"))]
        {
            (0, 0)
        }
    }

    /// Stop recording and return the collected audio data (Opus/OGG).
    ///
    /// Sends EOS first so `oggmux` finalizes headers, waits for it on the
    /// bus (5s cap), then tears down.
    pub fn stop(&self) -> Result<Vec<u8>, String> {
        if *self.state.borrow() != RecordState::Recording {
            return Err("Not recording".to_string());
        }

        // Calculate duration
        if let Some(start) = *self.start_time.borrow() {
            let elapsed = start.elapsed().as_secs_f64();
            *self.duration.borrow_mut() = elapsed;
        }

        // Check max duration
        if *self.duration.borrow() > config::MAX_VOICE_DURATION as f64 {
            self.shutdown_pipeline();
            *self.state.borrow_mut() = RecordState::Idle;
            return Err(format!(
                "Recording too long: {:.1}s > {}s max",
                *self.duration.borrow(),
                config::MAX_VOICE_DURATION
            ));
        }

        #[cfg(feature = "gstreamer")]
        {
            use gstreamer::prelude::*;
            if let Some(ref pipeline) = *self.pipeline.borrow() {
                let _ = pipeline.send_event(gstreamer::event::Eos::new());
                if let Some(bus) = pipeline.bus() {
                    let _ = bus.timed_pop_filtered(
                        gstreamer::ClockTime::from_seconds(5),
                        &[gstreamer::MessageType::Eos, gstreamer::MessageType::Error],
                    );
                }
            }
            // Final drain after EOS (muxer flushes headers/trailers).
            self.pump();
            self.shutdown_pipeline();
        }

        let data = self.audio_data.borrow().clone();
        *self.state.borrow_mut() = RecordState::Idle;

        log::info!(
            "Voice recording stopped, duration: {:.2}s, size: {} bytes",
            *self.duration.borrow(),
            data.len()
        );
        Ok(data)
    }

    /// Cancel recording and discard collected data.
    pub fn cancel(&self) {
        if *self.state.borrow() != RecordState::Recording {
            return;
        }

        self.shutdown_pipeline();

        *self.state.borrow_mut() = RecordState::Idle;
        *self.duration.borrow_mut() = 0.0;
        *self.audio_data.borrow_mut() = Vec::new();
        *self.waveform.borrow_mut() = Vec::new();
        *self.start_time.borrow_mut() = None;
        *self.last_wave.borrow_mut() = None;

        log::info!("Voice recording cancelled");
    }

    #[cfg(feature = "gstreamer")]
    fn shutdown_pipeline(&self) {
        use gstreamer::prelude::*;
        if let Some(ref pipeline) = *self.pipeline.borrow() {
            let _ = pipeline.set_state(gstreamer::State::Null);
        }
        *self.pipeline.borrow_mut() = None;
        *self.enc_sink.borrow_mut() = None;
        *self.pcm_sink.borrow_mut() = None;
    }

    #[cfg(not(feature = "gstreamer"))]
    fn shutdown_pipeline(&self) {}

    /// Check if currently recording.
    pub fn is_recording(&self) -> bool {
        *self.state.borrow() == RecordState::Recording
    }

    /// Get current recording duration in seconds.
    pub fn duration(&self) -> f64 {
        if let Some(start) = *self.start_time.borrow() {
            start.elapsed().as_secs_f64()
        } else {
            *self.duration.borrow()
        }
    }

    /// Get collected waveform data for visualization.
    pub fn waveform(&self) -> Vec<f32> {
        self.waveform.borrow().clone()
    }

    /// Update waveform with a new amplitude sample.
    pub fn add_waveform_sample(&self, amplitude: f32) {
        let mut wave = self.waveform.borrow_mut();
        if wave.len() < 500 {
            wave.push(amplitude.clamp(0.0, 1.0));
        }
    }

    /// Get the GStreamer pipeline (for debugging/inspection).
    #[cfg(feature = "gstreamer")]
    pub fn pipeline(&self) -> Option<gstreamer::Pipeline> {
        self.pipeline.borrow().clone()
    }

    /// Simulate audio input for testing (generates silent PCM).
    /// In production, GStreamer handles audio capture.
    pub fn simulate_input(&self, sample_rate: u32) {
        if !self.is_recording() {
            return;
        }

        let mut frame = vec![0u16; sample_rate as usize / 10];
        for sample in frame.iter_mut() {
            let noise = (random_u16() % 100) as i16;
            *sample = noise as u16;
        }

        self.audio_data
            .borrow_mut()
            .extend(frame.iter().flat_map(|s| s.to_le_bytes()));
        let max_amp = frame
            .iter()
            .map(|s| (*s as f32 / 32768.0).abs())
            .fold(0.0f32, f32::max);
        self.add_waveform_sample(max_amp);
    }
}

impl Clone for VoiceRecorder {
    fn clone(&self) -> Self {
        Self {
            source: self.source,
            state: RefCell::new((*self.state.borrow()).clone()),
            duration: RefCell::new(*self.duration.borrow()),
            audio_data: RefCell::new(self.audio_data.borrow().clone()),
            waveform: RefCell::new(self.waveform.borrow().clone()),
            start_time: RefCell::new(*self.start_time.borrow()),
            last_wave: RefCell::new(*self.last_wave.borrow()),
            pipeline: RefCell::new(self.pipeline.borrow().clone()),
            #[cfg(feature = "gstreamer")]
            enc_sink: RefCell::new(self.enc_sink.borrow().clone()),
            #[cfg(feature = "gstreamer")]
            pcm_sink: RefCell::new(self.pcm_sink.borrow().clone()),
        }
    }
}

/// Peak amplitude of an S16LE mono chunk, 0.0–1.0.
fn pcm_peak(bytes: &[u8]) -> f32 {
    let mut peak = 0f32;
    for chunk in bytes.chunks_exact(2) {
        let sample = i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0;
        peak = peak.max(sample.abs());
    }
    peak.clamp(0.0, 1.0)
}

fn random_u16() -> u16 {
    use std::cell::Cell;
    thread_local!(static SEED: Cell<u64> = Cell::new(42));
    SEED.with(|s| {
        let mut val = s.get();
        val = val.wrapping_mul(6364136223846793005).wrapping_add(1);
        s.set(val);
        (val >> 48) as u16
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_start_stop() {
        let recorder = VoiceRecorder::new();
        assert!(recorder.start().is_ok());
        assert!(recorder.is_recording());
        let data = recorder.stop().unwrap();
        #[cfg(not(feature = "gstreamer"))]
        assert!(data.is_empty());
        #[cfg(feature = "gstreamer")]
        drop(data);
        assert!(!recorder.is_recording());
    }

    #[test]
    fn test_cancel() {
        let recorder = VoiceRecorder::new();
        recorder.start().unwrap();
        recorder.cancel();
        assert!(!recorder.is_recording());
    }

    #[test]
    fn test_waveform() {
        let recorder = VoiceRecorder::new();
        recorder.start().unwrap();
        recorder.add_waveform_sample(0.5);
        recorder.add_waveform_sample(0.8);
        assert_eq!(recorder.waveform().len(), 2);
        recorder.stop().unwrap();
    }

    #[test]
    fn test_pcm_peak() {
        assert_eq!(pcm_peak(&[]), 0.0);
        assert_eq!(pcm_peak(&[0x00, 0x00]), 0.0);
        let loud = pcm_peak(&[0xFF, 0x7F]);
        assert!(loud > 0.99 && loud <= 1.0);
    }

    /// Real pipeline test (gstreamer): records test tone, checks OGG bytes.
    #[cfg(feature = "gstreamer")]
    #[test]
    fn test_record_produces_ogg() {
        let recorder = VoiceRecorder::with_source(VoiceSource::Test);
        recorder.start().expect("start");
        // Let the live test source flow, pumping like the UI timer does.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut pumped = 0usize;
        while recorder.audio_data.borrow().is_empty() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(50));
            let (bytes, _) = recorder.pump();
            pumped += bytes;
        }
        assert!(pumped > 0, "no encoded bytes captured");
        // A few more pumps for waveform + muxer warmup.
        for _ in 0..6 {
            std::thread::sleep(std::time::Duration::from_millis(50));
            recorder.pump();
        }
        assert!(!recorder.waveform().is_empty(), "no waveform samples");
        let data = recorder.stop().expect("stop");
        assert!(
            data.len() >= 4 && &data[..4] == b"OggS",
            "not an OGG stream"
        );
        assert!(recorder.duration() > 0.0);
    }
}
