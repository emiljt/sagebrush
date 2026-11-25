# Research: List tmux Sessions

**Scope**: Phase 0 design decisions for [spec.md](spec.md). Sources justify
the design; implementation and live-tmux evidence are tracked separately in
[acceptance.md](acceptance.md) and [performance-results.md](performance-results.md).

## Rust stack and workflow

**Decision**: Use Rust 1.99.0, edition 2024, clap 4.6, ratatui 0.30 with its
compatible Crossterm 0.29 backend, serde/serde_json, Unicode display-width
helpers, signal-hook and libc. Use tempfile and portable-pty for tests; the
existing Cargo lockfile records resolved releases.

**Rationale**: Satisfies the requested stack in one package without requiring
another runtime command or a database. Direct libc calls support Unix terminal
control, nonblocking I/O, and child-process cleanup. Crossterm enables
`use-dev-tty`; this does not relax the requirement that both standard input
and output be terminals before opening the TUI.

**Alternatives considered**: A workspace, async runtime, tmux wrapper framework
or broad configuration system adds machinery beyond current requirements.

Commit gates check formatting and linting against exact staged content; allow
only an explicit local bypass. Merge/release additionally require tests and a
build, with no bypass. Checks do not modify files; ordinary local builds are
ungated. Test failure and unavailability as well as staged/unstaged divergence.

