# Data Model: List tmux Sessions

**Related**: [spec.md](spec.md), [research.md](research.md),
[CLI contract](contracts/cli.md), [TUI contract](contracts/tui.md).

All state is process-local; tmux remains the source of truth. No persistence or
mutation model is introduced.

## Session identity and fields

| Entity/field | Rust representation | Meaning and validation |
|--------------|---------------------|------------------------|
| `SessionId` | Newtype over `String` | tmux `$` followed by decimal digits; stable within this server lifetime, never a row number/name |
| `WindowId` | Newtype over `String` | tmux `@` followed by decimal digits; current window identity |
| `Session.id` | `SessionId` | Required unique ID within a snapshot |
| `Session.name` | `Field<String>` | Full decoded UTF-8 name, no trimming/normalization/truncation |
| `Session.window_count` | `Field<u32>` | At least one for a surviving session |
| `Session.created_at` | `Field<SystemTime>` | Nonnegative Unix creation timestamp from tmux |
| `Session.current_window` | `Field<CurrentWindow>` | Current window identity and individually available dimensions |
| `Session.active` | `Field<bool>` | Match with invoking context; never attachment count or row focus |
| `CurrentWindow.width/height` | `Field<u32>` each | Independently available and greater than zero; not pane or viewport dimensions |

`Field<T>` is either `Known(T)` or `Unavailable(FieldIssue)`. Missing/invalid
values are not zero, false, empty strings, or default windows. An invalid or
duplicate required session ID invalidates the load because rows can no longer be
identified safely. A valid ID with an unavailable individual attribute yields a
partial snapshot and diagnostic rather than an invented value.

Names containing tabs, newlines, backslashes or control characters remain raw
model values. Human output escapes them; JSON serialization round-trips them.
Invalid UTF-8 is a field issue, not silently lossy-decoded output.

## Invoking context

`InvokingContext` is either:

- `OutsideTmux`: no inherited tmux context, all session active fields are known false.
- `InsideTmux`: identified invoking server and, when reliably determinable,
  invoking client/session.
- `ActiveUnavailable`: server is identifiable but the invoking client/session
  cannot be resolved safely; active fields are unavailable with diagnostics.

An inherited context that cannot identify its server is a fatal request error,
not `ActiveUnavailable` or a fallback to the default server. Pane membership
alone does not establish client identity in shared/grouped windows. Never select
an unrelated client arbitrarily. At most one row may be active; if a previously
resolved ID disappears during acquisition, reconcile within the deadline or
mark active status unavailable. Outside tmux uses the default server.

## Snapshot and issues

`SessionSnapshot` contains:

- `sessions: Vec<Session>` in ascending numeric session-ID order; no duplicates.
- `captured_at: SystemTime`, sampled once for the completed load.
- `captured_monotonic: Instant`, used for stable elapsed display between refreshes.
- `issues: Vec<FieldIssue>`, nonempty when any row has unavailable required data.
- Resolved invoking context for this load.

`FieldIssue` identifies an optional session ID, affected field, failure category,
plain-language message, and a recovery hint when known. Failure categories cover
missing/unsupported tmux, connection/access, malformed output, invalid text,
context resolution, timeout, cancellation, and resource exhaustion. There is no
fixed output-byte or session-count limit.

A `LoadOutcome` is either a complete/partial snapshot or a fatal acquisition
error. Confirmed disappearing sessions are removed, not represented by ghosts.
Successful zero rows is an empty snapshot. A failed acquisition is not one.

The CLI serializes six flat keys through a DTO defined in the CLI
contract, not the internal enum shapes or ID/window metadata. Unavailable values
become null, diagnostics go to stderr, and the command exits 1.

## Uptime

Derive uptime as whole elapsed seconds between creation and snapshot capture.
For TUI display between automatic refreshes, add monotonic elapsed time locally;
this updates uptime without querying tmux. Other fields remain a snapshot.

Human formatting includes all units from the largest to the smallest nonzero
unit, including intermediate zeros, and omits leading and trailing zeros; zero
is `0s`.
JSON uses integer seconds. No negative durations or counter wrap are allowed.
A creation timestamp later than capture time is unavailable with a clock-related
diagnostic rather than a silent clamp to zero.

## TUI state and transitions

`AppState` contains the current snapshot (if any), focused session ID, scroll
position, current screen (`List` or `Help`), viewport dimensions, load state,
retry timing, a current crawl pass, and diagnostic/warning state. A crawl pass
owns the values it started with; it advances one cell per second and the next
pass uses the latest snapshot. It is independent of the terminal backend.

| Current state | Event | Next state / visible effect |
|---------------|-------|-----------------------------|
| Initial | Start | `Loading`; show loading feedback, allow quit/help |
| Loading | Complete snapshot | `Fresh`; show list or explicit empty state |
| Loading | Partial snapshot | `Partial`; show available values and warning, not success-only feedback |
| Loading | Fatal error | `Failed`; show cause/recovery and retry automatically |
| Fresh/Partial | Regular refresh due | `Refreshing` if no request is running |
| Failed/Stale | Retry due, 5 seconds after failure finishes | `Refreshing`; skip regular refreshes until then |
| Refreshing | Complete snapshot | `Fresh`; replace data; retain partial warning through any running partial crawl pass |
| Refreshing | Partial snapshot | `Partial`; replace with available current data and warning |
| Refreshing | Fatal error | `Stale` with prior data, otherwise `Failed` |
| Any | Resize | Recompute layout/size warning, preserve screen/focus; no query |
| Any | Help/list navigation | Change screen or scroll without changing active session |
| Any | Quit/Ctrl-C | Cancel worker, restore terminal, exit |

Scheduled refreshes while a load is in flight are skipped. A partial result is
retried at the next regular refresh. A focused ID surviving a refresh stays
focused even after rename. If removed, focus moves to the next higher numeric ID,
or nearest lower if none is higher, or none if empty. Focus change or removal
ends the old crawl; a refresh alone does not restart its current pass.

## Process ownership

One request owns one 5-second deadline, including version/context/name queries,
race reconciliation, and pipe reads. One worker owns each spawned tmux child.
Timeout, exit or cancellation kills/reaps that child and joins pipe readers.
No child handles are dropped while processes can continue unowned. A field
failure can yield a partial result; an exhausted request deadline is a load
failure so the TUI marks retained data stale.
Interrupt, SIGTERM and SIGHUP cause cancellation and cleanup, returning 130,
143 and 129 respectively unless TUI cleanup fails (status 1). TUI exit cleanup
has no fixed deadline; repeated exit requests indicate cleanup progress. Ctrl-Z
restores the terminal before suspension and redraws after resume.
