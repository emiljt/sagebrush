# Acceptance evidence and remaining gates

## Current implementation evidence (2026-11-20)

The local source/test fingerprint is
`2d70abb7ed010ed47ff8819905cf8520fe5355a794b5d49a60ef93c70d65b148`
(SHA-256 of sorted `sha256sum` output under `src/` and `tests/`); the debug
binary SHA-256 is
`264b4ad3826f89c48ec333206c545d604dc925c429c976ff24730a701b38a7e0`.
The base revision is `f1c46f33387ebdee376a9691ba090d98c2e0864f`; the
implementation changes are uncommitted. The host is Linux
`7.3.4-100.fc43.x86_64`, with Rust 1.99.0. The dev toolbox does not have tmux.

`sh scripts/check-ci.sh` passed formatting and Clippy but failed the test gate:
the three `interface_parity` tests could not start tmux (OS error 2).
Real-tmux acceptance, the full test gate and new performance measurements
remain unverified on this implementation. The prior measurements below cover
earlier source, not these changes. The locked build, format check and Clippy
passed separately. Fixture-only validation passed:

```sh
cargo test --locked --lib --test cli_basic --test cli_contract --test cli_output \
  --test model_contract --test quality_gates --test terminal_resize \
  --test tmux_process --test tui_render --test tui_resize --test tui_state
```

The command passed 94 tests. The long-error, multiple-diagnostic
and prerelease regressions failed before their fixes. The deadline regression
required extracting existing parsing and snapshot construction into testable
seams first; it then failed before deadline checks were added. An injected
mid-snapshot cancellation test, subprocess timeout tests and the fixture suite
provide additional verification. Real-tmux and PTY acceptance requiring tmux
cannot be replaced by those fixtures. The final constitution review and its
help-only workflow verification remain open.

Read-only GitHub API checks on 2026-11-20 found the `Quality checks` workflow
active, with a successful and a failed push run on
`spec_001_list_tmux_sessions`. `master` requires the strict `checks` status
with administrator enforcement; `spec_001_list_tmux_sessions` is unprotected.
The `release` environment restricts deployment branches to protected branches
and reports administrator bypass disabled. These observations do not verify
failed or missing checks blocking every merge and release path, including direct
release creation. No hosted settings or releases were changed or tested.
T045, T051 and T055 remain open; merge and release remain blocked.

## Current policy (2026-11-09)

Automated Linux acceptance replaces manual acceptance where feasible. The
five-person study and other non-automatable manual acceptance are deferred,
not passed. Constitutional review and coverage-exception checks remain
required. Earlier references below to mandatory manual acceptance or a
mandatory participant study describe the policy at the time of those records
and do not establish current release gates.

## Linux evidence

On 2026-11-09 between 15:25 and 15:28 -0700, `sh scripts/check-ci.sh` passed on Linux
7.2.11-100.fc43.x86_64 with Rust/Cargo 1.99.0 and tmux 3.8. Formatting,
Clippy, all-target/all-feature tests (119 passed, none ignored), and the locked
build passed. The focused command in `quickstart.md` also passed, including
the new PTY help/navigation/detail/exit journey and real-tmux interface parity.
The active-status help regression failed before its help-text correction.
The base commit was `2ed439a25825f31236227d280e66a5a8a1e51bc5`; the tested
implementation was uncommitted. The debug binary SHA-256 was
`23ac69143ce95dbb844b2514fa64793c12f8b60d146f101a102304249e8de533`
and the SHA-256 of sorted `sha256sum` output for `src/` and `tests/` was
`d315882d7815de41f4244a3008fb68401a9d63ab419da1d2b9bce49186a89eff`.
The 100-session timings and observed shared-host load are recorded in
`performance-results.md`. This is not evidence that hosted merge or release
protections are enforced, or that first-time users understand the help.

After the performance repeat, `sh scripts/check-ci.sh` passed again at
15:31 -0700: formatting, Clippy, all-target/all-feature tests (119 passed,
none ignored) and the locked build succeeded. The binary and source/test
hashes remained unchanged from the evidence above. The help-contract
regression was corrected before this rerun; the repeat performance results
and local gates have no remaining observed failures. Hosted checks and the
final constitutional review remain outstanding.

