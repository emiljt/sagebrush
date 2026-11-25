---
description: "Implementation tasks for the read-only tmux session browser"
---

# Tasks: List tmux Sessions

**Input**: `specs/001-list-tmux-sessions/{spec.md,plan.md,data-model.md,research.md,quickstart.md,contracts/}`

**Tests**: The specification and constitution require automated, test-first verification where feasible. Write each test group before its corresponding implementation and observe the intended failure. Record any impractical test-first or automated-coverage exception and reproducible alternative in implementation review evidence.

**Format**: `[P]` identifies tasks that can run concurrently in the same dependency wave, with different files. Story labels identify the increment; paths are repository-relative. Stable historical IDs are retained; follow the dependency graph rather than numeric order for convergence tasks.

**Status reconciliation**: Existing task IDs are retained where applicable.
The former participant-study tasks are deferred, not passed; the user-reported
exercise lacks recorded outcomes and times. T042 now covers automated help and
workflow checks instead. Constitutional review and coverage-exception checks
remain required.

## Phase 1: Setup

**Purpose**: Establish the single Rust package and test infrastructure.

- [X] T001 Create the single Rust 2024 package with compatible clap 4.6 derive, ratatui 0.30/Crossterm, serde/serde_json, unicode-width, unicode-segmentation, signal-hook, libc, tempfile and portable-pty dependencies; resolve and lock versions in `Cargo.toml` and `Cargo.lock`.
- [X] T002 [P] Pin Rust 1.99.0 and the rustfmt and clippy components in `rust-toolchain.toml`.
- [X] T003 [P] Create isolated process, temporary tmux socket, controllable clock, and PTY test helpers in `tests/support/mod.rs`; never operate on a user's server.
- [X] T004 Create minimal package and module entry points in `src/main.rs` and `src/lib.rs` so the locked package and test harness compile.

## Phase 2: Foundational (blocks all stories)

**Purpose**: Implement and verify the shared, read-only snapshot boundary and quality gates.

### Tests first

- [X] T005 [P] Add staged-snapshot gate tests in `tests/quality_gates.rs` for divergent staged/unstaged files, missing and failing checks, non-mutating formatting, explicit commit bypass, and non-bypassable merge/release failures.
- [X] T006 [P] Add shared-model tests in `tests/model_contract.rs` for numeric ID order, invalid/duplicate IDs, field-level unavailability, independent dimensions, exact names and control escaping, future timestamps, and uptime boundaries.
- [X] T007 [P] Add controlled subprocess tests in `tests/tmux_process.rs` for client/server version parsing and skew, socket selection, absent versus inaccessible server, ambiguous active context, malformed framing, disappearing sessions, timeout/cancellation, pipe draining, resource errors, and child reaping.
- [X] T008 [P] Add isolated real-tmux tests in `tests/tmux_integration.rs` for default and inherited non-default servers, no-server non-creation, attached/detached sessions, current-window sizes, special names, shared panes/client ambiguity, races, and unchanged tmux state; fail release validation if tmux is unavailable.

### Implementation

