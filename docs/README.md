# ZenLavaTerm documentation

Index of documentation for ZenLavaTerm (`lavaterm`, crate version `1.0.1`).

## Authority hierarchy

When resolving conflicting details, follow this order from highest to lowest authority:

1. `src/` and `tests/` (compiled implementation and automated tests)
2. `docs/`, `README.md`, and `CHANGELOG.md` (human documentation)
3. `AGENTS.md` (repository agent instructions)
4. `.ai/` (supporting context cache)

## Documents

- [configuration.md](configuration.md): CLI flags, TOML configuration schema, theme presets, and custom palettes.
- [architecture.md](architecture.md): Unidirectional data pipeline, physics model, renderers, lock-free audio ring buffer, and OS telemetry.
- [troubleshooting.md](troubleshooting.md): Audio device setup, Windows WASAPI loopback, Linux ALSA, macOS permissions, color support, and terminal recovery.
- [releasing.md](releasing.md): Packaging targets, release checklist, and verification steps.
- [benchmarks/benchmark_baseline.md](benchmarks/benchmark_baseline.md): Criterion micro-benchmark baseline measurements.
