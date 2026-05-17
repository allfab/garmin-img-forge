# Repository Guidelines

## Project Structure & Module Organization

This repository combines Garmin map pipeline scripts, native tools, and a documentation site. `scripts/` contains public pipeline entry points plus `typ/`, `release/`, `ops/`, `dev/`, and `debug/` helpers. `pipeline/` holds configuration, resources, generated data, and outputs; do not commit local `.env` secrets or generated bulk data from `pipeline/data/` or `pipeline/output/`. `tools/` contains Rust tools (`mpforge`, `imgforge`, `typforge`, `garmin-routing-graph`) and CMake/GDAL tools (`ogr-polishmap`, `ogr-garminimg`). `site/` contains the bilingual Zensical documentation site. Tests live beside each tool under `tools/*/tests` or `tools/*/test`.

## Build, Test, and Development Commands

- `./scripts/check_environment.sh`: verify required system tools when present in the checkout.
- `./scripts/download-data.sh --zones D038 --dry-run`: validate BDTOPO download configuration without fetching data.
- `./scripts/build-garmin-map.sh ...`: build a Garmin map through the production pipeline; see `scripts/README.md` for required options.
- `cargo build --release --manifest-path tools/mpforge/Cargo.toml`: build one Rust tool; replace the manifest path for `imgforge`, `typforge`, or `garmin-routing-graph`.
- `cargo test --manifest-path tools/mpforge/Cargo.toml`: run Rust tests for a tool.
- `cmake -S tools/ogr-garminimg -B tools/ogr-garminimg/build && cmake --build tools/ogr-garminimg/build`: configure and build a C++ GDAL plugin.
- `cd site && ./build-site.sh --serve`: build and serve the FR/EN documentation locally.

## Coding Style & Naming Conventions

Follow `.editorconfig`: UTF-8, LF endings, final newline, spaces by default, 4-space indents for Rust, Python, C/C++, and shell; 2-space indents for Markdown, YAML, TOML, JSON, CMake, and config files. Use `cargo fmt` and `cargo clippy` for Rust changes. Shell scripts should use `set -euo pipefail`; prefer POSIX `sh` unless Bash features are needed. Keep pipeline configuration names descriptive, for example `sources.yaml`, `garmin-rules.yaml`, and department IDs such as `D038`.

## Testing Guidelines

Run the narrowest relevant test suite before submitting. Rust integration tests are under `tools/<tool>/tests` and use Cargo. C++ tools provide CMake-built test executables in each `build/` directory, with additional regression scripts under `tools/ogr-polishmap/test/`. Python tests follow `test_*.py`, for example `scripts/typ/test_build_ign_bdtopo_typ.py`.

## Commit & Pull Request Guidelines

Recent history uses concise conventional commits, often in French: `fix(routing): ...`, `chore: ...`, `docs(site): ...`. Prefer `type(scope): description` with `feat`, `fix`, `docs`, `chore`, or `refactor`. PRs should include context, a short change list, linked issues when applicable, and the local build/test commands run. This repository is a public mirror; accepted PRs are merged upstream in Forgejo and then reflected back here, so follow `CONTRIBUTING.md` and avoid excluded paths such as `_bmad/`, `docs/`, `.woodpecker/`, local IDE config, and `.env*` files.
