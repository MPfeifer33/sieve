# sieve

`sieve` is a test impact analyzer. It looks at changed files and recommends a
small, ranked set of tests to run, with reasons and confidence.

It is meant for the moment before an agent runs validation:

```text
What is the smallest useful test set for this diff?
```

## Quickstart

```sh
cargo build

# Analyze staged, unstaged, or untracked changes.
cargo run -- analyze

# Analyze explicit files.
cargo run -- analyze --file src/claims.rs

# Get machine-readable output.
cargo run -- analyze --file src/claims.rs --format json
```

After installation, replace `cargo run --` with `sieve`.

## Commands

### analyze

```sh
sieve analyze
sieve analyze --base main
sieve analyze --file src/claims.rs
sieve analyze --file src/claims.rs --file src/tasks.rs
sieve analyze --format json
```

`analyze` selects changed files in this order:

1. explicit `--file` arguments
2. staged git diff
3. unstaged diff against `--base` or `HEAD`
4. untracked files

### files

```sh
sieve files
sieve files --base main
sieve files --file src/claims.rs
```

Lists changed files without producing recommendations.

## Signals

`sieve` uses five signal types:

- direct test changes
- source-to-test convention matches
- import/use analysis
- git co-change history
- project-level fallback command

Recommendations are deduplicated by command, scored, and sorted.

Confidence values:

- `certain`
- `high`
- `medium`
- `low`

## JSON Output

```json
{
  "ok": true,
  "changed_files": ["src/claims.rs"],
  "recommendations": [
    {
      "command": "cargo test --test cli_claims",
      "confidence": "high",
      "score": 340,
      "test_files": ["tests/cli_claims.rs"],
      "changed_files": ["src/claims.rs"],
      "reasons": ["Convention: src->cli_test"],
      "signal_kinds": ["convention_match"]
    }
  ],
  "coverage_gaps": [],
  "signals": [],
  "full_validation_command": "cargo test"
}
```

`signals` preserves the raw evidence. `recommendations` is the collapsed,
ranked view most agents should use first.

## Typical Agent Flow

```sh
# 1. Understand project readiness.
probe doctor

# 2. Claim files before editing.
latch claim acquire src/report.rs --intent "recommendation rendering"

# 3. Work.

# 4. Ask sieve for targeted validation.
sieve analyze

# 5. Run recommended tests, then full validation when needed.
cargo test --test cli_claims
cargo test
```

## Design

The implementation contract is in [docs/SPEC.md](docs/SPEC.md).
