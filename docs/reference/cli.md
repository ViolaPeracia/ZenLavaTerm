# Command-Line Interface (CLI) Reference

ZenLavaTerm provides an extensive set of CLI options to configure renderers, frame rates, physics, telemetry, and execution modes.

---

## 1. Synopsis

```bash
lavaterm [OPTIONS]
```

---

## 2. Command-Line Options

| Option | Value | Default | Description |
|---|:---:|:---:|---|
| `-c, --config` | `<PATH>` | Platform default | Path to custom TOML configuration file. |
| `-r, --renderer` | `halfblock \| block \| braille` | `halfblock` | Terminal renderer backend: Unicode half-block, full-block, or Braille matrix. |
| `--fps` | `<U32>` | `30` (or `15` in widget) | Target frames per second ($1 \le \text{fps} \le 240$). |
| `--blobs` | `<USIZE>` | `12` | Number of metaball blobs in the simulation ($1 \le \text{blobs} \le 128$). |
| `-t, --theme` | `<THEME>` | `lava` | Theme preset name (`lava`, `ocean`, `cyberpunk`, etc.), `auto`, `pywal`, `wallust`, or path to custom JSON/TOML palette file. |
| `--compact` | Flag | `false` | Force compact geometry and profile scaling (recommended for small panes). |
| `--widget` | Flag | `false` | Run in low-overhead ambient widget mode (defaults to 15 FPS and compact physics). |
| `--inline` | Flag | `false` | Run inline in the terminal without entering the alternate screen buffer. |
| `--snapshot` | Flag | `false` | Render a single ANSI frame to stdout and exit immediately without TTY mode. |
| `--width` | `<COLS>` | Terminal cols | Explicit viewport width in columns. |
| `--height` | `<ROWS>` | Terminal rows | Explicit viewport height in rows. |
| `--system` | Flag | `false` | Enable ambient system-reactive telemetry mode (CPU, RAM, Battery). |
| `--audio` | Flag | `false` | Enable audio-reactive visualizer mode (FFT bass, mid, treble response). |
| `--audio-device` | `<DEVICE>` | `default` | Name of specific audio capture device endpoint. |
| `--audio-loopback` | Flag | `false` | Capture system audio output (loopback) instead of microphone (Windows WASAPI). |
| `--list-audio-devices` | Flag | - | List available audio capture endpoints and exit. |
| `--no-mouse` | Flag | `false` | Disable mouse click shockwaves, dragging, and scroll pressure. |
| `--no-ripple` | Flag | `false` | Disable keyboard ripples on character keypresses. |
| `--shockwave-force` | `<FORCE>` | `1.0` | Multiplier for mouse click shockwave force. |
| `--stir-force` | `<FORCE>` | `1.0` | Multiplier for mouse drag stirring force. |
| `--headless` | Flag | `false` | Run headless simulation loops without terminal output (for CI testing). |
| `--frames` | `<USIZE>` | `60` | Total frames to execute before auto-terminating (used with `--headless`). |
| `-h, --help` | Flag | - | Print help information. |
| `-V, --version` | Flag | - | Print version information. |

---

## 3. Interactive Keybindings & Controls

When running in normal interactive mode:

| Key / Input | Action |
|---|---|
| `q`, `Q`, `Esc`, `Ctrl+c` | Clean exit (restores terminal state and cursor) |
| `Space`, `p`, `P` | Pause / resume fluid simulation |
| `r`, `R` | Reset blob positions, velocities, and temperatures |
| `+` / `=` / `↑` / `→` | Speed Up (increase upward convective buoyancy by `+0.1`) |
| `-` / `_` / `↓` / `←` | Slow Down (decrease upward convective buoyancy by `-0.1`) |
| `Left Click` | Trigger radial fluid shockwave at mouse coordinates |
| `Click + Drag` | Stir fluid and transfer directional velocity |
| `Right Click` | Thermal pulse (heats nearby blobs) |
| `Scroll Wheel` | Modulate vertical convective fluid pressure |
| Typing Keys | Induce harmonic wave ripples across the column |
