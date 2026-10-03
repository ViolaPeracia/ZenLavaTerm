# ZenLavaTerm Documentation Hub

Welcome to the documentation for **ZenLavaTerm** (`lavaterm`), a high-performance terminal-native ambient lava lamp and metaball visualizer written in Rust.

---

## 1. Documentation Hierarchy & Authority

To prevent contradictory guidelines across the repository, the documentation follows a strict precedence order:

```text
Level 1: Working Rust Implementation & Tests
         (Ground truth of actual behavior: src/, tests/, benches/, Cargo.toml)
                       ▲
                       │
Level 2: Authoritative Human Documentation
         (Source of truth for specifications: docs/, README.md, CHANGELOG.md)
                       ▲
                       │
Level 3: Repository Agent Instructions
         (Rules for coding agents: AGENTS.md, .cursor/rules/*.mdc)
                       ▲
                       │
Level 4: AI Supporting Context Cache
         (Workflow scratchpad & context cache: .ai/context/, .ai/tasks/, .ai/decisions/)
```

- **Conflict Resolution**: Human documentation in `docs/` and the compiled codebase are authoritative. AI scratchpads (`.ai/`) must be updated when they diverge from `docs/` and code—never the reverse.
- **Accuracy Guarantee**: All benchmarks, test counts, and platform claims in `docs/` are backed by empirical measurements and automated test results.

---

## 2. Documentation Map

### [Architecture](architecture/)
- [Overview & Pipeline](architecture/overview.md) — Unidirectional data flow, module boundaries, and thread model.
- [Audio Pipeline](architecture/audio-pipeline.md) — SPSC lock-free Seqlock ring buffer, FFT spectrum analyzer, and hardware capture fallbacks.
- [Rendering Pipeline](architecture/rendering-pipeline.md) — Virtual framebuffer, Half-Block / Block / Braille character encodings, ANSI batching.
- [Simulation & Physics](architecture/simulation.md) — Metaball potential field summation, thermal buoyancy, viscosity drag, and user interactions.
- [Reactive Telemetry](architecture/reactive-telemetry.md) — Cross-platform OS telemetry providers (Linux procfs, Windows Win32, macOS Mach kernel).
- [Terminal UI & Widgets](architecture/ui-and-widgets.md) — Terminal-native TUI architecture, multiplexer integration, and distinction from GUI/Tauri frameworks.

### [Development](development/)
- [Getting Started](development/getting-started.md) — Prerequisites, system packages, clone, build, and run instructions.
- [Development Workflow](development/workflow.md) — Branching, commits, quality gates (`cargo fmt`, `cargo clippy`), and test verification.
- [Contributing Guide](development/contributing.md) — Contribution process, PR requirements, and issue creation guidelines.

### [Testing & Performance](testing/)
- [Testing Strategy](testing/strategy.md) — Unit tests, integration tests, mock audio feeders, and headless verification.
- [Benchmarks & Profiling](testing/benchmarks.md) — Criterion micro-benchmarks, baseline latency, and throughput verification (>5,000 FPS).

### [Operations & CI/CD](operations/)
- [Packaging Guide](operations/packaging.md) — Building AppImage, DEB, Windows MSI (WiX), macOS Universal DMG, AUR, and Homebrew.
- [CI/CD Pipelines](operations/ci-cd.md) — GitHub Actions workflows for continuous integration, packaging validation, and release publishing.

### [Troubleshooting](troubleshooting/)
- [Common Issues](troubleshooting/common-issues.md) — Audio capture permissions, Windows headless VM loopback, Linux ALSA headers, and TrueColor support.

### [Reference](reference/)
- [CLI Reference](reference/cli.md) — Full reference for command-line arguments, flags, and environment variables.
- [Configuration Reference](reference/configuration.md) — Comprehensive TOML configuration schema and key specifications.
- [Themes Reference](reference/themes.md) — Color presets, Pywal / Wallust integration, and custom palette file formats.

### [Releases](releases/)
- [Release Process](releases/process.md) — Release engineering playbook, SemVer tag validation, artifact checksums, and manifest updates.
- [Release History](releases/history.md) — High-level milestone history and pointers to [CHANGELOG.md](../CHANGELOG.md).

### [Archive](archive/)
- [Historical Archive](archive/README.md) — Archived design documents, early setup prompts, past audits, and phase plans from Phase 0 through Phase 12.

