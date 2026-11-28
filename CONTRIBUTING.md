# Contributing to LavaTerm

Contributions are welcome. Keep changes focused, preserve the module boundaries, and verify all checks locally before opening a pull request.

## Development setup

- Rust 1.86+ (`rustup default stable`)
- `rustfmt` and `clippy` (`rustup component add rustfmt clippy`)
- Linux only: `libasound2-dev` and `pkg-config` for ALSA audio capture

## Validation

Run these commands after making changes:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo run -- --headless --frames 30
```

## Workflow

1. Create a branch using one of these prefixes:
   - `feat/<name>`
   - `fix/<name>`
   - `refactor/<scope>`
   - `docs/<topic>`
2. Write commit messages using Conventional Commits:
   ```text
   feat(core): add thermal pulse decay
   fix(render): clamp braille cell bounds
   docs: update troubleshooting steps
   ```
3. Open a pull request against `main`. Note that `autofix.ci` runs on pull requests to apply formatting and lint fixes automatically.

## Architecture and rules

- Keep `src/core/` pure: do not import `crossterm`, OS telemetry, or audio APIs there.
- Return `Result<T, LavaError>` on runtime paths; do not use `unwrap()`, `expect()`, or `panic!()` in production code.
- See [docs/architecture.md](docs/architecture.md) for the data pipeline and [AGENTS.md](AGENTS.md) for repository-wide rules and validation requirements.