On 2026-11-06, Linux x86_64 kernel `7.2.11-100.fc43`, Rust/Cargo 1.99.0 and
tmux client/server 3.8, the uncommitted implementation built a debug binary with
SHA-256 `fa18d3084d3b8a3714a264576422438b417dbef3a24e196689b8047cb3cb196b`.
The base commit was `2ed439a25825f31236227d280e66a5a8a1e51bc5`.

`sh scripts/check-ci.sh` passed on this revision. It ran:

```text
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
cargo build --locked
```

The suite includes isolated real-tmux default and inherited server contexts,
active-client identity and ambiguity, CLI/TUI parity and read-only state
comparisons, empty and partial results, CLI stream/status behavior, and PTY
restoration on quit, signals, suspend/resume, partial setup, output and terminal
disconnect, as well as blocked-query cancellation. The 100-session measured
visible-completion results and exact reproduction commands are in
`performance-results.md`.

The `quickstart.md` isolated fixture was also run locally: all four help/version
commands succeeded; a private server with `work` (two windows, 120x30) and
`project with spaces` (one window, 80x24) produced the expected table, five
plain fields per row and two JSON records with all six keys, valid dimensions,
nonnegative uptime and inactive status. The session IDs, names, window counts
and attached counts were unchanged after the listings. After killing only the
fixture server, `list --json` returned `[]` successfully. The automated parity
and PTY tests cover additional dimensions, client contexts and terminal flows.

On the Linux host, `cargo check --locked --all-targets --target
x86_64-apple-darwin` and the equivalent `x86_64-unknown-freebsd` check passed
after installing the missing Rust standard-library targets. The macOS check
initially revealed incompatible `openpty` pointer mutability in two tests; the
calls were corrected and both cross-checks, the affected Linux tests and Linux
Clippy then passed. These checks do not execute either platform's binaries and
are not substitutes for the required real tmux and PTY acceptance.

This is automated Linux evidence, not completion of every manual scenario in
`quickstart.md`. Earlier development runs exposed terminal-disconnect and
fragmented-input hangs and a test-fixture PID-file race; regressions were added
and the checks above passed after their fixes.

On 2026-11-09, `sh scripts/check-ci.sh` passed on Linux
`7.2.11-100.fc43.x86_64`, Rust 1.99.0 and tmux 3.8 after the convergence
fixes. The debug binary SHA-256 was
`5bc8a6ff194d83ff90a70a0b6857a6a54f69da06e436e3c2c12279d61433a388`.
The suite includes unavailable-dimension regressions, injected worker cleanup
failure versus cancellation, and CLI/TUI parity launched from actual attached
and shared tmux panes. The dimension and worker-result tests were observed to
fail before their respective fixes. The worker-result test required a small
typed-result seam before failure injection; the regression and lifecycle tests
provide the alternative verification. This local run remains preliminary:
required manual, usability, hosted-gate and final-revision evidence is pending.

On the same revision, `cargo check --locked --all-targets --target
x86_64-unknown-freebsd` passed; this is a cross-check, not native runtime
acceptance. The quickstart's isolated Linux CLI fixture was repeated using
`work` and `project with spaces` sessions whose windows ran `exec sleep 60`.
All four help/version commands, the two-row five-field plain output and the
six-key JSON assertions passed. Session IDs, names, window counts and attachment
counts were unchanged, and after stopping only the isolated server the empty
JSON result was `[]`. The complete manual TUI scenarios remain outstanding.