- [X] T009 Implement non-mutating format/lint checks against an isolated exact-index snapshot, explicit local bypass, and actionable failures in `scripts/check-staged.sh` and `.githooks/pre-commit`; document installation and bypass in `README.md`.
- [X] T010 Configure required merge and release checks for `cargo fmt --all -- --check`, locked clippy, tests and build, including real-tmux/PTY prerequisites, in `.github/workflows/ci.yml`; document the required hosted branch rules and release environment settings in `README.md`.
- [X] T011 Implement shared identifiers and snapshot types in `src/model.rs`: `SessionId` is “tmux `$` followed by decimal digits; stable within this server lifetime, never a row number/name”; `WindowId` is “tmux `@` followed by decimal digits; current window identity”; `Session.id` is “Required unique ID within a snapshot”; order IDs numerically and reject invalid or duplicate required IDs.
- [X] T012 Implement session fields and issues in `src/model.rs`: name is “Full decoded UTF-8 name, no trimming/normalization/truncation”; window count is “At least one for a surviving session”; creation time is “Nonnegative Unix creation timestamp from tmux”; current window is “Current window identity and individually available dimensions”; its width and height are “Independently available and greater than zero; not pane or viewport dimensions”; active is “Match with invoking context; never attachment count or row focus”. Preserve `Known`/`Unavailable` independently and reject invalid UTF-8 without lossy decoding.
- [X] T013 Implement snapshot capture, contextual diagnostics, clock-mismatch handling, whole-second uptime and the specified `d/h/m/s` format in `src/model.rs`; preserve monotonic elapsed time for the TUI and never fabricate missing values.
- [X] T014 Implement an injectable tmux subprocess boundary in `src/tmux.rs` with argument arrays, concurrent pipe draining, one 5-second deadline for the entire request, cancellation and child/reader cleanup, no fixed byte/count cap, and explicit resource errors.
- [X] T015 Implement selected-server resolution and independent client/server version checks in `src/tmux.rs`: accept final 3.3 and lettered patches, reject 3.3 prereleases, distinguish missing server from access/protocol errors, never start a server or fall back from invalid inherited context, and diagnose unverifiable versions.
- [X] T016 Implement delimiter-safe metadata and exact-ID name acquisition, strict validation, disappearance reconciliation, complete/partial/fatal outcomes, and numeric ordering in `src/tmux.rs`; establish invoking-client identity only when reliable, otherwise mark active unavailable with a diagnostic.
- [X] T017 Measure initial shared acquisition with 100 isolated sessions using `tests/tmux_integration.rs`; retain raw timings and optimize `src/tmux.rs` if needed without limiting results or weakening correctness.

**Checkpoint**: Shared data and acquisition are independently verifiable; both interfaces consume the same outcome. Local staged checks and the CI workflow are in place. Hosted protections are configured and verified in T045 before merge or release, but do not block story implementation.

## Phase 3: User Story 1 - Inspect sessions interactively (P1, internal milestone)

**Goal**: Open the read-only session browser, inspect all sessions and details, navigate and recover from refresh failures, and exit safely.

**Independent test**: With several known isolated tmux sessions, compare names, counts, uptime, current-window sizes and active/unavailable status; use `j`/`k`, observe the detail crawl, open `?` help, return with `Esc`, and exit with `q`. Check empty, partial, initial-failure, stale and retry states independently. Discover `list` through built-in help and verify redirected, untruncated basic listing and diagnostics without a TUI.

### Tests first

- [X] T018 [P] [US1] Add state-machine tests in `tests/tui_state.rs` for navigation/help scrolling, focus identity and numeric successor/predecessor, loading/empty/partial/stale/failure, one outstanding request, 1-second refresh scheduling, 5-second post-failure retry, partial retry and warning persistence.
- [X] T019 [P] [US1] Add Ratatui TestBackend tests in `tests/tui_render.rs` for populated/empty/loading/error views, all fields, focus versus active/unavailable indicators, visible diagnostics, crawl continuity and help content at a full-size viewport; independently test TUI color suppression for nonempty `NO_COLOR`, `TERM=dumb`, and global `--no-color`.
- [X] T020 [P] [US1] Add PTY lifecycle tests in `tests/terminal_lifecycle.rs` for terminal preconditions, no-argument launch, `q`, Ctrl-C, SIGTERM/SIGHUP, Ctrl-Z/resume, blocked requests, repeated exit, partial initialization, draw/read/cleanup failures and restored terminal state. Add CLI tests in `tests/cli_basic.rs` for help without tmux, redirected listing of all fields and escaped names, empty/partial/fatal streams and statuses, cancellation, closed pipes, no prompts, and independent color suppression per stream for nonempty `NO_COLOR`, `TERM=dumb`, `--no-color` and non-terminal streams.

### Implementation

