# Configuration and CLI reference

LavaTerm loads settings in three steps: built-in defaults, an optional TOML configuration file, and CLI flags that override file values for the current run.

## Config file discovery

Pass `-c` or `--config <PATH>` to load a specific file. When `--config` is omitted, LavaTerm checks the first existing `config.toml` in the following order per platform:

- **Linux and Unix**:
  1. `$XDG_CONFIG_HOME/lavaterm/config.toml`
  2. `$HOME/.config/lavaterm/config.toml`
- **macOS**:
  1. `$XDG_CONFIG_HOME/lavaterm/config.toml`
  2. `$HOME/Library/Application Support/lavaterm/config.toml`
  3. `$HOME/.config/lavaterm/config.toml`
- **Windows**:
  1. `%APPDATA%\lavaterm\config.toml`
  2. `%USERPROFILE%\AppData\Roaming\lavaterm\config.toml`
  3. `%USERPROFILE%\.config\lavaterm\config.toml`
  4. `$XDG_CONFIG_HOME/lavaterm/config.toml`

If no file is found, LavaTerm runs with its built-in defaults.

## Full `config.toml` reference

The sample below lists every table and key with its built-in default and valid range. Optional keys are commented out.

```toml
[simulation]
blobs = 12                    # 1..=128
gravity = 0.12                # 0.0..=5.0
buoyancy = 0.80               # 0.0..=5.0
viscosity = 0.93              # 0.0..=1.0
noise = 0.15                  # 0.0..=2.0
threshold = 1.00              # 0.1..=10.0
thermal_transfer_rate = 0.40  # >0.0..=5.0

[render]
renderer = "halfblock"        # "halfblock" | "block" | "braille"
fps = 30                      # 1..=240
gradient = true               # smooth color interpolation
glow = true                   # ambient edge aura & bottom furnace glow (auto-off on braille/stepped)
molten_pool = false           # optional undulating lava pool along the bottom edge

[palette]
bottom = "#ff3b00"            # heat source (hot)
middle = "#ff7a00"            # mid-chamber convection
top = "#7b2cff"               # cooled top (cold)
background = "#0d0d15"        # chamber background

[reactive]
enabled = false               # system-reactive mode (CPU/RAM/battery)
poll_interval_ms = 500        # metric polling interval in ms

[audio]
enabled = false               # audio-reactive FFT mode
bpm = 120.0                   # 20.0..=300.0 (synthetic fallback tempo)
# device = "default"          # optional input/capture device name
loopback = false              # capture system output instead of microphone

[theme]
# name = "lava"               # preset name, "auto", "pywal", or "wallust"
# path = "/path/to/theme.json" # optional custom theme file path

[widget]
compact = false               # compact layout scaling (widget/inline/snapshot modes only)
fps = 15                      # 1..=240 (target FPS in widget mode)
inline = false                # render inline without alternate screen (widget/inline modes)
# width = 40                  # optional fixed columns (1..=1000, widget/inline/snapshot modes)
# height = 12                 # optional fixed rows (1..=1000, widget/inline/snapshot modes)
adapt_blobs = true            # scale blob count and physics in compact mode

[interaction]
mouse = true                  # click shockwaves, drag stirring, scroll pressure
keyboard_ripple = true        # ripples on alphanumeric keypresses
shockwave_force = 1.0         # 0.1..=10.0
stir_force = 1.0              # 0.1..=10.0
```

> **Strict schema checking**: Unknown keys in `config.toml` are rejected with an error naming the config file path so typos do not fail silently. Settings in `[widget]` only apply in `--widget`, `--inline`, or `--snapshot` modes and never override a normal interactive `lavaterm` session or lock its viewport size.

### Legacy field aliases

Older config files using earlier key names remain supported through Serde aliases:

- `[simulation]`: `num_blobs` (`blobs`), `gravity_constant` (`gravity`), `buoyancy_force` (`buoyancy`)
- `[render]`: `renderer_type` (`renderer`), `target_fps` (`fps`), `smooth_gradient` (`gradient`)
- `[audio]`: `tempo` (`bpm`)
- `[widget]`: `compact_mode` (`compact`)

## CLI options

Run `lavaterm --help` for the live flag summary from the installed binary. Note that `--system` and `--audio` are mutually exclusive and cannot be combined in the same invocation.