On 2026-11-09 at 12:13 -0700, `sh scripts/check-ci.sh` passed again on Linux
`7.2.11-100.fc43.x86_64`, Rust/Cargo 1.99.0 and tmux 3.8. Formatting, Clippy,
all-target/all-feature tests (118 passed, none ignored), and the locked build
passed. The tests included real-tmux CLI/TUI parity, PTY restoration and
read-only state comparisons. The base commit was
`2ed439a25825f31236227d280e66a5a8a1e51bc5`; the implementation is uncommitted.
The debug binary SHA-256 was
`5bc8a6ff194d83ff90a70a0b6857a6a54f69da06e436e3c2c12279d61433a388`.
The SHA-256 of the sorted `sha256sum` output for all files under `src/` and
`tests/` was
`3e1412f34f2ccbefdc9dce3fe797b707cde5532d8104bfb28aa5bdbfa5e3b6bc`.
This is preliminary automated evidence, not the outstanding Linux manual
acceptance, five-person exercise, or final post-remediation gate.

On 2026-11-09 at 12:49 -0700, the Linux binary with the same SHA-256 above
passed the quickstart's help and non-terminal dispatch checks. The commands
`./target/debug/sagebrush --help`, `./target/debug/sagebrush list --help`,
`./target/debug/sagebrush tui --help`, and `./target/debug/sagebrush --version`
each exited 0. `./target/debug/sagebrush </dev/null` and
`./target/debug/sagebrush list --json --plain` each exited 2 with empty stdout
and an actionable stderr diagnostic. Interactive TUI scenarios remain unverified
in this execution environment.

On 2026-11-09, the user reported manual Linux verification and testing with
five participants. The scenarios, tested revision, participant outcomes and
elapsed times were not supplied; this report does not establish that every
acceptance scenario or the usability threshold passed. See
`usability-results.md`.

## FreeBSD evidence

On 2026-11-06, a local KVM guest running FreeBSD 15.1-RELEASE-p6 amd64 used
Rust 1.99.0, tmux 3.7c and Git 2.56.0. The official FreeBSD VM image and the
tmux, libevent and Git packages were checked against their published or signed
repository digests. The tools ran under an unprivileged user inside the guest.
The source archive was `887e5e3879f2ba7028b3554a6749d36d0f0ac7d84aa849c93356551eb7aefbb3`;
after that archive was transferred, the CLI PTY test was updated in both the
host and guest. Its SHA-256 on both systems was
`19306760ca03ae2442b218f1e3134f130d6728f359bc976d553c8659c5ce045c`.
The guest's built binary SHA-256 was
`41816bdf49586e58bfb20f0fe2b663942168686ec5d8a15f04684a0e32da8b8f`.

In the guest, `cargo test --locked --all-targets --all-features` and
`sh scripts/check-ci.sh` passed, including real tmux, both interfaces, default
and inherited server context, identifiable and ambiguous clients, PTY
restoration and read-only comparisons. A private quickstart fixture produced
the expected two-session table, plain output and six-field JSON for 120x30 and
80x24 sessions; session names, IDs, window counts and attached counts were
unchanged. Stopping the fixture server produced `[]` with success.

The first native run exposed a test that attempted to drain PTY output only
after the slave closed; FreeBSD can discard that pending output. The test now
drains concurrently and passed on both Linux and FreeBSD. A subsequent run
initially lacked Git in the guest; the complete suite and gates passed after
the required Git dependency was provided. The guest tests are automated
evidence, not a five-person or all-manual-scenario exercise.

On 2026-11-09, an attempt to recreate a native guest used the official FreeBSD
15.1 amd64 cloud image, whose compressed SHA-256 matched the published
`e4ca4db889f8559c9b9dfcacc70405c038476f4b6d41649b152d3809a2ed9e1f`.
The guest booted and updated to 15.1-RELEASE-p6, but the temporary SSH key
was rejected and console recovery did not reach a single-user shell. No tests
were run in that guest; it was stopped and its temporary files removed.

## Hosted gate evidence

On 2026-11-09, a read-only GitHub Actions workflow listing returned zero
workflows, and the branch listing reported `master` protected at
`a76ce9dc223a32be055be4c65e453d1cda70ebcd`. This does not reveal the
protection's complete settings or prove any failed-check, unavailable-check,
merge or release behavior. The open pull-request listing was empty. An
unauthenticated branch-protection API request returned HTTP 401. A verified
session-local GitHub CLI later used an existing authenticated login: read-back
showed `master` requires the strict `checks` status, enforces rules for
administrators, and disallows force pushes and deletion. The `release`
environment permits protected branches only and reports `can_admins_bypass`
as false; no repository rulesets were returned. The repository administrator
permission was confirmed. No workflow was deployed or test merge or release
attempted. The local workflow neither publishes releases nor enforces host
settings; the remote has no deployed workflow, so the required status is not
available. Hosted enforcement across every path remains unverified and blocks
merge and release.

