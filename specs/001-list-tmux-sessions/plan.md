# Implementation Plan: List tmux Sessions

**Feature selection**: `001-list-tmux-sessions` | **Planning date**: 2026-11-09
| **Spec**: [spec.md](spec.md) | **Actual Git branch during planning**: `master`

**Input**: The feature specification; Rust, clap, ratatui, formatting and
Clippy are required. This is a design, not an implementation or test report.

## Summary

Build a read-only Rust session browser using a shared tmux acquisition layer for
the CLI and a two-screen TUI: session list with automatic detail crawl, and
help. The TUI automatically refreshes and supports only `?`, `Esc`, `q`, `j`,
and `k` plus normal signal handling. Both interfaces expose the same session
fields and report unavailable data and retrieval failures explicitly.

## Technical Context

**Language/toolchain**: Rust 1.99.0, edition 2024, pinned with rustfmt and
Clippy. The manifest and lockfile define the current package and resolved
dependency versions.

**Dependencies**: clap 4.6 with derive; ratatui 0.30 and Crossterm 0.29;
serde/serde_json; unicode-width and unicode-segmentation; signal-hook and libc.
Test-only tempfile and portable-pty. Use a single Cargo package and locked
releases. Runtime requires no separately installed command except tmux; linked
libraries and normal OS services are allowed.

**Storage**: Process-local snapshots only. No configuration file, persistence,
network service, session mutation, or cross-server aggregation.

**Platforms**: The first release supports Linux and FreeBSD. Validate the
session-listing acceptance scenarios on Linux with automated checks where
feasible; FreeBSD testing is not required for release for now. macOS and other
Unix-like systems are best effort and do not require release validation.
The tmux client and any running server must each be at least final 3.3; see
[CLI contract](contracts/cli.md).

**Testing**: Test-first Rust unit and integration tests, controlled subprocess
fixtures, Ratatui TestBackend, isolated real-tmux servers, PTY lifecycle tests,
automated Linux acceptance and measured trials. The five-person help-only
study and non-automatable manual acceptance are deferred; constitution-required
reviews and coverage-exception checks remain. FreeBSD tests are optional.

**Performance and constraints**: With up to 100 sessions, initial display or an
explicit error within 2 seconds in at least 19 of 20 trials per interface;
automatic changes visible within 5 seconds in at least 19 of 20 trials at 120+
columns; resize usable within 1 second in at least 19 of 20 trials. A session
request is cancelled after 5 seconds; these measured targets are not an
additional 2-second hard timeout. At most one request is outstanding. There is
no fixed session-count or tmux-output byte limit. Recoverable resource
exhaustion must fail explicitly; an OS-killed process cannot promise a diagnostic.

**Quality gates**: A commit runs non-mutating formatting and lint checks against
the exact staged content, even with divergent unstaged changes; an explicit local
commit bypass is allowed. Merge and release require formatting, linting, tests
and build, with no bypass. Unavailable or failed checks block the relevant
action. Ordinary local builds remain ungated. Repository commands:

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
cargo build --locked
```

The staged-snapshot mechanism must be tested rather than assuming working-tree
commands check staged content. Hosted merge and release enforcement remains
to be verified as recorded in [acceptance.md](acceptance.md).

## Constitution Check

Pre-research: **PASS by design**. Post-design: **PASS by design**, subject to
implementation and validation; no exception requested.

| Principle or gate | Design evidence |
|---|---|
| I. Focused Unix Simplicity | One package; read-only list and help; shared snapshot; no speculative configuration or framework. |
| II. Help Is the Complete User Guide | Root and command help cover entry points, fields, defaults, errors and recovery; TUI help covers every supported key, crawl and refresh. Reviewers verify the acceptance workflow can be completed using built-in help alone. |
| III. Intuitive Terminal Experience | Consistent CLI/TUI values, explicit loading/empty/partial/stale/failure states, non-color status markers, accessible help and exit. |
| IV. Test-First Confidence | Test observable behavior first where feasible; for each coverage exception identify the limitation and reproducible manual checks as required by the constitution. FreeBSD runtime validation is not required and must not be claimed without evidence. |
| V. Opinionated Defaults | TUI only on attached terminals; explicit scriptable listing; no persistent configuration; documented color controls. |
| Streams, statuses and restoration | Specified stdout/stderr and status rules, no prompts, signal-aware cancellation, terminal restoration, suspension and cleanup-failure reporting. |
| Workflow quality | Staged-content commit checks and non-bypassable merge/release checks as above; no formatting rewrite during checks. |

These are design assessments, not evidence that checks or platform tests passed.

## Project Structure

```text
specs/001-list-tmux-sessions/
|-- spec.md, plan.md, research.md, data-model.md, quickstart.md
`-- contracts/cli.md, contracts/tui.md

Cargo.toml, Cargo.lock, rust-toolchain.toml
.github/workflows/ci.yml
.githooks/, scripts/
src/
|-- main.rs, lib.rs, cli.rs, model.rs, tmux.rs, output.rs
`-- tui/mod.rs, state.rs, view.rs
tests/
|-- cli_contract.rs, tmux_integration.rs, tui_render.rs, quality_gates.rs
|-- terminal_lifecycle.rs, interface_parity.rs
`-- support/
```

