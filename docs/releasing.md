# Releasing and CI/CD

ZenLavaTerm uses GitHub Actions for continuous integration, packaging validation, dependency updates, and release publishing.

## Continuous integration

The main CI workflow ([.github/workflows/ci.yml](../.github/workflows/ci.yml)) runs on pushes and pull requests to `main` and `dev`.

Four checks are required on `main`:

1. `Code Quality & Linting (Ubuntu x86_64)`: Runs `cargo fmt --check` and `cargo clippy --all-targets --all-features -- -D warnings`.
2. `Test Suite (Linux x86_64 glibc)`: Runs `cargo test`, `cargo bench --no-run`, and `python3 scripts/smoke_test.py` against the debug binary.
3. `Representative Platform (Windows x86_64 (MSVC))`: Runs `cargo test` and `scripts/smoke_test_windows.ps1` (25 smoke test cases).
4. `Security Vulnerability Audit`: Runs `cargo audit` using a prebuilt binary installed via `taiki-e/install-action`.

CI also runs `Representative Platform (macOS arm64 (Apple Silicon))` (`cargo test`, headless smoke check, Intel cross-target check) and `MSRV Check (Rust 1.86)` (`cargo check --all-targets`).

Two supporting workflows handle PR maintenance:

- [.github/workflows/autofix.yml](../.github/workflows/autofix.yml) runs on `pull_request` events only.
- [.github/dependabot.yml](../.github/dependabot.yml) opens weekly grouped PRs for `cargo-dependencies` and `github-actions`.

## Tagging rules

Production releases are triggered by pushing a Git tag matching `v*` to [.github/workflows/release.yml](../.github/workflows/release.yml).

- **Tag pattern**: Tags must match `^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$`.
- **Version match**: The tag version must match the `lavaterm` package version in `Cargo.toml`.
- **Pre-release tags**: Tags containing `-rc`, `-beta`, or `-alpha` are rejected by `release.yml` and instead trigger packaging validation in [.github/workflows/package.yml](../.github/workflows/package.yml).

## Release artifacts

When a valid release tag is pushed, `release.yml` builds and publishes:

| Platform | Artifact | Build script / tool |
|---|---|---|
| Linux (AppImage) | `ZenLavaTerm-v<VERSION>-linux-x86_64.AppImage` | `scripts/package_linux.sh` |
| Linux (Debian/Ubuntu) | `ZenLavaTerm-v<VERSION>-linux-x86_64.deb` | `scripts/package_linux.sh` (`dpkg-deb`) |
| Windows (MSI) | `ZenLavaTerm-v<VERSION>-windows-x86_64.msi` | `scripts/package_windows.ps1` (WiX) |
| macOS (Universal DMG) | `ZenLavaTerm-v<VERSION>-macos-universal.dmg` | `scripts/package_macos.sh` (`lipo` + `hdiutil`) |
| Homebrew & AUR | `lavaterm.rb`, `PKGBUILD`, `.SRCINFO` | `scripts/update_package_manifests.sh` |

Each platform job writes a `.sha256` sidecar file. In `package.yml`, Linux packages are verified end-to-end with `scripts/smoke_test.py --require-packages` (including extracted DEB and AppImage PTY runs) and Windows MSI installers are tested with silent `msiexec /i` install, execution, and `/x` uninstall checks. The release job verifies every artifact against its sidecar, generates a combined `SHA256SUMS.txt`, enforces SLSA build provenance via `actions/attest-build-provenance`, updates Homebrew and AUR manifests, and publishes the GitHub Release with `softprops/action-gh-release` (`v3.0.4`).

> **Note**: macOS-specific runtime/packaging follow-ups (CoreAudio minimum version alignment, battery telemetry, and Finder launch verification) are tracked separately and deferred from `v2.0.0`.

## Release checklist

1. Update `[package] version` in [Cargo.toml](../Cargo.toml) and run `cargo check` to refresh `Cargo.lock`.
2. Move the release notes from `## [Unreleased]` in [CHANGELOG.md](../CHANGELOG.md) under the new `## [X.Y.Z] - YYYY-MM-DD` heading.
3. Run local validation before tagging:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo run -- --headless --frames 30
```

4. Commit the version and changelog updates, create an annotated tag, and push `main` and the specific release tag explicitly:

```bash
git tag -a vX.Y.Z -m "Release vX.Y.Z"
git push origin main
git push origin vX.Y.Z
```

5. Confirm all jobs in the GitHub Actions `Release` workflow succeed and inspect the generated GitHub Release assets and `SHA256SUMS.txt`.
