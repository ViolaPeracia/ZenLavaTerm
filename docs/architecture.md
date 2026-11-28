# Architecture

ZenLavaTerm (`lavaterm`) is a terminal-native ambient lava lamp and metaball visualizer written in Rust (Edition 2021, `rustc 1.86+`). It renders directly to a TTY using `crossterm` and ANSI TrueColor escape sequences rather than a GUI toolkit or webview.

## Unidirectional data flow

Every frame moves forward through five decoupled stages:

```text
Platform / Inputs  -->  Signals & Interactions
                              |
                              v
                       Simulation Core (Blobs, Field, Physics)
                              |
                              v
                       Virtual Framebuffer (RGB grid)
                              |
                              v
                       Terminal Renderer (HalfBlock, Block, Braille)
                              |
                              v
                       Stdout TTY Stream (Batched ANSI bytes)
```

`src/core/` operates exclusively in normalized `[0.0, 1.0]` continuous space and never imports `crossterm`, OS telemetry crates, audio APIs, or terminal rendering code. All hardware telemetry and audio backends normalize their outputs into `[0.0, 1.0]` signals and degrade to deterministic synthetic fallbacks when hardware is unavailable.

## Module breakdown

The library crate root is [src/lib.rs](../src/lib.rs) and the CLI binary entry point is [src/main.rs](../src/main.rs).

### Core simulation (`src/core/`)

`core` implements the metaball physics and scalar field math:

- **Scalar field (`field.rs`)**: Evaluates metaball potential summation and weighted temperature across normalized 2D coordinates with area-preserving isotropic aspect scaling `(sqrt(aspect), 1/sqrt(aspect))` and compact-support metaball falloff (`d < 2.2 * r`) so 10–32 droplets stay distinct without far-field clumping.
- **Simulation (`simulation.rs`)**: Steps blob positions, thermal buoyancy, 2D convective swirl with soft inter-blob separation, viscosity drag, user `speed_scale` (`0.25x..4.0x` with safe sub-stepping), expanding `RippleWave` surface rings, and audio impulses. Audio reactions use frame-rate-independent (`dt`-normalized) EMA bass smoothing, edge-detonated bass shockwaves with inward rebound, smoothly decaying `audio_beat_envelope` radius/glow breathing, soft-music temperature shimmer, and `sqrt(dt)` treble jitter.
- **User baseline & compact preservation**: `Simulation::base_params()`, `Simulation::set_base_params()`, `Simulation::apply_compact_scales()` (with spatially uniform subsampling when shrinking), `Simulation::nudge_buoyancy()`, `Simulation::nudge_speed()`, and `Simulation::reset()` keep configured `[simulation]` parameters, dynamic compact resize profiles, and user offsets intact across reactive polls and terminal resizes.

### Audio pipeline (`src/audio/`)

`audio` captures live system or microphone audio and extracts frequency bands without blocking the render loop:

- **Lock-free ring buffer (`ring_buffer.rs`)**: `PcmRingBuffer` uses a single-producer, single-consumer (SPSC) model with a 64-bit Seqlock (`version: AtomicU64`) and an atomic CAS producer guard so readers obtain tear-free snapshots without blocking writers.
- **Zero-allocation spectrum analysis & Adaptive AGC (`fft.rs`)**: `SpectrumAnalyzer` pre-allocates its Hann window and FFT scratch buffers (default 2048-sample window) so `analyze(&mut self, pcm)` performs in-place Cooley-Tukey Radix-2 FFT with Parseval band RMS energy and noise-gated Adaptive Auto-Gain Control (AGC) across quiet and loud playback volumes.
- **Native capture, source reporting, and fallbacks (`native.rs`, `capture.rs`, `provider.rs`)**: Captures audio via `cpal` (`F32`, `I16`, `U16`, `I32`, `F64`; WASAPI input or `--audio-loopback` desktop output on Windows, ALSA on Linux, CoreAudio on macOS). Idle capture buffers decay smoothly to silence between 40ms and 140ms without new samples, transient Seqlock contention holds the last verified frame instead of injecting synthetic beats, and non-fatal stream errors (such as pause, `Xrun`, `DeviceChanged`, or `RealtimeDenied`) do not trigger synthetic fallback beats. Every provider exposes `AudioSourceKind` (`SystemAudio`, `InputDevice`, `Synthetic`) so the TUI can display an automatic fallback badge whenever capture degrades to `SyntheticAudioGenerator` (unless `--no-audio-fallback` is set).

### Reactive telemetry (`src/reactive/`)

`reactive` translates OS resource metrics into normalized `[0.0, 1.0]` signals via the `SystemProvider` trait:

- **Linux**: Reads `/proc/stat`, `/proc/meminfo`, and `/sys/class/power_supply`.
- **Windows**: Queries Win32 APIs (`GetSystemTimes`, `GlobalMemoryStatusEx`, `GetSystemPowerStatus`).
- **macOS**: Calls Mach kernel APIs (`host_statistics64` with `HOST_CPU_LOAD_INFO` and `HOST_VM_INFO64`).
- **Fallback**: `MockSystemProvider` supplies deterministic telemetry on unsupported platforms or when OS queries fail.

