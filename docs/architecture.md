# Architecture

ZenLavaTerm (`lavaterm`) is a terminal-native ambient lava lamp and metaball visualizer written in Rust (Edition 2021, `rustc 1.85+`). It renders directly to a TTY using `crossterm` and ANSI TrueColor escape sequences rather than a GUI toolkit or webview.

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

- **Scalar field (`field.rs`)**: Evaluates metaball potential summation and weighted temperature across normalized 2D coordinates with loop-invariant hoisting.
- **Simulation (`simulation.rs`)**: Steps blob positions, thermal buoyancy, viscosity drag, and user or audio impulses. Audio reactions include edge-detonated bass shockwaves with inward rebound and soft-music radius and temperature shimmer.
- **User baseline preservation**: `Simulation::base_params()`, `Simulation::set_base_params()`, and `Simulation::nudge_buoyancy()` keep configured `[simulation]` parameters and keyboard speed offsets intact across reactive signal polls.

### Audio pipeline (`src/audio/`)

`audio` captures live system or microphone audio and extracts frequency bands without blocking the render loop:

- **Lock-free ring buffer (`ring_buffer.rs`)**: `PcmRingBuffer` uses a single-producer, single-consumer (SPSC) model with a 64-bit Seqlock (`version: AtomicU64`) and an atomic CAS producer guard so readers obtain tear-free snapshots without blocking writers.
- **Spectrum analysis (`fft.rs`)**: Applies a Hann window and computes an in-place Cooley-Tukey Radix-2 FFT to extract bass, mid, and treble energy.
- **Native capture and fallbacks (`native.rs`, `capture.rs`, `provider.rs`)**: Captures audio via `cpal` (WASAPI loopback on Windows, ALSA on Linux, CoreAudio on macOS). Idle capture buffers decay to silence after 120ms without new samples, and non-fatal stream errors (such as pause, `Xrun`, `DeviceChanged`, or `RealtimeDenied`) do not trigger synthetic fallback beats. When no audio device is available, `SyntheticAudioGenerator` supplies deterministic beat signals.

### Reactive telemetry (`src/reactive/`)

`reactive` translates OS resource metrics into normalized `[0.0, 1.0]` signals via the `SystemProvider` trait:

- **Linux**: Reads `/proc/stat`, `/proc/meminfo`, and `/sys/class/power_supply`.
- **Windows**: Queries Win32 APIs (`GetSystemTimes`, `GlobalMemoryStatusEx`, `GetSystemPowerStatus`, `GetProcessIoCounters`).
- **macOS**: Calls Mach kernel APIs (`host_statistics64` with `HOST_CPU_LOAD_INFO` and `HOST_VM_INFO64`).
- **Fallback**: `MockSystemProvider` supplies deterministic telemetry on unsupported platforms or when OS queries fail.

### Rendering (`src/render/`)

`render` discretizes the scalar field into a 2D pixel buffer and encodes ANSI output:

- **Virtual framebuffer (`framebuffer.rs`)**: Holds a contiguous RGB grid and maps field potential and temperature through smooth or stepped color palettes.
- **Sub-cell encoders (`halfblock.rs`, `block.rs`, `braille.rs`)**:
  - `HalfBlockRenderer`: Packs two vertical pixels per cell using `▀` (2x vertical resolution).
  - `BlockRenderer`: Maps one pixel per character cell using `█`.
  - `BrailleRenderer`: Maps a 2x4 subpixel dot matrix per cell using `U+2800`..`U+28FF`.
- **Cursor modes and cleanup**: Renderers support `CursorMode::{Absolute, Inline, Stream}` for fullscreen, inline, or streamed output. `TerminalGuard` provides RAII cleanup in `run_event_loop` to restore raw mode, alternate screen, mouse capture, and cursor visibility on exit or early error.

### Input (`src/input/`)

`input` translates `crossterm` keyboard and mouse events into simulation actions (`SpeedUp`, `SlowDown`, `Reset`, theme/renderer cycling, and mouse drag/click impulses). On Windows, `KeyEventKind::Release` events are filtered out so key presses do not fire twice.

### Configuration, themes, and widgets (`src/config/`, `src/theme/`, `src/widget/`)

- **`config`**: Parses TOML configuration from XDG, `%APPDATA%`, or macOS Application Support paths, validates bounds (such as `target_fps` and `widget` dimensions), and supports legacy field names through Serde `#[serde(alias = ...)]` attributes. See [configuration.md](configuration.md) for the schema.
- **`theme`**: Provides built-in color palettes, custom palette loading, and Pywal/Wallust JSON cache integration.
- **`widget`**: Supports terminal multiplexers (tmux, zellij) and scriptable output via `CompactScaler`, `ExecutionMode`, and `render_snapshot_options` (`--snapshot`).

## Concurrency and error handling

- **Main thread**: Runs the terminal event loop, polls input and telemetry/audio providers, steps the simulation, rasterizes `VirtualFramebuffer`, and writes batched ANSI escape sequences to `stdout`.
- **Audio thread**: Managed by `cpal`, pushes incoming PCM samples into `PcmRingBuffer` without locks or allocations on the hot path.
- **No production panics**: All runtime operations return `Result<T, LavaError>` and avoid `unwrap()`, `expect()`, or `panic!()` in production code.

## Verification and benchmarks

The automated test suite contains 162 tests (138 unit tests and 24 integration tests) verified via `cargo test`, alongside headless checks (`cargo run -- --headless --frames 30`) and platform smoke scripts (`scripts/smoke_test.py` on Linux and `scripts/smoke_test_windows.ps1` on Windows). Criterion micro-benchmark measurements for field evaluation, rasterization, renderers, FFT, and Seqlock ring buffer contention are recorded in [benchmark_baseline.md](benchmarks/benchmark_baseline.md).
