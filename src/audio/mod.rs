//! Audio reactive signals, FFT spectrum analysis, and audio providers.

pub mod capture;
pub mod fft;
pub mod native;
pub mod provider;
pub mod ring_buffer;
pub mod signals;

pub use capture::LiveAudioProvider;
pub use fft::SpectrumAnalyzer;
pub use provider::{
    AudioDeviceInfo, AudioProvider, AudioSourceKind, MockAudioProvider, SyntheticAudioGenerator,
};
pub use ring_buffer::PcmRingBuffer;
pub use signals::AudioSignals;

use crate::config::AudioConfig;
use crate::Result;

/// Enumerates all available audio capture devices across supported platform backends.
pub fn list_audio_devices() -> Vec<AudioDeviceInfo> {
    native::NativeAudioCapture::list_devices()
}

/// Creates a default audio provider (synthetic beat generator or live capture fallback).
pub fn default_audio_provider() -> Box<dyn AudioProvider> {
    Box::new(SyntheticAudioGenerator::default())
}

/// Creates a unified audio provider based on configuration and fallback policy.
///
/// Explicit user requests that cannot be honored — such as `--audio-loopback` on a non-Windows
/// platform, a named `--audio-device` that does not exist, or `--no-audio-fallback` — return an
/// explicit `Err` rather than silently substituting synthetic beats.
pub fn create_audio_provider(
    config: &AudioConfig,
    allow_fallback: bool,
) -> Result<Box<dyn AudioProvider>> {
    if !config.enabled {
        return Ok(Box::new(SyntheticAudioGenerator::new(config.bpm)));
    }

    match create_live_audio_provider(config.device.as_deref(), config.loopback) {
        Ok(mut live_provider) => {
            live_provider.set_fallback_bpm(config.bpm);
            Ok(Box::new(live_provider))
        }
        Err(err) => {
            let unsupported_loopback = config.loopback && !cfg!(target_os = "windows");
            let explicit_device = config.device.is_some();

            if unsupported_loopback || explicit_device || !allow_fallback {
                return Err(err);
            }

            eprintln!("Audio capture unavailable: {err}; falling back to synthetic generator");
            Ok(Box::new(SyntheticAudioGenerator::new(config.bpm)))
        }
    }
}

/// Creates an active `LiveAudioProvider` with the platform-native capture stream running.
pub fn create_live_audio_provider(
    device_name: Option<&str>,
    loopback: bool,
) -> Result<LiveAudioProvider> {
    let ring_buffer = PcmRingBuffer::new(8192);
    let capture = native::NativeAudioCapture::new(ring_buffer.clone(), device_name, loopback)?;
    let analyzer = SpectrumAnalyzer::new(capture.actual_sample_rate, 2048);
    let stream_alive = capture.stream_alive();
    let source_kind = if loopback {
        AudioSourceKind::SystemAudio
    } else {
        AudioSourceKind::InputDevice
    };

    let backend: Option<Box<dyn std::any::Any + Send + Sync>> = Some(Box::new(capture));

    let mut provider = LiveAudioProvider::with_backend(ring_buffer, analyzer, backend)
        .with_source_kind(source_kind);
    provider.set_stream_alive(stream_alive);
    Ok(provider)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_explicit_missing_device_returns_error_instead_of_silent_fallback() {
        let cfg = AudioConfig {
            enabled: true,
            bpm: 120.0,
            device: Some("NonExistentAudioDeviceXYZ987".to_string()),
            loopback: false,
        };
        let err = create_audio_provider(&cfg, true)
            .err()
            .expect("Missing explicit device must fail fast");
        assert!(err.to_string().contains("not found"));
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn test_loopback_on_non_windows_fails_fast_even_when_fallback_allowed() {
        let cfg = AudioConfig {
            enabled: true,
            bpm: 120.0,
            device: None,
            loopback: true,
        };
        let err = create_audio_provider(&cfg, true)
            .err()
            .expect("Loopback on non-Windows must fail fast");
        assert!(err.to_string().contains("Windows WASAPI"));
    }
}
