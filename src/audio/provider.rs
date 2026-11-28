use super::signals::AudioSignals;

/// Active audio signal source reported for user-visible diagnostics and HUD display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioSourceKind {
    /// System output loopback capture (e.g. Windows WASAPI loopback).
    SystemAudio,
    /// Microphone or explicit monitor input device capture.
    InputDevice,
    /// Deterministic synthetic beat generator (fallback or offline demo).
    Synthetic,
}

impl AudioSourceKind {
    /// Human-readable label for status overlays and diagnostics.
    pub const fn label(self) -> &'static str {
        match self {
            Self::SystemAudio => "System Audio",
            Self::InputDevice => "Input Device",
            Self::Synthetic => "Synthetic",
        }
    }
}

/// Trait implemented by audio capture providers.
pub trait AudioProvider: Send {
    fn poll_signals(&mut self) -> AudioSignals;
    fn is_live(&self) -> bool {
        false
    }
    fn provider_name(&self) -> &'static str {
        "unknown"
    }
    fn source_kind(&self) -> AudioSourceKind {
        AudioSourceKind::Synthetic
    }
}

/// Metadata description for an enumerated audio input or loopback device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioDeviceInfo {
    pub name: String,
    pub is_default: bool,
    pub direction: &'static str,
}

/// Deterministic mock audio provider for unit testing.
#[derive(Debug, Clone)]
pub struct MockAudioProvider {
    pub signals: AudioSignals,
}

impl MockAudioProvider {
    pub fn new(signals: AudioSignals) -> Self {
        Self { signals }
    }
}

impl AudioProvider for MockAudioProvider {
    fn poll_signals(&mut self) -> AudioSignals {
        self.signals
    }
    fn is_live(&self) -> bool {
        false
    }
    fn provider_name(&self) -> &'static str {
        "mock"
    }
    fn source_kind(&self) -> AudioSourceKind {
        AudioSourceKind::Synthetic
    }
}

/// Procedural synthetic audio signal generator for headless benchmarking and demos.
#[derive(Debug, Clone)]
pub struct SyntheticAudioGenerator {
    pub time: f32,
    pub bpm: f32,
    last_poll: Option<std::time::Instant>,
}

impl Default for SyntheticAudioGenerator {
    fn default() -> Self {
        Self::new(120.0)
    }
}

impl SyntheticAudioGenerator {
    pub fn new(bpm: f32) -> Self {
        Self {
            time: 0.0,
            bpm,
            last_poll: None,
        }
    }
    pub fn step(&mut self, dt: f32) -> AudioSignals {
        self.time += dt;
        let beat_freq = (self.bpm / 60.0).max(0.1);
        let phase = (self.time * beat_freq * std::f32::consts::TAU).sin().abs();
        let bass = phase.powi(3);
        let mid = (self.time * 2.1).sin().abs() * 0.6;
        let treble = (self.time * 4.3).cos().abs() * 0.4;
        let volume = (bass * 0.5 + mid * 0.3 + treble * 0.2).clamp(0.0, 1.0);
        AudioSignals::new(bass, mid, treble, volume)
    }
}

impl AudioProvider for SyntheticAudioGenerator {
    fn poll_signals(&mut self) -> AudioSignals {
        let now = std::time::Instant::now();
        let dt = self.last_poll.map_or(1.0 / 30.0, |prev| {
            now.duration_since(prev).as_secs_f32().clamp(0.0, 0.1)
        });
        self.last_poll = Some(now);
        self.step(dt)
    }
    fn is_live(&self) -> bool {
        false
    }
    fn provider_name(&self) -> &'static str {
        "synthetic"
    }
    fn source_kind(&self) -> AudioSourceKind {
        AudioSourceKind::Synthetic
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synthetic_poll_signals_uses_real_elapsed_time() {
        let mut gen = SyntheticAudioGenerator::new(120.0);
        gen.poll_signals();
        let t1 = gen.time;
        gen.poll_signals();
        assert!(gen.time - t1 < 0.01);
        assert_eq!(gen.source_kind(), AudioSourceKind::Synthetic);
    }
}
