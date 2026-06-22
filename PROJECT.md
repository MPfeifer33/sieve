# PROJECT.md — sieve

**What:** Test impact analyzer. Given changed files, recommends targeted test
commands with confidence, reasons, raw signals, and a full validation fallback.

**Status:** MVP implemented. Analyzer pipeline, recommendation rendering,
docs, and integration tests are complete.

**Tech:** Rust 2021, clap 4, serde/serde_json, regex, thiserror.

## Module Ownership

| Module | Owner | Status |
| ------ | ----- | ------ |
| cli.rs | Nix | Done |
| main.rs | Nix | Done |
| detect.rs | Nix | Done |
| diff.rs | Nix | Done |
| imports.rs | Nix | Done |
| history.rs | Nix | Done |
| conventions.rs | Nix | Done |
| analyze.rs | Nix | Done |
| report.rs | Bjarn | Done |
| docs/SPEC.md | Bjarn | Done |
| README.md | Bjarn | Done |

## Build

```sh
cargo build
cargo check
cargo test
```

## Usage

```sh
sieve analyze                         # analyze staged/unstaged/untracked files
sieve analyze --file src/claims.rs    # analyze explicit path
sieve analyze --format json           # structured recommendations
sieve files                           # list changed files only
```

## Signals

- `direct_test_change`: changed file is itself a test
- `convention_match`: source path maps to a test path by convention
- `import_match`: test imports the changed module
- `history_correlation`: git history shows source and test co-change
- `fallback`: project-level test command

## Last Updated

2026-06-22 — MVP complete; `cargo test` passes with 6 tests.