### Rendering (`src/render/`)

`render` discretizes the scalar field into a 2D pixel buffer and encodes ANSI output:

- **Virtual framebuffer & rasterizer (`framebuffer.rs`, `mod.rs`, `color.rs`)**: Holds a contiguous RGB grid and maps field potential and temperature through `RasterizeOptions` (`threshold`, `gradient`, `ambient_glow`, `molten_pool`, `aspect_ratio`). Uses physically-inspired volumetric wax shading (deep curved rim, theme-faithful glowing core, bottom bulb under-lighting, expanding ripple wave crests, and optional `molten_pool`) while automatically disabling ambient glow for 1-bit `braille` and stepped color modes to keep silhouettes sharp.
- **Sub-cell encoders (`halfblock.rs`, `block.rs`, `braille.rs`)**:
  - `HalfBlockRenderer`: Packs two vertical pixels per cell using `▀` (2x vertical resolution).
  - `BlockRenderer`: Maps one pixel per character cell using `█`.
  - `BrailleRenderer`: Maps a 2x4 subpixel dot matrix per cell using `U+2800`..`U+28FF`.
- **Cursor modes, Synchronized Output, HUD & cleanup**: Renderers support `CursorMode::{Absolute, Inline, Stream}` (where `Stream` emits LF-only rows for snapshot/piping, and `Inline` tracks `last_rendered_rows`, emits `\x1b[K` per row, and clears leftover lines with `\x1b[J` when shrinking). Interactive frames wrap rendering and the alpha-blended, rounded-border top-left HUD (`Ctrl+i` / `Tab`, supported in both fullscreen and inline via `\x1b7`/`\x1b8`) in Synchronized Output (`\x1b[?2026h…l`) with a 256 KB write buffer to eliminate HUD flicker. Fullscreen mode also disables line-wrap (`\x1b[?7l`) and syncs native terminal padding via `OSC 11/111`, restored cleanly by RAII `TerminalGuard` on exit or panic.

### Input (`src/input/`)

`input` translates `crossterm` keyboard and mouse events into simulation actions (`Quit` via `Esc`/`Ctrl+q`/`Ctrl+c`, `TogglePause` via `Ctrl+p`, `Reset` via `Ctrl+r`, `ToggleHud` via `Ctrl+i`/`Tab`, `SpeedUp`/`SlowDown` via `+`/`-`/arrows, localized QWERTY `Ripple` on all normal letters and `Space`, and mouse click/drag/scroll impulses). On Windows, `KeyEventKind::Release` events are filtered out so key presses do not fire twice.

### Configuration, themes, and widgets (`src/config/`, `src/theme/`, `src/widget/`)

- **`config`**: Parses and saves TOML configuration (`load_config`, `save_config`) from XDG, `%APPDATA%`, or macOS Application Support paths, provides the flicker-free interactive keyboard setup wizard (`wizard.rs`, opened on first bare run or via `lavaterm --config` / `--setup`), rejects unknown keys via `#[serde(deny_unknown_fields)]` with file-path diagnostics, validates bounds (such as `target_fps` and `widget` dimensions), and supports legacy field names through Serde `#[serde(alias = ...)]` attributes. See [configuration.md](configuration.md) for the schema.
- **`theme`**: Provides built-in color palettes, custom palette loading, and Pywal/Wallust JSON cache integration.
- **`widget`**: Supports terminal multiplexers (tmux, zellij) and scriptable output via `CompactScaler` (recomputed dynamically on `Event::Resize`), mode-scoped `resolve_policy` (isolating `[widget]` TOML keys from normal interactive sessions), and `render_snapshot_options` (`--snapshot`).

## Concurrency and error handling

- **Main thread**: Runs the terminal event loop, polls input and telemetry/audio providers, steps the simulation, rasterizes `VirtualFramebuffer`, and writes batched ANSI escape sequences to `stdout`.
- **Audio thread**: Managed by `cpal`, pushes incoming PCM samples into `PcmRingBuffer` without locks or allocations on the hot path.
- **No production panics**: All runtime operations return `Result<T, LavaError>` and avoid `unwrap()`, `expect()`, or `panic!()` in production code.

## Verification and benchmarks

The automated test suite contains 187 tests (157 unit tests and 30 integration tests) verified via `cargo test`, alongside headless checks (`cargo run -- --headless --frames 30`) and platform smoke scripts (`scripts/smoke_test.py` on Linux and `scripts/smoke_test_windows.ps1` on Windows). Criterion micro-benchmark measurements for field evaluation, rasterization, renderers, FFT, and Seqlock ring buffer contention are recorded in [benchmark_baseline.md](benchmarks/benchmark_baseline.md).
