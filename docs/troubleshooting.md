# Troubleshooting

## Audio capture and device selection

List available input and loopback devices before debugging audio-reactive mode:

```bash
lavaterm --list-audio-devices
```

If no default capture device is available or initialization fails, LavaTerm falls back to `SyntheticAudioGenerator` and displays an automatic `[audio: synthetic fallback]` badge in the bottom-right corner so you know it is not reacting to live audio. Press `Ctrl+i` (or `Tab`) at any time to display the fading bordered top-left status HUD showing the active audio source (`System Audio`, `Input Device`, or `Synthetic`). Pass `--no-audio-fallback` to fail fast instead of falling back.

Specifying `--audio-device <NAME>` or `--audio-loopback` automatically enables audio mode and fails fast if the requested device does not exist or loopback is unavailable on your OS.

### Windows WASAPI loopback

On Windows, desktop audio output capture is supported natively via WASAPI loopback (supporting `F32`, `I16`, `U16`, `I32`, and `F64` endpoints with Adaptive Auto-Gain Control across quiet and loud playback volumes):

```bash
lavaterm --audio-loopback
```

When desktop audio stops playing, WASAPI loopback buffers decay smoothly to silence between 40ms and 140ms. Pausing media or encountering a non-fatal stream pause does not trigger synthetic fallback, so the simulation settles cleanly until playback resumes. If you switch default output devices (such as unplugging headphones) mid-session, WASAPI invalidates the active stream and LavaTerm transitions to the synthetic fallback badge — restart `lavaterm --audio-loopback` to bind to the new device.

### Linux ALSA & PipeWire / PulseAudio monitors

`--audio-loopback` is currently Windows-only (WASAPI). On Linux, capture desktop audio by selecting a PipeWire or PulseAudio monitor device with `--audio-device`:

```bash
lavaterm --list-audio-devices
lavaterm --audio-device "Monitor of ..."
```

Building from source on Linux requires ALSA development headers and `pkg-config`:

```bash
sudo apt install libasound2-dev pkg-config
```

At runtime, if permission to open the audio device is denied, verify that your user belongs to the `audio` group (or that PipeWire/PulseAudio ALSA compatibility is active):

```bash
groups "$USER"
```

### macOS permissions and build tools

On macOS, `--audio` requires Microphone permission for your terminal application under **System Settings > Privacy & Security > Microphone**.

Source builds require Xcode Command Line Tools:

```bash
xcode-select --install
```

## `--system` and `--audio` cannot be combined

`--system` and `--audio` are mutually exclusive CLI flags because both drive the simulation modulation channels. Run one mode at a time:

```bash
lavaterm --system
# or
lavaterm --audio
```

## Washed-out colors or missing glyphs

LavaTerm targets 24-bit True Color terminals. Check that `COLORTERM` is set to `truecolor` or `24bit`:

```bash
echo "$COLORTERM"
```

If your terminal or font cannot render half-block (`▀`) or Braille (`U+2800`..`U+28FF`) characters cleanly, switch to the full-block renderer:

```bash
lavaterm --renderer block
```

## Kitty Graphics or Sixel output looks blank or prints raw escape text

High-resolution graphics renderers require terminal support for their protocol:

- `-r kitty` requires a Kitty Graphics Protocol terminal (**Kitty**, **Ghostty**, or **WezTerm**).
- `-r sixel` requires a DEC Sixel-capable terminal (**Windows Terminal**, **WezTerm**, **Foot**, **Konsole**, **iTerm2**, or **XTerm**).

If your current terminal does not support the selected graphics protocol (or `tmux` is not configured with passthrough), press `Ctrl+o` during a session or pass `-r halfblock` to return to universal Unicode TrueColor rendering.

## Terminal cursor or raw mode not restored

If LavaTerm is terminated abruptly (for example with `SIGKILL`) and the cursor remains hidden or keystrokes are not echoed, reset the terminal state:

```bash
stty sane
tput cnorm
```

On Windows, closing and reopening the terminal tab also restores normal input state.