- [X] T021 [US1] Implement basic `list` and TUI dispatch in `src/cli.rs` and `src/main.rs`: default and explicit `tui` require terminal stdin and stdout; help/version run before terminal or tmux access; non-terminal TUI requests return 2 with actionable stderr. Support global `--no-color` before or after a subcommand, and provide root/list/tui help describing currently implemented commands, fields, defaults, color controls, examples and recovery.
- [X] T022 [US1] Implement `AppState` in `src/tui/state.rs` with screen, snapshot, focus, scroll, viewport, load/error state and timing; keep focus by numeric ID through rename/refresh, choose the next higher or nearest lower ID on removal, and ignore unsupported keys.
- [X] T023 [US1] Implement the single-worker request and refresh scheduler in `src/tui/mod.rs`: nonblocking event handling, 1-second regular intervals, no overlapping requests, partial retry on the next interval, 5-second retry after failed completion, stale/partial warnings and cancellation on exit.
- [X] T024 [US1] Implement list and scrollable help screens in `src/tui/view.rs`, including every supported key, focus versus active semantics, crawl/refresh explanation, explicit loading/empty/refreshing/partial/stale/failure feedback, field placeholders and actionable recovery; suppress TUI color independently for nonempty `NO_COLOR`, `TERM=dumb`, and `--no-color`, without relying on color for meaning.
- [X] T025 [US1] Implement the focused-session repeated one-cell-per-second detail crawl in `src/tui/state.rs` and `src/tui/view.rs`; freeze each pass across refresh, switch to updated values on the next pass, end on focus change/removal, and retain a partial warning until an old partial pass finishes.
- [X] T026 [US1] Implement scoped terminal ownership, raw/alternate-screen setup, restoration on partial setup, errors and unwind, signal cancellation/statuses 130/143/129, cleanup-failure status 1, repeated-exit progress and Ctrl-Z restoration/resume redraw in `src/tui/mod.rs` and `src/main.rs`.
- [X] T027 [US1] Integrate the shared snapshot into the TUI in `src/tui/mod.rs` and `src/tui/view.rs`, escape user-controlled text, tick uptime locally, and ensure intentional `q` returns 0 after a recoverable error when cleanup succeeds. Implement basic untruncated human-readable CLI listing in `src/output.rs` and `src/main.rs` with shared data, safe escaping, partial/fatal diagnostics, correct streams/statuses, cancellation and broken-pipe handling; independently suppress color per stream for nonempty `NO_COLOR`, `TERM=dumb`, `--no-color` and non-terminal streams.

**Checkpoint**: The full-size TUI and basic non-interactive CLI listing are usable, with matching built-in help. This is an internal milestone, not a feature-ready MVP: JSON and plain output remain US3 work.

## Phase 4: User Story 2 - Inspect sessions in a small or resized terminal (P2)

**Goal**: Preserve complete access to information while adapting the existing TUI to supported and undersized terminals.

**Independent test**: View and navigate identical sessions at 120x30, 80x24 and 40x10, resize among them, shrink below 40x10 and restore; inspect every field through the crawl and verify focus and screen remain intact.

### Tests first

- [X] T028 [P] [US2] Add cell-accurate layout tests in `tests/tui_resize.rs` for 120x30, 119x30, 80x24, 40x10, 39x10, 40x9 and zero area; cover 40-cell names, grapheme-aware truncation, combining marks, wide-character edges, indicators, no overflow and recovery.
- [X] T029 [P] [US2] Add resize-during-blocked-query PTY tests in `tests/terminal_resize.rs` for responsive resize/help/quit, preserved focus and screen, clipped warnings and recovery without an extra tmux request.

### Implementation

- [X] T030 [US2] Implement responsive row and card layout in `src/tui/view.rs`: at >=120 columns show all fields and names up to 40 cells, indicate overflow with `...`; below 120 show name and active only in rows and all remaining fields in the crawl.
- [X] T031 [US2] Implement grapheme-boundary truncation and display-cell-aware crawl clipping in `src/tui/view.rs`, leaving partially visible wide characters blank and preserving complete escaped values across successive crawl portions.
- [X] T032 [US2] Implement immediate resize and scrolling behavior in `src/tui/state.rs` and `src/tui/view.rs`: preserve screen/focus, avoid overlap and horizontal overflow, clip the size warning below 40x10, draw nothing at zero area, and retain exit/resize recovery.

**Checkpoint**: All sessions and full field values remain accessible across the specified viewport sizes.

