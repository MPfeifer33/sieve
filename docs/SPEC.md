# sieve spec

Status: MVP implementation contract

`sieve` is a test impact analyzer. Given changed files, it recommends the
smallest useful test set, explains why each command was selected, and keeps a
full validation command available for final confidence.

## Goals

- Reduce wasted test time by recommending targeted validation commands.
- Explain every recommendation with provenance.
- Preserve raw signals for agents that want to inspect the underlying evidence.
- Work locally without services or network access.
- Support common Rust, Node, Python, and Go project conventions.

## Non-Goals

- Perfect coverage analysis.
- Running tests automatically.
- Replacing full validation before release.
- Building a full semantic code graph. That belongs to a future `atlas`-style
  tool.

## Commands

### analyze

```sh
sieve analyze
sieve analyze --base main
sieve analyze --file src/claims.rs
sieve analyze --format json
```

Input selection:

- explicit `--file` values win
- otherwise staged git diff
- otherwise unstaged diff against `--base` or `HEAD`
- otherwise untracked files

### files

```sh
sieve files
sieve files --base main
sieve files --file src/claims.rs
```

Lists changed files without analysis. Useful for debugging diff selection.

## Signals

Analyzer signals are raw evidence. Report rendering collapses them into ranked
recommendations.

```json
{
  "kind": "convention_match",
  "changed_file": "src/claims.rs",
  "test_file": "tests/cli_claims.rs",
  "command": "cargo test --test cli_claims",
  "confidence": "high",
  "reason": "Convention: src->cli_test"
}
```

Signal kinds:

- `direct_test_change`: changed file is itself a test
- `convention_match`: path/name convention maps source to test
- `import_match`: test imports the changed module
- `history_correlation`: git history shows source and test co-change
- `fallback`: project-level test command when no targeted test is found

Confidence values:

- `certain`
- `high`
- `medium`
- `low`

## Recommendation Schema

`sieve analyze --format json` returns:

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

Recommendations are deduplicated by command and sorted by score descending.
Multiple signals can strengthen the same recommendation.

Scoring:

| Confidence | Base Score |
| ---------- | ---------- |
| `certain` | 400 |
| `high` | 300 |
| `medium` | 200 |
| `low` | 100 |

Signal bonuses:

| Signal | Bonus |
| ------ | ----- |
| `direct_test_change` | 50 |
| `convention_match` | 40 |
| `import_match` | 40 |
| `history_correlation` | 20 |
| `fallback` | 0 |

## Coverage Gaps

Coverage gaps identify changed files without a targeted test-file signal.
Fallback recommendations can still exist.

```json
{
  "changed_file": "src/orphan.rs",
  "reason": "No targeted test file found; using project-level fallback if available"
}
```

## Text Output

Text output is optimized for direct agent reading:

```text
sieve: 1 changed file(s)

  Recommendations:
    [high score=340] cargo test --test cli_claims
      tests: tests/cli_claims.rs
      changes: src/claims.rs
      reason: Convention: src->cli_test

  Full validation: `cargo test`

  Raw signals: 1
```

## Exit Codes

| Code | Meaning |
| ---- | ------- |
| `0` | Success |
| `1` | Validation or JSON error |
| `2` | IO error |

No recommendations is not a process failure. Consumers should inspect
`recommendations` and `coverage_gaps`.
