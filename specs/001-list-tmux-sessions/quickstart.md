# Quickstart Validation: List tmux Sessions

This is the Phase 1 validation guide. Run from the repository root; the Cargo
package and tests already exist. Recorded results and remaining release
evidence are in [acceptance.md](acceptance.md), not implied by this guide.

See [CLI contract](contracts/cli.md), [TUI contract](contracts/tui.md), and
[data model](data-model.md) for expected field shapes and behavior.

## Prerequisites and required gates

Use Linux, Rust 1.99.0 with rustfmt and clippy, a supported tmux client and
server >=3.3, and Python 3 for the performance harness. A real terminal is
needed only for the optional interactive checks below. Python is only a
validation tool, not a runtime dependency.
Validate the acceptance scenarios on Linux. FreeBSD remains supported, but
its testing is optional and not required for release for now. Manual acceptance
and participant studies are deferred; constitution-required review and
coverage-exception checks remain. macOS and other Unix-like systems are best effort;
their validation is optional and not required for release.

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
cargo build --locked
```

Formatting and linting must check the exact staged content before a commit, even
when the working tree differs; only an explicit local commit bypass is allowed.
Merge and release gates require formatting, linting, tests, and build, without
bypass. A failed or unavailable check blocks its action. Ordinary local builds
remain ungated, and formatting checks must not modify files. The staged
check implementation must be tested with differing staged/unstaged contents.
CI must run the checks above plus real-tmux/PTY cases; a skipped unavailable
dependency is not a passing result.
Use the existing Cargo.lock with `--locked`. Do not suppress failures or bypass
tests to obtain a green check.

FreeBSD validation may be run optionally; record its environment, commands,
revision and outcomes if performed. Cross-compilation alone is not runtime
validation. The manual scenarios below are optional supplementary checks; use
the automated tests for required acceptance where feasible.

## Automated acceptance

Run the locked test suite above on Linux with tmux available. It exercises
isolated tmux servers, CLI contracts and outputs, TUI rendering and state, PTY
lifecycle and resizing, and CLI/TUI parity. Tests must pass rather than be
skipped; compare the recorded revision, environment, commands, results and
coverage exceptions with [acceptance.md](acceptance.md).

For SC-004, automated checks must cover help content, both listing workflows,
active or unavailable status, hidden details and exit. Some additional checks
are planned in `tasks.md` (T042); do not treat the current suite as proof of
complete coverage until those checks exist and pass. Reviewers must also verify
that the acceptance scenario can be completed using built-in help alone.
Automation does not establish first-time-user comprehension. For any automated
coverage exception, document the limitation and reproducible manual checks as
required by the constitution.

Run the relevant existing targets together after T042 is implemented:

```sh
cargo test --locked --test cli_contract --test cli_output --test tui_render \
  --test tui_state --test terminal_lifecycle --test interface_parity