## Phase 5: User Story 3 - List sessions without interactive navigation (P2)

**Goal**: Provide scriptable and human-readable listings with the same shared data and actionable help.

**Independent test**: Discover listing through built-in help; list from inside and outside an isolated tmux server, redirect outputs, parse JSON and compare all available fields and active/unavailable status with the TUI.

### Tests first

- [X] T033 [P] [US3] Add CLI argument/help contract tests in `tests/cli_contract.rs` for help/version without tmux, root/list/tui help and recovery, global `--no-color`, invalid flags/conflicting formats, redirected default launch and statuses 0/2.
- [X] T034 [P] [US3] Add CLI output and failure tests in `tests/cli_output.rs` for table/plain/JSON, six exact keys and independent nulls, numeric ordering, Unicode/control escaping, empty/partial/fatal streams, closed pipes (141), timeout and signal statuses; independently test color suppression on each CLI stream for nonempty `NO_COLOR`, `TERM=dumb`, `--no-color`, and non-terminal streams.
- [X] T035 [P] [US3] Add isolated cross-interface parity tests in `tests/interface_parity.rs` for inside/outside selected-server scope, available/unavailable active status, all fields, uptime precision and no tmux mutations.

### Implementation

- [X] T036 [US3] Extend clap `list` with mutually exclusive `--json`/`--plain` in `src/cli.rs`; update root, list and tui help with all available fields, defaults, examples, escaping, server selection, color controls, statuses and recovery, without requiring tmux.
- [X] T037 [US3] Extend the existing human table with five-field headerless tab-separated plain output in `src/output.rs`, with documented control/backslash escaping, `unavailable`, uptime, `WIDTHxHEIGHT`, yes/no active status and no empty-state prose.
- [X] T038 [US3] Implement pretty JSON array serialization in `src/output.rs` with exactly `name`, `windows`, `uptime`, `width`, `height`, `active`; preserve full strings and independently emit null for unavailable values, including dimensions, with a final newline.
- [X] T039 [US3] Extend shared retrieval integration for all output formats in `src/main.rs` and `src/output.rs`, including buffered pre-output failure handling, stderr diagnostics, partial status 1, empty-success semantics, cancellation and broken-pipe status 141; independently suppress color on each non-terminal stream or when `NO_COLOR` is nonempty, `TERM=dumb`, or `--no-color` is passed.

**Checkpoint**: CLI and TUI share session semantics; redirected output remains scriptable and errors never masquerade as empty success.

## Phase 6: Polish & cross-cutting acceptance

- [X] T040 After T042 and T049, run and record automated Linux acceptance for both interfaces, including real-tmux and PTY cases, using `specs/001-list-tmux-sessions/quickstart.md`. Capture revision, environment, tmux versions/context, exact commands, results, read-only state comparisons, failures and constitution-required coverage exceptions in `specs/001-list-tmux-sessions/acceptance.md`; skipped required Linux checks are not passing results. FreeBSD and macOS validation are optional.
- [X] T041 On an otherwise idle Linux host, run `python3 scripts/measure-performance.py` after the locked build and record 20 measured 100-session initial launches per interface, 20 refresh-visibility trials at 120 or more columns and 20 resize trials in `specs/001-list-tmux-sessions/performance-results.md`. Measure visible completion, not just subprocess or model timing; record raw timings and require at least 19 of 20 initial displays or errors within 2 seconds, refresh changes within 5 seconds and usable resizes within 1 second.
- [X] T042 [P] Extend `tests/cli_contract.rs`, `tests/terminal_lifecycle.rs` and `tests/tui_render.rs` as needed to check root and command help, active/unavailable explanations, and a PTY help-to-operation journey using `?`, `j`/`k`, `Esc` and `q` to navigate and inspect hidden details; verify CLI listing and use existing state and parity coverage where sufficient. Run the focused command in `specs/001-list-tmux-sessions/quickstart.md`; do not claim these checks establish first-time-user comprehension.
- [X] T043 After T040–T042 and T049 finish collecting and reconciling evidence, investigate and fix any automated acceptance or performance failures in the affected `src/` and `tests/` files without weakening targets; rerun affected checks against the final revision and update `specs/001-list-tmux-sessions/acceptance.md` and `specs/001-list-tmux-sessions/performance-results.md` with the revision and outcomes.
- [X] T044 Run the locked formatting, clippy, all-target/all-feature tests and build commands from `specs/001-list-tmux-sessions/quickstart.md`; record exact outcomes and any unavailable checks in `specs/001-list-tmux-sessions/acceptance.md` and block merge/release until all required checks pass.
- [ ] T045 Configure hosted branch rules and release environments to require all checks from `.github/workflows/ci.yml` on every merge and release path without administrative or other bypasses; verify failed or unavailable checks block each action and verify exact-staged commit checks, explicit local bypass and non-mutating formatting using `tests/quality_gates.rs`. Record protected paths, settings, evidence and configuration limitations in `specs/001-list-tmux-sessions/acceptance.md`; missing permissions or host capabilities block merge and release, not story implementation.
- [X] T046 Conduct the implementation review against every principle and applicable requirement in `.specify/memory/constitution.md`; verify that the acceptance workflow can be completed using built-in help alone, and record the outcome in `specs/001-list-tmux-sessions/acceptance.md`. For coverage exceptions identify the limitation and reproducible manual checks; record any constitutional exception's affected rule, rationale, scope, compensating measures and maintainer approval before merge.