| Flag | Argument | Description |
| :--- | :--- | :--- |
| `-c`, `--config` | `[PATH]` | Without a path, opens the interactive keyboard setup menu to save defaults. With `<PATH>`, loads a custom TOML file. |
| `--setup` | | Open the interactive keyboard setup menu to edit and save default settings. |
| `--no-setup` | | Skip the automatic first-run setup wizard. |
| `-r`, `--renderer` | `<TYPE>` | Renderer backend: `halfblock`, `block`, or `braille`. |
| `--fps` | `<FPS>` | Target frames per second (`1..=240`). |
| `--blobs` | `<COUNT>` | Number of metaball blobs (`1..=128`). |
| `-t`, `--theme` | `<THEME>` | Preset name, `auto`, `pywal`, `wallust`, or theme file path. |
| `--compact` | | Force compact geometry and profile scaling. |
| `--widget` | | Run as a low-overhead ambient widget (`15` FPS default, compact physics). |
| `--inline` | | Render inline in the terminal without entering the alternate screen. |
| `--snapshot` | | Render a single ANSI frame to stdout and exit. |
| `--width` | `<COLS>` | Explicit viewport width in terminal columns (`1..=1000`). |
| `--height` | `<ROWS>` | Explicit viewport height in terminal rows (`1..=1000`). |
| `--system` | | Enable system-reactive mode (CPU, RAM, battery). |
| `--audio` | | Enable audio-reactive mode (FFT bass, mid, treble). |
| `--audio-device` | `<DEVICE>` | Target audio input or capture device name (automatically enables `--audio`; fails fast if device not found). |
| `--audio-loopback` | | Capture desktop output audio via Windows WASAPI loopback (automatically enables `--audio`; fails fast on non-Windows). |
| `--no-audio-fallback` | | Fail immediately if live audio capture cannot start instead of falling back to synthetic beats. |
| `--list-audio-devices` | | List available audio capture devices and exit. |
| `--no-mouse` | | Disable mouse click shockwaves, dragging, and scroll pressure. |
| `--no-ripple` | | Disable keyboard ripples on character keypresses. |
| `--shockwave-force` | `<FORCE>` | Multiplier for mouse click shockwave force (default `1.0`). |
| `--stir-force` | `<FORCE>` | Multiplier for mouse drag stirring force (default `1.0`). |
| `--headless` | | Run simulation without taking over the TTY (for testing or CI). |
| `--frames` | `<FRAMES>` | Number of frames to step in headless mode (default `60`). |
| `-h`, `--help` | | Print help information. |
| `-V`, `--version` | | Print version information. |

## Themes and palettes

LavaTerm resolves themes in this order: `--theme` on the CLI, `[theme]` in `config.toml`, and finally `[palette]` hex colors.

### Built-in presets

Preset lookup is case-insensitive and converts underscores to hyphens (for example `tokyo_night` matches `tokyo-night`). `classic` and `default` are aliases for `lava`.

| Preset | Bottom | Middle | Top | Background |
| :--- | :--- | :--- | :--- | :--- |
| `lava` (`classic`, `default`) | `#ff3b00` | `#ff7a00` | `#7b2cff` | `#0d0d15` |
| `ocean` | `#00f0ff` | `#0077be` | `#0a192f` | `#020b14` |
| `cyberpunk` | `#fcee0a` | `#ff0055` | `#7122fa` | `#05050d` |
| `synthwave` | `#ff2a85` | `#9a48d0` | `#2de2e6` | `#120b22` |
| `nord` | `#88c0d0` | `#5e81ac` | `#81a1c1` | `#2e3440` |
| `forest` | `#55ff77` | `#2e8b57` | `#1b4332` | `#081c15` |
| `monochrome` | `#ffffff` | `#999999` | `#444444` | `#0a0a0a` |
| `matrix` | `#a6ff00` | `#00ff41` | `#003b00` | `#0d1117` |
| `sunset` | `#ff4500` | `#ff8c00` | `#4a0e4e` | `#1a0022` |
| `dracula` | `#ff79c6` | `#bd93f9` | `#8be9fd` | `#282a36` |
| `catppuccin` | `#f5c2e7` | `#cba6f7` | `#89b4fa` | `#1e1e2e` |
| `tokyo-night` | `#f7768e` | `#bb9af7` | `#7aa2f7` | `#1a1b26` |

### Dynamic theme extraction

Three special theme names read desktop color caches at startup:

- `pywal`: reads Pywal's `colors.json` cache (falling back to `lava` if unavailable).
- `wallust`: reads Wallust's `colors.json` cache (falling back to `lava` if unavailable).
- `auto`: checks Pywal and Wallust caches automatically and falls back to `lava` if neither is present.

### Custom theme files

You can point `--theme` or `[theme].path` at a custom file in any of four formats:

1. **Direct JSON** (requires at least `bottom` or `top`; omitted keys fall back to `lava` defaults):

```json
{
  "bottom": "#00ffcc",
  "middle": "#0077ff",
  "top": "#ff0077",
  "background": "#000011"
}
```

2. **Direct TOML** (same keys and requirement as direct JSON):

```toml
bottom = "#00ffcc"
middle = "#0077ff"
top = "#ff0077"
background = "#000011"
```

3. **Pywal or Wallust `colors.json`**: standard JSON exports produced by `wal` or `wallust`.
4. **Flat 16-color hex text**: a plain text file with one `#rrggbb` color per line (up to 16 lines).
