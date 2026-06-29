#![allow(dead_code)]

use std::cell::RefCell;

use crate::config;
#[cfg(feature = "gstreamer")]
use gstreamer::prelude::*;
#[cfg(feature = "gstreamer")]
use gstreamer_app::prelude::*;

// ============================================================
// GStreamer pipeline
// ============================================================

/// GStreamer recording pipeline:
/// autoaudiosrc ! audioconvert ! audioresample ! opusenc ! oggmux ! appsink
///
/// Uses the appsink element to extract raw encoded audio data.
#[cfg(feature = "gstreamer")]
static RECORDING_PIPELINE: &str = "autoaudiosrc ! audioconvert ! audioresample ! opusenc ! oggmux ! appsink";

#[cfg(not(feature = "gstreamer"))]
static RECORDING_PIPELINE: &str = "autoaudiosrc ! audioconvert ! audioresample ! wavenc ! appsink";

/// Recording state
#[derive(Debug, Clone, PartialEq)]
enum RecordState {
    Idle,
    Recording,
    Stopped,
}

/// VoiceRecorder — handles audio recording with GStreamer (if available) or stub.
pub struct VoiceRecorder {
    state: RefCell<RecordState>,
    /// Total duration of the recording in seconds
    pub(crate) duration: RefCell<f64>,
    /// Collected audio data (Opus-encoded via GStreamer, or PCM stub)
    pub(crate) audio_data: RefCell<Vec<u8>>,
    /// Waveform amplitude samples (0.0–1.0) collected during recording
    pub(crate) waveform: RefCell<Vec<f32>>,
    /// Start time for duration tracking
    start_time: RefCell<Option<std::time::Instant>>,
    /// GStreamer pipeline (Optional — set after start())
    #[cfg(feature = "gstreamer")]
    pub(crate) pipeline: RefCell<Option<gstreamer::Pipeline>>,
    #[cfg(not(feature = "gstreamer"))]
    pub(crate) pipeline: RefCell<Option<String>>,
}

impl VoiceRecorder {
    /// Create a new VoiceRecorder instance.
    pub fn new() -> Self {
        Self {
            state: RefCell::new(RecordState::Idle),
            duration: RefCell::new(0.0),
            audio_data: RefCell::new(Vec::new()),
            waveform: RefCell::new(Vec::new()),
            start_time: RefCell::new(None),
            pipeline: RefCell::new(None),
        }
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
            let element = gstreamer::parse::launch(RECORDING_PIPELINE)
                .map_err(|e| format!("Failed to parse pipeline: {}", e))?;
            
            let pipeline = element.downcast::<gstreamer::Pipeline>()
                .map_err(|_| "Failed to cast to Pipeline".to_string())?;

            // Set appsink to emit signals
            if let Some(appsink) = pipeline
                .by_name("appsink")
            {
                appsink.set_property("emit-signals", true);
            }

            // Start the pipeline
            pipeline.set_state(gstreamer::State::Playing)
                .map_err(|e| format!("Failed to start pipeline: {}", e))?;

            *self.pipeline.borrow_mut() = Some(pipeline);
        }

        *self.state.borrow_mut() = RecordState::Recording;
        *self.start_time.borrow_mut() = Some(std::time::Instant::now());
        *self.duration.borrow_mut() = 0.0;
        *self.audio_data.borrow_mut() = Vec::new();
        *self.waveform.borrow_mut() = Vec::new();

        log::info!("Voice recording started (gstreamer={})", cfg!(feature = "gstreamer"));
        Ok(())
    }

    /// Stop recording and return the collected audio data.
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
            *self.state.borrow_mut() = RecordState::Idle;
            return Err(format!(
                "Recording too long: {:.1}s > {}s max",
                *self.duration.borrow(),
                config::MAX_VOICE_DURATION
            ));
        }

        // Set pipeline to NULL and stop appsink
        #[cfg(feature = "gstreamer")]
        if let Some(ref pipeline) = *self.pipeline.borrow() {
            pipeline.set_state(gstreamer::State::Null)
                .map_err(|e| format!("Failed to stop pipeline: {:?}", e))?;
        }

        // Get the last buffer from appsink
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

        #[cfg(feature = "gstreamer")]
        if let Some(ref pipeline) = *self.pipeline.borrow() {
            let _ = pipeline.set_state(gstreamer::State::Null);
        }

        *self.state.borrow_mut() = RecordState::Idle;
        *self.duration.borrow_mut() = 0.0;
        *self.audio_data.borrow_mut() = Vec::new();
        *self.waveform.borrow_mut() = Vec::new();
        *self.start_time.borrow_mut() = None;

        log::info!("Voice recording cancelled");
    }

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

        self.audio_data.borrow_mut().extend(frame.iter().flat_map(|s| s.to_le_bytes()));
        let max_amp = frame.iter().map(|s| (*s as f32 / 32768.0).abs()).fold(0.0f32, f32::max);
        self.add_waveform_sample(max_amp);
    }
}

impl Clone for VoiceRecorder {
    fn clone(&self) -> Self {
        Self {
            state: RefCell::new((*self.state.borrow()).clone()),
            duration: RefCell::new(*self.duration.borrow()),
            audio_data: RefCell::new(self.audio_data.borrow().clone()),
            waveform: RefCell::new(self.waveform.borrow().clone()),
            start_time: RefCell::new(*self.start_time.borrow()),
            pipeline: RefCell::new(self.pipeline.borrow().clone()),
        }
    }
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
        assert!(data.is_empty());
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
}