```

Expected: all selected tests pass, including the added help-to-workflow cases;
PTY and real-tmux tests require their stated prerequisites and must not be
silently skipped. This focused command supplements, not replaces, the required
full gate above.

## Help and non-terminal dispatch (optional manual checks)

```sh
./target/debug/sagebrush --help
./target/debug/sagebrush list --help
./target/debug/sagebrush tui --help
./target/debug/sagebrush --version
```

All succeed without querying tmux. Help includes entry points, defaults, fields,
flags, color/environment behavior, examples, exit codes and recovery steps.

```sh
./target/debug/sagebrush </dev/null
printf 'status=%s\n' "$?"
./target/debug/sagebrush list --json --plain
printf 'status=%s\n' "$?"
```

Both return 2 with actionable stderr and empty stdout. Neither opens the TUI or
prompts. Explicit `list` works with redirected stdin.

## Isolated tmux fixture (optional manual checks)

Do not create/kill sessions on a real user's default server. The following
subshell isolates its socket directory, environment and cleanup; it does not
change the calling shell's tmux context.

```sh
(
  fixture_dir=$(mktemp -d /tmp/sagebrush-validate.XXXXXX) || exit 1
  export TMUX_TMPDIR="$fixture_dir"
  unset TMUX TMUX_PANE
  fixture_server_running=0
  cleanup() {
    if [ "$fixture_server_running" -eq 1 ]; then
      tmux kill-server 2>/dev/null || :
    fi
    rmdir "$fixture_dir/tmux-$(id -u)" 2>/dev/null || :
    rmdir "$fixture_dir" 2>/dev/null || :
  }
  trap cleanup EXIT

  tmux new-session -d -s work -x 120 -y 30 || exit 1
  fixture_server_running=1
  tmux new-window -t work: -n second || exit 1
  tmux new-session -d -s 'project with spaces' -x 80 -y 24 || exit 1

  tmux list-sessions
  ./target/debug/sagebrush list
  ./target/debug/sagebrush list --plain
  ./target/debug/sagebrush list --json | python3 -c '
import json, sys
rows = json.load(sys.stdin)
assert len(rows) == 2
assert {r["name"] for r in rows} == {"work", "project with spaces"}
for r in rows:
    assert set(r) == {"name", "windows", "uptime", "width", "height", "active"}
    assert r["windows"] >= 1 and r["uptime"] >= 0
    assert r["width"] >= 1 and r["height"] >= 1
    assert r["active"] is False
'
  ./target/debug/sagebrush list --plain | awk -F '\t' 'NF != 5 { exit 1 }'

  tmux kill-server || exit 1
  fixture_server_running=0
  ./target/debug/sagebrush list --json
  printf 'empty status=%s\n' "$?"
)
```

Expected: names round-trip, all six JSON keys are present, no session is active
outside tmux, and the final empty JSON result is `[]` with status 0.
Sagebrush must not start a server after the fixture server is stopped. Inspect
the isolated socket directory as part of the automated regression test.

Automated fixture tests additionally cover tab/newline/backslash/control and
Unicode names, exact current-window sizes with differently sized windows,
creation-based uptime boundaries, session changes mid-query, partial fields,
unsupported tmux, permission errors, timeouts, closed pipes, and child cleanup.
Use actual argument arrays, not shell-interpolated session names.

## Interactive flow and context (optional manual checks)

In the same kind of isolated fixture, attach a client to `work` and launch the
built binary from its shell. Use an absolute binary path if the fixture shell
does not start in the repository.

Verify:

1. No-argument launch opens the TUI. When the invoking client is reliably
   identifiable, only its session has `*`; otherwise active status is unavailable
   with a diagnostic.
2. `j`/`k` change focus but never the active tmux session. Unsupported keys are
   ignored. `?` opens help; `j`/`k` scroll help and Esc returns to the list.
3. Confirm each focused card's automatic one-cell-per-second crawl exposes full
   names and every field without extra keys. Refresh does not restart a pass.
4. Rename/create/remove/change windows from a second fixture client. Confirm
   automatic updates and stable focus; on removal focus moves to the next higher
   numeric session ID, otherwise the nearest lower.
5. Test 120x30, 80x24, and 40x10; resize between them. At >=120 columns all
   fields are represented; below 120 only name and active appear in rows while
   all fields remain available in the crawl.
6. Shrink below 40x10 and restore. As much warning as fits is drawn; a
   zero-width or zero-height viewport draws nothing. Quit and resize recovery
   remain available.
7. Exit with `q` and Ctrl-C; ensure echo, cursor, screen and shell input work.
   Repeat with a blocked query; test SIGTERM, SIGHUP, and Ctrl-Z/resume as well.
8. With two attached clients and moved/grouped panes, verify active status is
   correctly resolved or explicitly unavailable, never guessed.

The PTY suite must also force initialization, draw/read, and cleanup errors.
Ratatui TestBackend alone cannot verify actual terminal restoration.

## Failure, stale, and presentation checks

Use process fixtures in the test harness to force missing executable, access
failure, malformed metadata, invalid text, unavailable fields, blocked children
and oversized output. These tests must not alter system installations or the
user's tmux permissions.

An initial fatal failure reports an error, not an empty list. A failed TUI refresh
marks previous data stale until a later successful refresh. Partial JSON retains
the six keys, uses null for unavailable fields, writes diagnostics to stderr,
and exits 1. Each dimension can independently be null. Fatal CLI errors leave
stdout empty and exit 1.

Verify `--no-color`, nonempty `NO_COLOR`, `TERM=dumb`, and redirection independently
for each output stream. JSON/plain contain no styling. Help and errors never
require an interactive confirmation or leak control sequences from names.

## Quantitative acceptance and recorded outcomes

Use the automated fixture harness to create 100 sessions in a private server,
record session state before/after Sagebrush runs, and confirm no mutations.
Capture raw trial durations and environment/tool versions in implementation
review evidence, not by changing this planning guide.

On an otherwise idle Linux host, run the existing harness against the locked
debug build:

```sh
cargo build --locked
python3 scripts/measure-performance.py
```

It uses an isolated server and reports individual trials; retain the raw output
and tested revision in [performance-results.md](performance-results.md). Do not
run other tests concurrently with the timing trials.

| Check | Required result |
|-------|-----------------|
| 20 initial launches per CLI/TUI interface | >=19 display list/error within 2 seconds |
| 20 automatic-refresh trials at >=120 columns | >=19 show changed membership and non-crawl details within 5 seconds |
| 20 resizes, including during blocked query | >=19 usable layouts within 1 second |
| Unchanged sessions across CLI/TUI | All names/counts/dimensions/active values agree; uptime agrees within displayed precision |
| Built-in help and workflows | Automated checks cover help content, both listings, active status, hidden details and exit; they do not prove first-time-user comprehension |

Measure UI-visible completion, not just subprocess duration or model calculation.
The five-person study is deferred, not passed. Record failures plainly and
iterate before marking acceptance complete. If the
per-session name calls miss the 100-session target, optimize that boundary before
shipping rather than loosening the target or returning silently incomplete data.
