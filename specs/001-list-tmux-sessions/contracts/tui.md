# TUI Contract

**Scope**: Read-only session inspection using ratatui and its Crossterm backend.
The [CLI contract](cli.md) defines launch preconditions and process statuses.

## Screens and keys

| Key | Session list | Help |
|-----|--------------|------|
| `j` / `k` | Move focus down / up | Scroll help down / up |
| Esc | Stay on list | Return to list |
| `?` | Open help | Stay on help |
| `q` | Exit normally | Exit normally |

These are the only supported TUI keys; other keys are ignored except normal
terminal signal handling. There is no details screen or manual refresh key.

Help documents every binding and explicitly distinguishes focused row from active
session. Help explains automatic refresh and the detail crawl. No mouse is required.

Focus/scroll changes never attach, switch, rename, resize, create, or destroy
tmux objects. Normal `q` exit is 0 after cleanup, including after an already
displayed recoverable refresh error; Ctrl-C is 130. Fatal terminal/runtime
errors are 1 and are reported after restoration. Errors displayed in the TUI
must remain visibly actionable rather than disappearing behind an exit status.

## Layout and complete information

- Support viewports >=40 columns and >=10 rows.
- At >=120 columns, names up to 40 cells are fully visible and every field is
  represented. Indicate truncated values; full values are available in the crawl.
- At supported widths below 120 columns, each row shows name and active status;
  window count, uptime and dimensions appear only in the selected row's crawl.
- Truncate names only at grapheme boundaries, using `...` as an explicit marker.
  Measure display cells after control escaping, not UTF-8 bytes.
- The focused row has a non-color-only focus indicator; active status is a
  separate `*` marker whose meaning is shown in help/legend.
- Every key-value pair, including the complete name, crawls repeatedly across
  the focused card at one terminal cell per second, without a key press. Do not
  draw partially visible wide characters; leave their partial cells blank.
- Refresh does not restart a pass while the same session stays focused; the
  current pass retains its values and the next uses the latest snapshot. A
  focus change or removal ends the old pass.
- Above/below minimum-size transitions preserve the focused session and screen.
  Below either minimum dimension, show a size warning clipped safely to the
  available cells. If either dimension is zero, draw nothing. `q` and Ctrl-C
  still work; return to the prior view when the viewport is usable.

Resize events relayout immediately without waiting for or requesting tmux I/O.
Preserve focus by stable session ID across rename, refresh, and resize. On focused
session removal, focus the next higher numeric session ID, otherwise the nearest
lower ID, or no row for an empty list. Scrolling must make every session
reachable, not just rendered rows.

## Loading, refresh, and errors

Load on entry and automatically refresh while open, initially scheduling regular
refreshes at 1-second monotonic intervals. A worker performs the request
while the event loop remains available for resize/help/quit; at most one request
is in flight and overlapping scheduled refreshes are skipped. A complete request
has a 5-second deadline. After failure, skip regular refreshes and retry 5 seconds
after it finishes; partial results retry at the next regular refresh.

| State | Required feedback |
|-------|-------------------|
| Loading without data | `Loading sessions...`, help and quit |
| Fresh empty | `No tmux sessions found.`, help/quit |
| Fresh populated | Session list and navigation |
| Partial | Available rows, `-` for unavailable fields, warning |
| Refreshing existing view | Previous data with explicit refreshing indicator |
| Failed initial load | Error cause and known recovery; automatic retry/help/quit, not empty success |
| Failed refresh with prior view | Persistent stale-data banner, error and known recovery, prior rows still inspectable |

A subsequent complete successful refresh clears stale/error feedback, except a
partial-data warning remains until an in-progress partial-data crawl pass ends.
A partial refresh displays current available values with warning, subject to the
crawl-pass rule. Errors must remain visibly actionable.
Show unavailable active context distinctly from known inactive status.

Uptime may tick locally from the snapshot without fetching tmux. Refresh resamples
authoritative values; a crawl pass remains a snapshot until its next pass.

## Lifecycle and capabilities

Enter raw mode/alternate screen only after terminal preconditions pass. Use
fallible setup and a scoped owner that restores state after normal exit, raw-mode
Ctrl-C, draw/read errors, initialization failure after partial setup, and panic
unwinding. Exit cancels outstanding work; cleanup has no fixed deadline. A repeated
exit request during cleanup indicates progress. Report cleanup failures and exit
1; never call process exit while raw mode is owned. SIGINT, SIGTERM and SIGHUP
return 130, 143 and 129 respectively unless cleanup fails; Ctrl-Z restores the
terminal before suspension and re-enters/redraws the TUI on resume.

Color is supplemental; honor global color-disable controls. Do not print raw
session names or diagnostics containing terminal controls. Keep all commands
keyboard accessible; no Unicode-only decoration is required for comprehension.

## Acceptance surfaces

TestBackend tests cover 120x30, 80x24, 40x10, 39x10, 40x9, 119x30 and zero-area
draws; 120-column boundary; long names, wide characters and combining marks;
distinguishable focus/active/unavailable markers; crawl passes and help scrolling;
loading/partial/failed/stale banners; and focus recovery after rename/removal.
PTY tests cover real launch detection, all exit paths, resize during an in-flight
request, and terminal restoration, which TestBackend alone cannot establish.