**Sources**: [clap](https://docs.rs/clap/4.6.7/clap/),
[ratatui](https://docs.rs/ratatui/0.30.2/ratatui/),
[Cargo rust-version](https://doc.rust-lang.org/cargo/reference/rust-version.html),
[Clippy](https://doc.rust-lang.org/clippy/usage.html).

## CLI and field presentation

**Decision**: Use explicit `list` and `tui` commands; default to the TUI only
when stdin and stdout are terminals. Parse help/version before tmux access.
Provide human table, headerless plain records, and a six-key JSON array as
specified in [contracts/cli.md](contracts/cli.md). Preserve names, escape
controls in text and diagnostics, and represent unavailable fields explicitly.

**Rationale**: Predictable scriptable output and consistent semantics avoid
hidden prompts or mode changes when redirected.

**Alternatives considered**: Automatically listing when piped conflicts with the
accepted dispatch contract; JSON-only defaults make human use less direct.

**Sources**: [CLI Guidelines](https://clig.dev/),
[IsTerminal](https://doc.rust-lang.org/std/io/trait.IsTerminal.html).

## Server selection and version verification

**Decision**: Verify the executable's version with `tmux -V` and independently
query `#{version}` from the selected running server with a non-starting
`display-message` request. Verify final 3.3 or later separately for each.
Outside tmux use the default server; inside use the inherited server, even if
non-default. Validate the inherited socket context before invoking tmux so a
malformed context cannot silently select the default. Distinguish an absent
server from access failure, protocol failure and unverifiable version. Treat
an incompatible protocol as an unverifiable server version with a diagnostic,
not an empty result.

**Rationale**: `-V` reports the client binary's version; the `version` format
is evaluated by the server. In tmux source, `display-message` lacks the
`CMD_STARTSERVER` command flag, so that command does not start a missing server.
The stock client's default socket-path helper may create its socket directory;
the feature prohibits creating sessions, windows or servers, not incidental
normal tmux socket-directory preparation. Explicit inherited socket selection
may use tmux's `-S` argument internally; it is not a public Sagebrush option.

Protocol mismatches return a mismatch diagnostic instead of a server release
number; do not infer a release from protocol number or assume that arbitrary
client/server version combinations are compatible.

**Alternatives considered**: Using `tmux -V` as the server version misidentifies
version-skewed servers. Mapping every connection failure to no sessions masks
permission and version failures. Trusting tmux's fallback for a malformed
inherited context can select the wrong server. Reimplementing all platform
socket-path logic merely to prevent incidental directory creation adds risk
without a product requirement.

**Sources**: [tmux 3.3 manual](https://github.com/tmux/tmux/blob/3.3/tmux.1),
[tmux 3.3 version and socket selection](https://github.com/tmux/tmux/blob/3.3/tmux.c),
[tmux 3.3 format callbacks](https://github.com/tmux/tmux/blob/3.3/format.c),
[tmux 3.3 display-message](https://github.com/tmux/tmux/blob/3.3/cmd-display-message.c),
[tmux 3.3 client](https://github.com/tmux/tmux/blob/3.3/client.c),
[tmux 3.3 protocol](https://github.com/tmux/tmux/blob/3.3/tmux-protocol.h).

## Invoking-client identity

**Decision**: Active means the session containing the invoking tmux client.
Outside tmux all active values are false. Inside, only mark a session active
when its client relationship is reliably established. Otherwise keep
identifiable rows, mark active fields unavailable, and report a diagnostic;
CLI output is partial and exits 1. Failure to identify the inherited server is
a fatal retrieval error, not merely unavailable active status.

**Rationale**: A pane can be linked into windows in multiple sessions and viewed
by different clients. tmux target resolution may choose a session or client by
recent activity rather than identify the client responsible for launching the
process. A pane environment cannot in general identify that viewer uniquely;
the `TMUX` session identifier is an environment snapshot, not an always-live
client identity. The user explicitly chose unavailable status for this case.
Treat unique, reliably supported contexts as success; prove any proposed
identification strategy with real-tmux tests, not an activity-based guess.

**Alternatives considered**: Marking any attached session, blindly resolving
`TMUX_PANE`, trusting a stale environment session ID, and selecting a
most-recent client may mark the wrong session. Prompting or a client-selection
flag would change the agreed scope and non-interactive behavior.

**Sources**: [tmux 3.3 target resolution](https://github.com/tmux/tmux/blob/3.3/cmd-find.c),
[tmux 3.3 environment](https://github.com/tmux/tmux/blob/3.3/environ.c),
[tmux manual](https://github.com/tmux/tmux/blob/3.3/tmux.1).

## Acquisition, framing and races

**Decision**: Execute tmux with argument arrays, never a shell. Initially
investigate bulk delimiter-safe numeric metadata using session ID, window count,
creation time, and current-window dimensions; retrieve names by exact session
ID separately so embedded newlines cannot corrupt record framing. Remove only
the command's final output newline, never trim the name. Strictly validate IDs
and individual values; preserve independently unavailable width and height.
Reconcile disappearing sessions within the same request deadline, failing
unreliable identity rather than fabricating a result.

**Rationale**: Arbitrary names cannot safely be embedded unescaped in
newline-delimited metadata. One command per name is simple but must be
benchmarked early with 100 sessions; optimize if necessary without imposing a
fixed output or session limit, losing arbitrary names, or silently truncating.

**Alternatives considered**: Newline-delimited raw names are ambiguous;
unbounded retries and lossy decoding break the correctness contract. tmux
control mode may be considered only if the simpler approach cannot meet the
measured target and its framing is verified.

**Sources**: [tmux 3.3 format and commands](https://github.com/tmux/tmux/blob/3.3/tmux.1),
[list-sessions source](https://github.com/tmux/tmux/blob/3.3/cmd-list-sessions.c).

## TUI, process ownership and signals

**Decision**: A synchronous Ratatui event loop communicates with one acquisition
worker. Schedule regular automatic refreshes initially at 1-second monotonic
intervals, skip overlapping scheduled requests, and retry
failed requests 5 seconds after they finish; partial requests retry at the next
regular refresh. Each request has a 5-second deadline. Concurrently drain
subprocess pipes, cancel and reap owned children on timeout or exit, and report
recoverable resource exhaustion without a fixed byte cap.

Render only the list and help screens; focus by numeric session ID and provide
the repeated one-cell-per-second detail crawl. Preserve the current crawl pass
across refresh, including its partial-data warning; start the next from updated
values. At below 120 columns, rows contain name and active status, with other
fields in the crawl. Do not split wide characters at crawl edges.

Handle interrupt, SIGTERM and SIGHUP through ordinary cancellation/cleanup
control flow, not work in signal handlers. TUI cleanup errors override status
with 1; cleanup has no fixed deadline and repeated exit requests indicate
progress. Restore the terminal before suspension and re-enter/redraw on resume.

**Rationale**: I/O outside the event loop keeps help, resize and exit available.
Snapshot-based state and pass boundaries make updates predictable.

**Alternatives considered**: Blocking the UI on tmux or guessing success from
truncated data violates responsiveness or correctness; manual-only refresh and
an additional detail screen contradict the current specification.

**Sources**: [Ratatui lifecycle](https://docs.rs/ratatui/0.30.2/ratatui/),
[Crossterm events](https://docs.rs/crossterm/0.29.0/crossterm/event/),
[Rust Child](https://doc.rust-lang.org/std/process/struct.Child.html),
[signal-hook](https://docs.rs/signal-hook/0.3.18/signal_hook/flag/).

## Automated help and workflow validation

**Decision**: Combine CLI help-contract checks, TUI cell-render and state
checks, isolated real-tmux parity tests, and PTY-driven workflows. Extend the
existing tests where needed to connect help instructions to listing, active or
unavailable status, hidden details, navigation and exit. Reviewers separately
verify the acceptance workflow using built-in help alone.

**Rationale**: `tests/cli_contract.rs` checks entry-point help and
`tests/tui_render.rs` checks help content and unavailable active feedback;
`tests/terminal_lifecycle.rs` exercises real terminal exit and restoration.
Those checks alone do not demonstrate a complete help-to-operation journey.
Combining them with state, parity and PTY tests covers observable behavior
without treating simulated actions as evidence of human comprehension.

**Alternatives considered**: A five-person study is deferred by the
specification, not counted as passed. Help-text substring checks alone cannot
establish that advertised actions work; replacing constitutional reviewer
verification or required coverage-exception checks with those assertions would
not satisfy the constitution.

**Evidence boundary**: This is a review of test sources, not a test execution
or a claim that the new SC-004 coverage is complete. See T042 in
`tasks.md` and the runnable guide in [quickstart.md](quickstart.md).

## Validation and evidence boundaries

**Decision**: Use test-first development where feasible, deterministic process
fixtures, isolated real tmux servers, cell-level render tests, and PTYs for
real terminal behavior. Exercise version skew, missing/inaccessible servers,
non-default inherited sockets, shared/linked panes, client switches, timeout,
signals, staged-content gates and no mutations. Validate acceptance scenarios
for both interfaces on Linux using automated checks where feasible. FreeBSD
remains supported, but its testing is optional for release. Non-automatable
manual acceptance and the five-person study are deferred; constitution-required
review and coverage-exception checks remain. macOS and other Unix-like systems
are best effort, with no required release validation. Record automated acceptance
and performance outcomes; do not claim human usability or FreeBSD validation
without evidence.

**Rationale**: Mocks alone cannot establish tmux context or terminal restoration.
Unique temporary socket directories protect real user servers.

**Alternatives considered**: Mock-only tests or a developer's real server cannot
safely establish these behaviors; silently skipped dependencies are not passing
release evidence. Requiring FreeBSD manual acceptance was rejected in the
2026-11-09 clarification; a later clarification also removed the FreeBSD
automated release-validation requirement because of testing difficulties.
Automated help and workflow checks do not substitute for evidence of
first-time-user comprehension.

**Evidence boundary**: The repository now contains an implementation and
recorded Linux and earlier FreeBSD tmux/PTY results. The cited acceptance and
performance records identify their tested revisions and remaining gaps. Those
results do not prove final-revision or future-release compatibility, complete
final automated Linux acceptance, or establish hosted gate enforcement.
There is no general guarantee that the invoking client is identifiable;
ambiguous contexts must remain explicitly unavailable.