Use one package and an injectable process boundary, not a plugin framework.
The repository already contains application code, tests, and `tasks.md`;
this planning pass does not regenerate tasks or establish implementation
completion.

## Phase 0: Research

See [research.md](research.md) for choices, sources, alternatives and evidence
limits. Query the client version independently from the server version; avoid
creating a server. Select the inherited server inside tmux and the default
outside; fail instead of silently falling back when inherited context is
unresolvable. Do not equate pane membership, attached status or tmux's activity
heuristic with the invoking client. If the invoking client cannot be reliably
identified, mark active status unavailable and explain why, as clarified in
the specification.

Real-tmux verification remains mandatory. Prior Linux and FreeBSD runs and
their evidence boundaries are recorded in [acceptance.md](acceptance.md);
measurements are in [performance-results.md](performance-results.md).
These do not replace outstanding final-revision automated acceptance and hosted
gate evidence. A participant study is deferred, not passed.

## Phase 1: Design

1. Parse help, version and arguments before terminal changes or tmux requests.
2. Resolve and verify the selected server context and both required versions;
   fetch and validate metadata and names within one 5-second request deadline.
   Strict identity failure fails retrieval; individual invalid fields are
   unavailable in a partial result. Do not silently truncate output.
3. Use a shared model and numeric-ID ordering for CLI and TUI. Emit six flat
   JSON keys with independent `null` values; human output escapes controls.
4. A single worker supplies automatic refreshes without blocking TUI navigation.
   Initially schedule regular refreshes at 1-second monotonic intervals, and
   tune only as needed to meet the measured target. Skip overlapping scheduled
   requests; after failure retry 5 seconds after it completes, skipping
   intervening regular refreshes. Partial results retry on the next regular
   refresh.
5. Focus remains on the same ID across refresh; on removal select the next
   higher numeric ID or nearest lower. Crawl each focused session's complete
   details at one cell per second, preserving the current pass across refresh;
   a new pass uses current values. Retain a partial warning until any old
   partial-data pass ends.
6. On exit or signal cancel work, restore the terminal, and report cleanup
   errors. Cleanup has no fixed deadline; repeated exit requests show progress.
   Restore before Ctrl-Z suspension and redraw on resume.

See [data-model.md](data-model.md), [CLI contract](contracts/cli.md),
[TUI contract](contracts/tui.md), and [quickstart.md](quickstart.md).

## Implementation and Verification Sequence

1. Establish package, toolchain, exact-staged commit checks, merge/release gates,
   and test harness. Test staging divergence, failed checks and allowed bypass.
2. Test version parsing, selected-server safety, unavailable client identity,
   strict framing, races, deadlines and isolated real-tmux interactions. Benchmark
   100-session acquisition early; optimize if needed without limiting results.
3. Implement and test CLI schema, streams, partial/fatal outcomes, colors and
   signals. Use structured argument arrays rather than shell interpolation.
4. Implement TUI state, two-screen navigation, refresh/backoff, resize/crawl and
   partial warnings; cover rendering with TestBackend and lifecycle with PTYs.
5. Run required gates and record automated Linux acceptance and performance
   results for both interfaces. Confirm session and window state remains
   unchanged. Record coverage exceptions under the constitution. FreeBSD testing
   is optional and is not a release gate.

| Requirements | Planned verification |
|---|---|
| FR-001–004, FR-019–021 | Isolated tmux, version skew, selected server, ambiguity and field-validation tests on Linux |
| FR-005–008 | Cell-accurate layout/crawl, refresh, focus, backoff and resize tests |
| FR-009–017 | CLI contract/stream/signal tests; automated help and PTY workflow checks, read-only scenarios, and reviewer verification of help-only acceptance |
| FR-018; SC-007 | Commit staging divergence, unavailable/failed gates, bypass and non-mutating formatting tests |
| SC-001–006, SC-008–010 | Recorded automated Linux acceptance and performance; cross-interface, help, schema and lifecycle checks on Linux. The human usability study is deferred. |

No phase-1 design introduces a constitution exception. Actual checks, platform
support and performance remain to be demonstrated by implementation.
