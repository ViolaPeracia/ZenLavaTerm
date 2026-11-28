# Troubleshooting

## Audio capture and device selection

List available input and loopback devices before debugging audio-reactive mode:

```bash
lavaterm --list-audio-devices
```

If no capture device is available or initialization fails, LavaTerm falls back to `SyntheticAudioGenerator` so the simulation keeps running.

### Windows WASAPI loopback

To react to desktop audio output rather than a microphone, pass `--audio-loopback`:

```bash
lavaterm --audio --audio-loopback
```

When desktop audio stops playing, WASAPI loopback buffers decay to silence after roughly 120ms. Pausing media or encountering a non-fatal stream pause does not trigger synthetic fallback, so the simulation settles cleanly until playback resumes.

### Linux ALSA

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

## Terminal cursor or raw mode not restored

If LavaTerm is terminated abruptly (for example with `SIGKILL`) and the cursor remains hidden or keystrokes are not echoed, reset the terminal state:

```bash
stty sane
tput cnorm
```

On Windows, closing and reopening the terminal tab also restores normal input state.