## Dependencies & execution order

- Phase 1 precedes Phase 2. Phase 2 blocks all story implementation; T005-T008 can run concurrently after the harness exists, then their corresponding implementation proceeds.
- **US1** and its basic CLI together form an internal milestone, not the feature-ready MVP. T018-T020 use independent test files; implementation follows the relevant failing tests. T022 precedes T023-T025, T026 precedes T027, and the basic CLI and its help must pass before the internal checkpoint.
- **US2** extends US1's state, crawl and renderer, so it depends on US1; T028 and T029 can run concurrently before T030-T032. Complete responsive rendering before cross-interface acceptance.
- **US3** depends on the shared foundation and the dispatch established by T021, not on US2. Its tests T033-T035 can be authored concurrently; T035 is run against both completed interfaces, and T037-T039 extend the basic CLI from T027. Because US1/US2 and US3 can touch `src/main.rs`, `src/cli.rs`, or shared test infrastructure, coordinate those edits rather than treating conflicting tasks as parallel.
- Polish requires all selected stories. T049 first reconciles the acceptance record with the current platform policy; T040 follows T049 because both edit that record and follows T042 so new automated checks are included. T042 and T049 can proceed concurrently; run T041 trials without concurrent test load. T043 follows T040–T042 and T049, fixes failures and reruns affected checks against the final revision. T044–T046 follow T043 sequentially; T045's hosted protections must be verified before merge or release. T050 verifies the T043/T044 results; T051 follows T050 and can use the same hosted evidence as T045.

Dependency graph: `Setup → Foundation → US1 (including basic CLI) → US2 → Acceptance`; `Foundation → US1 dispatch (T021) → US3 tests`, with `US1 basic CLI (T027) → US3 implementation → Acceptance`.

## Parallel examples

- **US1**: After T004, T018 (`tests/tui_state.rs`), T019 (`tests/tui_render.rs`) and T020 (`tests/terminal_lifecycle.rs`) can be written together before their implementation.
- **US2**: After US1, T028 (`tests/tui_resize.rs`) and T029 (`tests/terminal_resize.rs`) can be written together before responsive-layout changes.
- **US3**: After foundation and T021, T033 (`tests/cli_contract.rs`), T034 (`tests/cli_output.rs`) and T035 (`tests/interface_parity.rs`) can be written together; defer parity execution until the TUI and CLI exist.
- **Remaining evidence**: T042 (`tests/cli_contract.rs`, `tests/terminal_lifecycle.rs`, `tests/tui_render.rs`) and T049 (`specs/001-list-tmux-sessions/acceptance.md`) can run together. T040 follows both; run T041 performance trials without concurrent test load.

## Implementation strategy