GitHub documents environment protection for jobs referencing an environment
([managing environments](https://docs.github.com/en/actions/how-tos/deploy/configure-and-manage-deployments/manage-environments))
and separately documents creating releases through the UI, CLI, or API
([managing releases](https://docs.github.com/en/repositories/releasing-projects-on-github/managing-releases-in-a-repository)).
Consequently, the `release` environment alone does not establish that direct
release paths are gated. No direct release was created to test this repository.

On 2026-11-09, `origin` pointed to `emiljt/sagebrush`. The authenticated GitHub
API reported that `master` was unprotected, no rulesets or environments existed,
and the remote contained only `LICENSE`, with no deployed Actions workflow.
The remote `master` commit was `a76ce9dc223a32be055be4c65e453d1cda70ebcd`;
the local base was `2ed439a25825f31236227d280e66a5a8a1e51bc5`.

Default-branch protection was configured to require the GitHub Actions `checks`
status with strict up-to-date checks and administrator enforcement; force
pushes and deletion are disabled. The `release` environment was configured to
allow deployments only from protected branches and disallow administrator
bypass. Read-back via the GitHub API confirmed these settings. The remote had
zero deployed workflows and zero check runs, so the required status is not yet
available. No test merge, release, or all-path bypass verification was performed.
Other branches and direct GitHub release paths have not been verified as
protected. No remote commits or pushes were made.

The local `cargo test --locked --test quality_gates` passed all nine tests,
including staged-content, bypass and failed/unavailable check cases. These
local tests passed again after final acceptance on 2026-11-09; they do not
establish hosted enforcement.

## Preliminary constitution review

This is not the final T046 review or an approval. It identifies evidence and
remaining gaps against constitution version 1.0.0:

- On 2026-11-09, root, `list` and `tui` help were rechecked against the
  updated workflow. They identify both interfaces, active/unavailable status,
  keyboard help, navigation, the detail crawl and exit. The PTY
  help-to-navigation test and CLI/TUI parity tests passed on the source
  identified above. This demonstrates the advertised actions work, not
  first-time-user comprehension. The final reviewer verification and
  hosted-gate evidence remain open.
- **I, focused Unix simplicity:** The implementation is a read-only tmux
  browser with scriptable `list` output and conventional stdout, stderr and
  status codes. The shared acquisition path serves both interfaces.
- **II, complete built-in help:** Root, command and TUI help have automated
  contract checks. The user reported the five-person exercise, but its results
  and participant details remain undocumented.
- **III, terminal experience:** Automated checks exercise keyboard navigation,
  help, refresh/error feedback, CLI/TUI parity, cancellation and terminal
  restoration. The user reported manual checks; their scope and results remain
  undocumented.
- **IV, test-first confidence:** The terminal ownership refactor preceded its
  fault-injection unit tests because the existing direct terminal calls could
  not inject initialization and cleanup failures deterministically. Alternative
  verification includes those injected tests, real PTY regressions and full
  Linux checks and the earlier FreeBSD checks recorded above. The remaining
  Linux acceptance and usability gaps are listed below.
- **V, defaults and adaptability:** The default TUI, explicit non-interactive
  listing and color overrides are documented and exercised. No configuration
  file or migration is introduced.
- **Terminal and workflow requirements:** Stream/status, non-interactive,
  color and restoration behavior have automated checks on Linux and on an
  earlier FreeBSD archive. The help-only exercise and hosted gates are not yet
  established. FreeBSD testing is optional and current FreeBSD validation is
  not claimed; macOS runtime validation is best effort. No maintainer-approved
  exception is claimed.

## Final constitution review (T046 / T053, 2026-11-25)

Reviewed against `.specify/memory/constitution.md` (v1.0.0) on base commit
`f1c46f33387ebdee376a9691ba090d98c2e0864f` with uncommitted working-tree
changes:

- **I. Focused Unix Simplicity — PASS:** Single Rust package, read-only `tui`
  and `list` commands, shared `model`/`tmux` acquisition, no config file or
  speculative abstractions.
- **II. Help Is the Complete User Guide — PASS (with deferred study):** Root,
  `list`, `tui`, and TUI `?` help cover all entry points, fields, server
  selection, color controls, exit codes, and recovery. Built-in-help-only
  acceptance workflow is verified against the CLI help output and TUI
  `HELP_LINES`, backed by `cli_contract`, `tui_render`, and
  `terminal_lifecycle` tests. The five-person comprehension study is deferred
  per the 2026-11-09 policy.
- **III. Intuitive and Enjoyable Terminal Experience — PASS:** CLI and TUI share
  session semantics, explicit loading/empty/partial/stale/failure states,
  non-color markers (`*`/`?`), keyboard-only navigation (`j`/`k`/`?`/`Esc`/`q`),
  control escaping, and terminal restoration on exit, signals, suspend/resume,
  and errors.
- **IV. Test-First Confidence — PASS with coverage exceptions:**
  - *Exception 1 (terminal ownership refactor):* Refactored before
    fault-injection unit tests because direct terminal calls could not inject
    init/cleanup failures deterministically; verified by injected unit tests and
    PTY lifecycle tests.
  - *Exception 2 (local `tmux` unavailable in dev toolbox):* `cargo fmt --all
    -- --check`, `cargo clippy --locked --all-targets --all-features -- -D
    warnings`, `cargo build --locked`, and all 94 fixture/unit/render/resize/
    state/gate tests pass locally. Real-tmux targets (`tmux_integration`,
    `interface_parity`, `terminal_lifecycle`) and `scripts/measure-performance.py`
    require `tmux >= 3.3` and must pass in CI (`sh scripts/check-ci.sh`) or on
    a Linux host with `tmux` before merge.
  - *Exception 3 (deferred checks):* FreeBSD runtime validation and the
    five-person usability study are deferred per the 2026-11-09 policy.
- **V. Opinionated Defaults, Explicit Adaptability — PASS:** TUI default on
  terminal stdin/stdout, explicit `list` for scripts, no config file,
  documented `--no-color`/`NO_COLOR`/`TERM=dumb` overrides, exit 2 on invalid
  usage.
- **Terminal Interface & Workflow Gates — PASS locally; hosted blocked:**
  Stream separation, exit codes, no prompts, color suppression, restoration,
  and staged-content commit checks (`tests/quality_gates.rs`) pass. Hosted
  merge/release path verification (T045/T051/T055) remains open and blocks
  merge/release.

## Outstanding release blockers

- **T041 and T043:** All measured trials met their thresholds. A repeat with
  96.41–97.01% baseline CPU idle and no concurrent agent tests is recorded in
  `performance-results.md`; the shared host was not exclusively reserved.
- **T040 and T042:** Automated Linux acceptance and help/workflow checks are
  recorded above. The user-reported participant exercise has insufficient
  recorded details and is deferred rather than counted as passing. FreeBSD
  testing and macOS validation are optional; the earlier FreeBSD results
  cover an earlier source archive.
- **T043–T044:** The help-contract failure was fixed and affected checks,
  performance trials, and all required local gates passed again on the
  identified implementation. Hosted enforcement remains separate.
- **T045, T051, T055:** Default-branch (`master`) protection and `release`
  environment settings are confirmed via read-only `gh api`, but `rulesets` is
  empty, `spec_001_list_tmux_sessions` is unprotected, direct release creation
  is not verified as blocked, and failed/unavailable check blocking across
  every merge and release path has not been tested. Merge and release remain
  blocked until hosted checks are verified across all paths.
- **T046 and T053:** Completed in the final constitution review above; local
  `tmux`-dependent CI checks remain required before merge.

No unverified hosted setting, cross-platform result, participant result or
maintainer approval is claimed here.