1. Complete setup and the shared foundation, including test-first acquisition, local staged checks and the CI workflow; hosted protection configuration does not block story implementation.
2. Demonstrate US1 plus its basic CLI listing and built-in help as an internal milestone, including the full-size TUI and its safe lifecycle alongside non-interactive listing.
3. Add US2 responsive layout and US3 JSON/plain CLI formats incrementally, coordinating shared files and validating each story at its checkpoint. Do not call the feature MVP ready until JSON and applicable plain output satisfy FR-015.
4. Complete automated Linux acceptance, help/workflow checks and quantitative evidence; remediate failures and rerun affected checks, then verify hosted protections and the constitutional implementation review. Manual acceptance and participant studies are deferred; constitution-required review and coverage-exception checks remain. FreeBSD testing is optional; unavailable or failed required checks block merge/release rather than count as passes.

## Phase 7: Convergence

- [X] T047 Propagate and report tmux subprocess cleanup failures during TUI exit in `src/tui/mod.rs` and `src/tmux.rs`, returning status 1 even when terminal restoration succeeds; add fault-injection regression tests that distinguish cleanup failure from normal cancellation per FR-014 and T026 (partial).
- [X] T048 Render both unavailable dimensions as `unavailablexunavailable` in `src/output.rs` and `-x-` in `src/tui/view.rs` and `src/tui/state.rs` when the current window is unavailable, preserving independent dimension markers; add regression tests in `tests/cli_output.rs` and `tests/tui_render.rs` per FR-004 (contradicts).
- [X] T049 [P] Reconcile the release-blocker summary in `specs/001-list-tmux-sessions/acceptance.md` with FR-020 and SC-010: retain unresolved automated Linux acceptance gaps and constitution-required reviews or coverage-exception checks, defer manual acceptance and participant studies, and preserve historical FreeBSD evidence without claiming current validation (partial).
- [X] T050 After T043 and T044, verify that automated acceptance and performance failures were remediated and affected checks and locked formatting, lint, test and build gates were rerun against the final revision; record outcomes and any remaining blockers in `specs/001-list-tmux-sessions/acceptance.md` (partial).
- [ ] T051 Configure and verify non-bypassable hosted checks from `.github/workflows/ci.yml` for every merge and release path, recording protected paths, settings and failure/unavailability evidence in `specs/001-list-tmux-sessions/acceptance.md`; keep those actions blocked if permissions or host capabilities prevent verification per FR-018 and T045 (missing).
- [X] T052 Add real-tmux CLI and TUI parity tests in `tests/interface_parity.rs` launched from inside an attached tmux pane, including a reliably identifiable client when possible and an ambiguous shared-pane case; resolve any observed active-status gap without guessing per US1/AC2, FR-003 and T035 (partial).

## Phase 8: Convergence

- [X] T053 CRITICAL: Complete the implementation review against every applicable constitution principle, verify the built-in-help-only acceptance workflow, and record outcomes and any coverage exceptions with reproducible alternative checks in `specs/001-list-tmux-sessions/acceptance.md` before merge per Constitution IV and T046 (missing).
- [X] T054 CRITICAL: Make complete TUI failure and partial-result diagnostics, including known recovery steps, accessible at all supported terminal widths without adding unsupported keys or screens; add rendering and navigation regression tests in `tests/tui_render.rs` and `tests/tui_resize.rs` per Constitution III and FR-012 (partial).
- [ ] T055 Configure and verify required non-bypassable hosted checks across every merge and release path, including failed and unavailable checks, and record the evidence or blocking host limitations in `specs/001-list-tmux-sessions/acceptance.md` per FR-018, T045 and T051 (missing).
- [X] T056 Enforce the single five-second deadline and cancellation throughout response parsing, reconciliation and final snapshot construction in `src/tmux.rs`, returning a retrieval failure instead of a late result; add focused deadline regression tests per FR-012 and T014 (partial).
- [X] T057 Report recognizable excluded tmux 3.3 prereleases as unsupported, identify the client or server and instruct users to upgrade; retain unverifiable diagnostics for genuinely indeterminate versions and add regression tests in `tests/tmux_process.rs` per FR-019 (contradicts).
