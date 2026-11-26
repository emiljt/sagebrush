# Feature Specification: List tmux Sessions

**Feature Branch**: `001-list-tmux-sessions`

**Created**: 2026-10-22

**Status**: Draft

**Input**: User description: "Sagebrush is a tmux session manager. The main use-case is as a TUI, but most things should also be accesible through a CLI interface. Users should be able to see their current tmux sessions. Information about each session will be provided: name, window count, uptime, window size, and whether it's the active session. Sagebrush's TUI will adapt to the size of it's terminal, which will affect how much of this information can be shown."

## Clarifications

### Session 2026-10-22

- Q: What output formats should CLI session listing support? → A: The CLI should follow the [CLI Guidelines](https://clig.dev/), including human-readable defaults and explicit machine-readable output.
- Q: What should running `sagebrush` with no arguments do when it is connected to a terminal versus used in a pipe or script? → A: Open the TUI only when both standard input and standard output are terminals; otherwise print concise usage to standard error and exit nonzero, directing users to explicit CLI listing.

### Session 2026-10-26

- Q: When more sessions or details fit than the TUI can display, how should users access the hidden information? → A: Cycle details and add Vim-style movement keys for sessions.
- Q: Which exact movement keys and detail-cycling behavior should the session-list screen support? → A: `j`/`k` move down/up through sessions; the selected session's details cycle automatically within the session-list screen.

### Session 2026-10-28

- Q: How should the session list update while the TUI is open, given that no refresh key is currently defined? → A: Refresh the session list automatically while the TUI is open.
- Q: How quickly should the TUI show session changes after they occur? → A: Within 5 seconds.
- Q: How long should each automatically cycling detail remain visible before the next one appears? → A: All key-value pairs crawl across the selected session card at one terminal cell per second.

### Session 2026-10-29

- Q: Which actions must be blocked when a required code check fails or cannot run? → A: Block commits, merges, and releases; allow ordinary local builds.
- Q: Which checks must pass before each commit, merge, and release? → A: Formatting and linting at commit; formatting, linting, automated tests, and a successful build at merge and release.
- Q: What version of the code must the commit checks evaluate when staged and unstaged changes differ? → A: The exact staged content intended for the commit.
- Q: Should anyone be allowed to bypass a required code-check gate? → A: Allow explicit local commit bypasses; never bypass merge or release gates.
- Q: What should happen when a formatting check finds code that needs changes? → A: Report the issue and block; never change files automatically.
- Q: How should the CLI represent a session when one of its information fields is unavailable? → A: Show partial records with `null` or `unavailable`, report the issue on standard error, and exit nonzero.
- Q: What JSON field names, types, and units should a complete session record use? → A: `name` (string), `windows` (integer), `uptime` (integer), `width` (integer), `height` (integer), and `active` (boolean).
- Q: What unit and precision should the JSON `uptime` integer use? → A: Whole elapsed seconds, rounded down.
- Q: How should uptime appear in the TUI and human-readable CLI output? → A: Use days, hours, minutes, and seconds without spaces, including every unit from the largest to the smallest nonzero unit and preserving zeros between them; show zero as `0s`.
- Q: How should the TUI display a session field that cannot be retrieved? → A: Show `-` for the unavailable value.
- Q: What should Sagebrush do when it receives an interrupt, including Ctrl-C in the TUI? → A: Cancel work, clean up, restore the terminal when applicable, and exit with status 130.
- Q: How should Sagebrush handle a termination or hangup signal while it is running? → A: Cancel work and clean up, attempt terminal restoration when available, and exit with status 143 for SIGTERM or 129 for SIGHUP.
- Q: Which exit statuses should distinguish successful completion, operational failure, and invalid usage? → A: Success `0`; operational or partial-result failure `1`; usage or terminal-precondition error `2`.
- Q: What status should the TUI return when the user presses `q` after an error is displayed? → A: `q` returns 0 after a displayed recoverable error; a forced fatal exit returns 1.
- Q: What should happen when a user suspends the TUI with Ctrl-Z and later resumes it? → A: Restore the terminal, suspend, then restore and redraw the TUI on resume.
- Q: Which numeric values should count as valid session information? → A: Windows, width, and height must each be at least 1; uptime may be 0 or greater.
- Q: How should window dimensions appear in human-readable session information? → A: Width followed by `x` and height, substituting the interface's unavailable marker for either missing value.
- Q: How should Sagebrush handle a session creation time that is later than the current clock time? → A: Mark uptime unavailable and report the clock mismatch.
- Q: What should happen if Sagebrush cannot reliably identify or distinguish a session in a retrieved list? → A: Fail the retrieval and report the identity problem.
- Q: How should Sagebrush handle a session name that cannot be decoded as valid UTF-8 text? → A: Mark the name unavailable and report the decoding problem.
- Q: What should the CLI do if a program receiving its output closes the pipe before the listing is complete? → A: Stop promptly and exit with status 141, without a stack trace.
- Q: What status should the TUI return if terminal restoration fails while it is exiting? → A: Report the failure and return 1, overriding any original exit status.
- Q: How should session names containing terminal control characters appear in output? → A: Escape controls in text output and diagnostics; preserve the original name through standard JSON escaping.
- Q: What should CLI listing output if retrieving the session list fails before any reliable result is available? → A: Leave standard output empty, report the failure on standard error, and return status 1.
- Q: In what order should sessions appear in the TUI and CLI? → A: Ascending tmux session ID in both interfaces.
- Q: How should Sagebrush handle a tmux installation older than the supported version? → A: Require tmux 3.3 or newer; report older versions as unsupported and suggest upgrading.
- Q: How should the TUI recover when a session refresh fails or returns only some fields? → A: Retain and mark stale on failure; show current partial values with a warning; automatically retry and clear warnings after a complete success.
- Q: What should the TUI do if it cannot load sessions when it first opens? → A: Show an actionable error, keep help and exit available, and automatically retry.
- Q: How should the TUI handle automatic refreshes while a previous session request is still running? → A: Skip overlapping refreshes; help, resize, and exit remain available; exit cancels outstanding work.
- Q: How should Sagebrush handle malformed data returned by tmux? → A: Mark isolated invalid fields unavailable; fail on an unparseable response or unreliable identity; report every error through standard error or the TUI.
- Q: How often should the TUI retry after an initial load or automatic refresh fails? → A: Retry 5 seconds after each failed request finishes.
- Q: How long may a session request run before Sagebrush treats it as a failure and cancels it? → A: Give tmux 5 seconds to respond; keep the existing refresh timing requirements.
- Q: How quickly should the TUI finish exiting after the user presses `q` while a session request is running? → A: No fixed deadline; if exit is requested again during cleanup, show that cleanup is in progress and report any cleanup failure.
- Q: When a refresh returns partial session data, when should the TUI try again? → A: Retry on the next regular refresh.
- Q: Which session should receive focus when the currently focused session disappears during a refresh? → A: Focus the next session in list order, or the previous one if there is no next session; clear focus if none remain.
- Q: What must happen within the 5-second tmux request timeout? → A: The complete session request must finish within 5 seconds.
- Q: If the focused session disappears while other sessions are added or removed, how should Sagebrush choose the next focused session? → A: Choose the remaining session with the next higher numeric session ID, or the nearest lower ID if there is none.
- Q: How should the 5-second change-visibility requirement apply when tmux responds slowly? → A: Require visibility within 5 seconds in at least 95% of the specified trials; slower cases may take longer.
- Q: What status should the TUI return if exit cleanup fails but terminal restoration succeeds? → A: Report the cleanup failure and exit with status 1.
- Q: How should users read all help content when it does not fit on the help screen? → A: Use `j` and `k` to scroll help down and up.

### Session 2026-10-30

- Q: What should the TUI do when its terminal is too small to display the full size warning, or has no drawable cells? → A: Clip the warning to available cells; draw nothing at zero size; keep exit and resize recovery available.
- Q: At 120 columns, how should the session list handle a field value that is too long to fit alongside all the other fields? → A: Show all fields, indicate truncated values, and show complete values in the detail crawl.
- Q: Below 120 columns, where should session fields that extend beyond the available width appear? → A: Show name and active marker in each row; show all other details only in the selected row's crawl.
- Q: How much tmux output may a single session request consume before Sagebrush stops it as too large? → A: No fixed output limit.
- Q: Should the 100-session performance target also be a maximum number of sessions Sagebrush can list? → A: No fixed session-count maximum; test performance with up to 100 sessions.
- Q: What should Sagebrush do if it cannot complete a session request because system resources are exhausted? → A: Report a retrieval failure; do not present a truncated list as complete.
- Q: What should happen to the selected session's detail crawl when an automatic refresh updates its data? → A: Finish the current pass, then use the latest values on the next pass.
- Q: What should Sagebrush do if tmux is installed but its version cannot be determined? → A: Report an unverifiable-version error and do not list sessions.
- Q: If the tmux client and the running server report different versions, which must meet the 3.3 minimum? → A: Both the client and any running server must be tmux 3.3 or newer.
- Q: Which operating systems must the first release support for session listing? → A: Initially Linux, macOS, and FreeBSD; superseded by the 2026-11-09 clarification.
- Q: How should Sagebrush handle a tmux 3.3 prerelease or a lettered release such as 3.3a? → A: Accept 3.3 and lettered patches such as 3.3a and 3.4a; reject 3.3 prereleases.
- Q: Besides a supported tmux installation, may Sagebrush require other separately installed command-line programs to list sessions? → A: tmux is the only separately installed command-line program required at runtime.
- Q: After a session request fails, should regular refreshes wait until the specified 5-second retry delay has elapsed? → A: Skip regular refreshes during the delay; retry 5 seconds after the failure.
- Q: If a complete refresh follows partial data while the detail crawl still shows the previous partial values, how should the TUI handle the warning? → A: Keep the partial-data warning until the old crawl pass finishes.
- Q: How should the detail crawl display a wide character when only part of it fits at the edge of the card? → A: Show only whole characters; leave partially visible characters blank.
- Q: Which tmux server should Sagebrush inspect when it is launched inside a session on a non-default server? → A: Inside tmux, inspect the invoking server; outside tmux, inspect the default server.
- Q: What should Sagebrush do if it is launched with a tmux context but cannot determine which server that context refers to? → A: Report a server-context error and fail the request.
- Q: What if the invoking tmux client cannot be reliably identified, such as when a pane is shared by multiple sessions? → A: Mark active status unavailable with a diagnostic rather than guess; mark the invoking session active when it can be identified reliably.

### Session 2026-11-09

- Q: What should happen if someone runs Sagebrush on macOS while it is out of scope? → A: Keep macOS as best-effort, without release validation.
- Q: Should FreeBSD remain a required supported platform when its manual checks are removed? → A: Keep FreeBSD support and required automated validation; require manual checks only on Linux. Both validation requirements are superseded by later clarifications in this session.
- Q: Is FreeBSD testing required for the first release? → A: No; retain FreeBSD support, but do not require FreeBSD testing for now.
- Q: How should manual acceptance and usability tasks be handled? → A: Automate behavioral checks where feasible and defer non-automatable manual acceptance and participant studies for now; retain constitution-required review and coverage-exception checks.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Inspect sessions interactively (Priority: P1)

As a tmux user, I want to open Sagebrush and see my existing sessions and their
details so I can understand my current workspace without inspecting each session.

**Why this priority**: Session visibility is the primary value of this feature and
the TUI is the primary interface.

**Independent Test**: Open the TUI with several known sessions and compare the
displayed names, window counts, uptimes, dimensions, and active status with their
known state. Navigate through the entire list using `j`/`k`, observe details
crawling across the selected session card, open help with `?`, return with `Esc`,
and exit with `q`.

**Acceptance Scenarios**:

1. **Given** three sessions and a terminal large enough to show all fields, **When**
   the user opens the TUI, **Then** all three sessions appear once with their names,
   window counts, uptimes, current-window dimensions, and active status.
2. **Given** Sagebrush was launched inside one of those sessions and its invoking
   client's session can be identified reliably, **When** the list is shown,
   **Then** only that session has a textual or symbolic active marker, regardless
   of whether other sessions have attached clients. If that client cannot be
   reliably identified, active status is unavailable with a diagnostic instead
   of an inferred active session.
3. **Given** Sagebrush was launched outside tmux, **When** the list is shown,
   **Then** no session is marked active.
4. **Given** no sessions exist, including when no tmux server is running, **When**
   the user opens the TUI, **Then** an explicit empty state appears rather than
   an error or fabricated session.
5. **Given** more sessions than fit vertically, **When** the user navigates using
   `j`/`k`, **Then** focus moves down/up and every session is reachable.
6. **Given** the session-list screen is open, **When** the user presses `?`,
   **Then** the help screen lists every supported key and explains the automatic
   detail crawl; `Esc` returns to the session-list screen, and `q` exits from
   either screen and restores the terminal's prior usable state.
7. **Given** standard input and standard output are both terminals, **When** the
   user runs `sagebrush` without arguments, **Then** the TUI opens.
8. **Given** a session is created, removed, renamed, or changed while the TUI is
   open, **When** automatic refresh succeeds, **Then** the list reflects the change
   without a key press and preserves focus if that session still exists; visibility
   meets the 5-second measured target in SC-006 rather than an absolute deadline.
9. **Given** either TUI screen is open, **When** the user presses a key other than
   `?`, `Esc`, `q`, `j`, or `k`, **Then** it is ignored; terminal signal handling
   remains available.
10. **Given** a session field is unavailable, **When** the TUI displays it,
    **Then** its value is `-`, a warning reports the issue, and an unavailable
    active status is distinguishable from a known inactive session.
11. **Given** a recoverable error is displayed in the TUI, **When** the user
    presses `q`, **Then** the TUI completes cleanup, restores the terminal, and
    exits with status 0; a fatal error that forces the TUI to exit or a cleanup failure returns
    status 1 instead.
12. **Given** the TUI is open, **When** the user suspends it with Ctrl-Z, **Then**
    the terminal is restored before suspension; on resume, the TUI is restored and
    redrawn.
13. **Given** attached and detached sessions, **When** the TUI opens, **Then**
    both kinds appear once; attachment alone does not determine active status.
14. **Given** initial loading fails, **When** the TUI remains open, **Then** it
    displays the error and known recovery step, allows help and exit, and retries.
15. **Given** a refresh fails or returns partial data, **When** the TUI updates,
    **Then** it marks any retained failed-refresh view stale or shows the current
    partial values with a warning, subject to the in-progress crawl rule. A later
    complete refresh clears those warnings, except that a partial-data warning
    remains until an in-progress partial-data crawl pass ends.
16. **Given** a session request is running, **When** a scheduled refresh occurs,
    **Then** it is skipped; help and resizing remain available, and exit cancels
    the outstanding request.
17. **Given** an initial load or refresh request fails, **When** it finishes,
    **Then** the TUI skips regular refreshes during the delay and retries
    5 seconds later unless it is exiting.
18. **Given** a session request has not completed within 5 seconds of starting,
    **When** it times out, **Then** the TUI cancels it and reports a failure
    under the existing load or refresh recovery rules, even if tmux has
    already produced some output.
19. **Given** TUI exit cleanup is in progress, **When** the user requests exit
    again, **Then** the TUI indicates that cleanup is in progress and reports
    any cleanup failure; a cleanup failure results in status 1.
20. **Given** a refresh returns partial data, **When** the next regular refresh
    occurs, **Then** the TUI requests session data again.
21. **Given** the focused session disappears on refresh, **When** sessions remain,
    **Then** focus moves to the session with the next higher numeric session ID,
    or the nearest lower ID if there is none; otherwise focus is cleared.
22. **Given** help content exceeds the available screen space, **When** the user
    presses `j` or `k`, **Then** help scrolls down or up so all content is reachable.

---

### User Story 2 - Inspect sessions in a small or resized terminal (Priority: P2)

As a user working in differently sized terminals, I want the session list to
prioritize essential information and adapt when resized so it remains usable
without restarting Sagebrush.

**Why this priority**: A terminal manager must remain useful in split panes and
small windows, not just full-screen terminals.

**Independent Test**: View the same sessions at 120 by 30, 80 by 24, and 40 by 10
terminal cells, resize between these sizes, and confirm the list remains readable
and navigable. Shrink below the supported minimum and then restore it.

**Acceptance Scenarios**:

1. **Given** a 120-column terminal and session names up to 40 display cells,
   **When** the list is shown, **Then** all five session fields are represented;
   any value too long to fit is visibly truncated and available in full in the
   selected session's detail crawl.
2. **Given** a supported terminal width below 120 columns, **When** the list
   adapts, **Then** each row shows the name and active status, and the selected
   row's crawl shows all other details without truncating their values.
3. **Given** a name too long for its available space, **When** its row is shown,
   **Then** truncation is indicated and all key-value pairs crawl across the selected
   session card so its complete name and all fields can be read without enlarging
   the terminal or opening another screen.
4. **Given** a supported terminal size, **When** the terminal is resized to another
   supported size, **Then** the layout updates without overlapping rows, horizontal
   overflow, or losing the currently focused session if it still exists.
5. **Given** fewer than 40 columns or 10 rows, **When** the TUI is shown, **Then**
   it displays as much of a size warning as fits and still allows exit; if either
   dimension is zero, it draws nothing. Restoring a supported size restores the
   previous screen without restarting.
6. **Given** a selected session card, **When** its details are displayed, **Then**
   all key-value pairs crawl across the card at one terminal cell per second and
   repeat without requiring a key press.
7. **Given** a selected session's detail crawl is in progress, **When** automatic
   refresh updates that session, **Then** the current pass finishes with its
   existing values and the next pass uses the latest values.
8. **Given** a partial-data crawl pass is in progress, **When** a complete refresh
   succeeds, **Then** the partial-data warning remains until that pass ends.
9. **Given** a wide character crosses a card edge during the crawl, **When** only
   part of it would fit, **Then** that partial area is blank and the character is
   shown when all of its cells fit.

---

### User Story 3 - List sessions without interactive navigation (Priority: P2)

As a shell user, I want a non-interactive way to obtain the same session
information so I can inspect it directly or pass it to other shell tools.

**Why this priority**: The CLI is the companion to the primary TUI and makes the
feature usable in automation.

**Independent Test**: Use built-in help to discover session listing, run it from
inside and outside tmux, redirect its output, and compare the results with the
same session state shown in the TUI.

**Acceptance Scenarios**:

1. **Given** existing sessions, **When** the user invokes CLI listing, **Then**
   it returns every session and all five fields without prompts or a TUI.
2. **Given** output is redirected, **When** listing completes successfully,
   **Then** standard output contains complete, untruncated session data without
   terminal control sequences, diagnostics go to standard error, and the exit
   status is zero.
3. **Given** no sessions exist, **When** CLI listing runs, **Then** it produces
   no session records and exits successfully; JSON output is an empty array,
   and interactive human-readable output makes the empty state clear.
4. **Given** tmux is unavailable or access to session information fails, **When**
   either interface requests the list, **Then** it reports the cause and a recovery
   step when known; the CLI writes the diagnostic to standard error and exits
   nonzero rather than presenting the failure as an empty result.
5. **Given** the user knows only Sagebrush's root help entry point, **When** they
   follow built-in help, **Then** they can discover both interfaces and learn the
   listing options, defaults, field meanings, examples, and error recovery.
6. **Given** existing sessions, **When** the user requests `--json`, **Then**
   standard output is a valid JSON array containing all five fields for each
   session, with complete names and no human-oriented messages or styling.
7. **Given** output is piped to another command or color is disabled, **When**
   the user lists sessions, **Then** the output respects the applicable CLI
   Guidelines conventions without animations or unwanted color.
8. **Given** either standard input or standard output is not a terminal, **When**
   the user runs `sagebrush` without arguments, **Then** no TUI or automatic
   listing starts, standard output remains empty, and concise usage on standard
   error directs the user to explicit CLI listing with a nonzero exit status.
9. **Given** a known session has an unavailable field, **When** CLI listing runs,
   **Then** the session and its available values remain in the output, the missing
   value is `null` in JSON or `unavailable` in human-readable output, and a
   diagnostic on standard error accompanies a nonzero exit.
10. **Given** a session has valid numeric values, **When** it is listed, **Then**
    windows, width, and height are at least 1, uptime is nonnegative, and
    human-readable dimensions show width followed by `x` and height.
11. **Given** a session's creation time is later than the current clock time,
    **When** it is listed, **Then** uptime is unavailable and a clock-mismatch
    diagnostic is reported.
12. **Given** a retrieved session cannot be reliably identified, **When** listing
    is requested, **Then** retrieval fails with an identity diagnostic rather than
    reporting an incomplete or ambiguous list as successful.
13. **Given** attached and detached sessions, **When** explicit CLI listing runs
    outside tmux with redirected output, **Then** both kinds appear once, none is
    marked active, and results and diagnostics use their specified streams.
14. **Given** an unsupported tmux version or denied server access, **When** either
    interface requests sessions, **Then** it reports the specific problem and
    known recovery step rather than an empty session set; CLI listing exits 1.
15. **Given** tmux returns an isolated malformed field or an unparseable response,
    **When** a list is requested, **Then** the affected field is unavailable or
    retrieval fails respectively, and the error is reported to the user.
16. **Given** a session request has not completed within 5 seconds of starting,
    **When** CLI listing times out, **Then** the request is cancelled and the CLI
    reports the failure on standard error and exits with status 1, even if tmux
    has already produced some output.
17. **Given** a session request cannot complete because system resources are
    exhausted, **When** either interface reports the failure, **Then** it does
    not present a truncated session list as complete; CLI listing reports the
    problem on standard error and exits with status 1.
18. **Given** tmux is installed but its version cannot be determined, **When**
    either interface requests sessions, **Then** it reports that the version
    could not be verified and does not retrieve a new list; CLI listing exits
    with status 1, and the TUI follows its existing failed-refresh rules.
19. **Given** the tmux client or running server is older than 3.3, **When**
    either interface requests sessions, **Then** it identifies the unsupported
    component and reports the failure; if no server is running and the client
    is supported, the result remains an empty session set.
20. **Given** tmux 3.3, 3.3a, or 3.4a, **When** its version is checked, **Then**
    it satisfies the version minimum; a 3.3 prerelease does not.
21. **Given** a supported operating system and tmux installation but no other
    separately installed command-line programs, **When** session listing is
    requested, **Then** it does not depend on another command-line program.
22. **Given** Sagebrush is launched inside a session on a non-default tmux
    server, **When** either interface lists sessions, **Then** it lists sessions
    from that invoking server rather than the default server.
23. **Given** an inherited tmux context whose server cannot be determined,
    **When** either interface requests sessions, **Then** retrieval fails with
    a server-context diagnostic rather than falling back to the default server.
24. **Given** root or command help containing examples, **When** a user reads
    the help, **Then** the examples are visually separated from surrounding
    text and the descriptions use clear, concise wording. Root help clearly
    explains what happens when Sagebrush runs without a command.

### Edge Cases

- Multiple attached sessions do not imply multiple active sessions; active status
  refers only to the invoking user's tmux context.
- When the invoking client cannot be reliably identified, including ambiguous
  shared-pane contexts, active status is unavailable with a diagnostic rather
  than guessed from pane membership or client activity.
- Inside tmux, the selected server is the invoking server, even if it is not
  the default; outside tmux, the selected server is the default server.
- If an inherited tmux context cannot identify its server, retrieval fails with
  a server-context diagnostic rather than falling back to the default server.
- Sessions with different window sizes report their current window's dimensions,
  not a pane size or an average across windows.
- Very long, non-ASCII, or visually similar names remain distinguishable through
  full-name inspection; they do not disrupt neighboring rows or fields.
- A name that is not valid UTF-8 is unavailable with a decoding diagnostic;
  the identifiable session remains in the partial result.
- Terminal control characters in session names are escaped in human-readable
  output and diagnostics; JSON preserves the original name using JSON escaping.
- Newly created sessions have nonnegative uptime; unavailable individual fields
  are explicitly marked unavailable rather than replaced with invented values.
- A creation time later than the current clock time makes uptime unavailable and
  produces a clock-mismatch diagnostic rather than a fabricated duration.
- A session with a known identity and an unavailable field remains in partial CLI
  results; its unavailable field is `null` in JSON or `unavailable` in human-readable
  output, with a diagnostic on standard error and a nonzero exit.
- In the TUI, `-` marks an unavailable value; unavailable active status is
  distinguishable from known inactive status, and a warning reports the issue.
- Sessions created, removed, renamed, or changed while the TUI is open are
  reflected on refresh. A removed focused session cannot leave stale detail data;
  focus moves to the session with the next higher numeric session ID, or the
  nearest lower ID if there is none, and is cleared if no sessions remain.
- A session disappearing during a refresh is omitted from the completed view;
  failure to retrieve a still-existing session's fields is disclosed.
- If a retrieved session cannot be reliably identified or distinguished from
  other sessions, retrieval fails with an identity diagnostic rather than
  returning an incomplete or ambiguously identified list as successful.
- An isolated malformed field is unavailable; an unparseable response or unreliable
  identity fails retrieval. Every such error is reported to standard error in the
  CLI or visibly in the TUI, never silently ignored.
- A session request that has not completed within 5 seconds of starting is
  cancelled and reported as a retrieval failure, even if some output arrived.
- Session requests have no fixed tmux-output byte limit; output MUST NOT be
  silently truncated.
- More than 100 sessions are supported without a fixed session-count maximum,
  subject to the request timeout and available system resources.
- Resource exhaustion during a session request is reported as a retrieval
  failure; a truncated list is not presented as complete.
- If the tmux client's version or a running server's version cannot be determined,
  session retrieval fails with an unverifiable-version diagnostic identifying
  the component instead of assuming compatibility.
- A fatal CLI retrieval failure before a reliable result is available leaves
  standard output empty, reports the cause on standard error, and exits with
  status 1; it is not represented as an empty session set.
- A failed refresh is visibly reported; any retained previous view is identified
  as stale rather than silently presented as current.
- Following a failed request, regular refreshes are skipped until the retry
  5 seconds after the failure finishes.
- An initial load failure shows its cause and known recovery step instead of an
  empty state; help and exit remain available while automatic retries continue.
- A partial refresh replaces the displayed data with its current available values,
  marks missing fields unavailable, and shows a warning; an in-progress crawl
  finishes its current pass before showing refreshed values. A subsequent complete
  refresh clears stale and partial-result warnings, except that the partial-data
  warning remains until an in-progress partial-data crawl pass ends.
- While a session request is outstanding, help, resize, and exit remain available;
  scheduled overlapping refreshes are skipped and exit cancels the request.
- Help, exit, cancellation, and handled errors preserve terminal usability;
  active status remains understandable without color.
- Help content that does not fit on screen remains fully accessible by scrolling
  with `j` and `k`.
- Below the minimum terminal size, the size warning is clipped to the available
  cells; with zero columns or rows nothing is drawn, while exit and resize
  recovery remain available.
- At 120 columns or wider, unusually long field values may be visibly truncated
  while every field remains represented; complete values remain available in the
  selected session's detail crawl.
- At supported widths below 120 columns, window count, uptime, and dimensions
  appear only in the selected session's detail crawl, without truncating their
  values.
- A refresh does not restart an in-progress detail crawl: its current pass uses
  the values it started with, and the next pass uses the latest values. A focus
  change or removal of the focused session ends that session's crawl.
- The crawl does not draw part of a wide character at a card edge; the partially
  occupied area is blank until the whole character fits.
- TUI exit cleanup has no fixed deadline. A repeated exit request during cleanup
  produces a progress indication. Any cleanup failure is reported and results in
  status 1, even if terminal restoration succeeds.
- Interrupting the CLI or TUI cancels work and exits with status 130; if the TUI
  changed terminal state, it restores that state before exiting; a TUI cleanup
  failure is reported and results in status 1.
- SIGTERM and SIGHUP cancel work and initiate cleanup; terminal restoration is
  attempted when the terminal is available. They exit with status 143 and 129,
  respectively, unless TUI cleanup fails, which results in status 1.
- If an output pipe closes before CLI listing completes, the CLI stops promptly
  and exits with status 141 without a stack trace.
- If an attempted terminal restoration fails during TUI exit, the failure is
  reported and status 1 takes precedence over the original exit status.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Sagebrush MUST provide a keyboard-accessible TUI listing all existing
  sessions available in the selected tmux server, once per session. Inside tmux,
  the selected server MUST be the invoking server; outside tmux, it MUST be the
  default server. If an inherited tmux context cannot identify its server, the
  request MUST fail with a server-context diagnostic instead of falling back to
  the default server.
  Both interfaces MUST list sessions by ascending numeric tmux session ID and
  MUST NOT impose a fixed session-count maximum.
- **FR-002**: Each session MUST expose its full name, total window count, uptime
  since session creation, current window's width and height in terminal cells,
  and whether it is active in the invoking context. Known window counts, widths,
  and heights MUST be integers of at least 1; known uptime MUST be nonnegative.
- **FR-003**: Active status MUST identify the session containing the invoking
  tmux client, not merely an attached session or the focused list row. Outside
  tmux, no session MUST be marked active. If the invoking client cannot be
  reliably identified, active status MUST be unavailable with a diagnostic
  rather than guessed. The indicator MUST NOT rely on color alone.
- **FR-004**: Human-readable uptime in both interfaces MUST use whole elapsed
  seconds, rounded down, expressed in days (`d`), hours (`h`), minutes (`m`),
  and seconds (`s`) with no spaces. Include all units from the largest to the
  smallest nonzero unit, including intermediate zeros; omit leading and trailing
  zero units. Zero uptime is `0s`. For example, `1d`, `23h0m59s`, and `1h1m59s`
  are valid displays. Human-readable dimensions MUST use width followed by `x`
  and height, such as `120x30`, substituting the interface's unavailable marker
  for either missing dimension (for example, `unavailablex30` in the CLI and
  `-x30` in the TUI).
- **FR-005**: At 120 columns or wider, names up to 40 display cells MUST be
  fully visible and all five fields MUST be represented. If a value does not fit, its
  truncation MUST be indicated and its complete value exposed in the selected
  session's detail crawl. At supported widths below 120 columns, each row MUST
  show name and active status; window count, uptime, and dimensions MUST appear
  only in the selected session's crawl without truncating their values. All
  key-value pairs for the focused session MUST crawl across its
  card automatically and repeatedly at one terminal cell per second, exposing
  every hidden field and the complete name, across successive portions if necessary,
  without additional key presses. The crawl MUST show a wide character only when
  all of its cells fit, leaving the partial area blank otherwise. A refresh MUST
  NOT restart an in-progress crawl
  while the same session remains focused; that pass MUST finish with its existing
  values, and the next pass MUST use the latest values. A focus change or removal
  of the focused session MUST end that session's crawl.
- **FR-006**: The TUI MUST support terminals at least 40 columns by 10 rows,
  adapt to width and height changes without restarting, and allow keyboard
  access to sessions beyond the visible rows using `j` to move focus down and
  `k` to move focus up. Resizing MUST preserve focus on the same session when
  it still exists.
- **FR-007**: Below either minimum dimension, the TUI MUST display as much of a
  size warning as fits in the available cells. If either dimension is zero, it
  MUST draw nothing. Exit and resize recovery MUST remain available, and restoring
  a supported size MUST restore the previous screen without restarting.
- **FR-008**: The TUI MUST automatically refresh session membership and all fields
  while open, preserving focus when possible. When the focused session no longer
  exists, focus MUST move to the session with the next higher numeric session ID,
  or the nearest lower ID if there is none, and MUST be cleared if none remain.
  Automatic refresh MUST show changes without a key press and meet the 5-second
  measured visibility target in SC-006; slower cases may take longer. Detail
  values in an in-progress crawl follow the pass behavior in FR-005. A failed or
  partial refresh MUST be retried automatically; a subsequent complete refresh
  MUST clear stale and partial-result warnings, except that the partial-data
  warning MUST remain until an in-progress partial-data crawl pass ends. After
  initial load failure, the TUI MUST show the cause and any known
  recovery step, keep help and exit available, and retry automatically. Following
  a failed initial load or refresh, the next request MUST start 5 seconds after
  the failed request finishes, unless the TUI is exiting; regular refreshes
  scheduled during that delay MUST be skipped. A partial refresh MUST be retried
  on the next regular refresh. No refresh
  key is provided. At most one session request may be outstanding; scheduled
  refreshes during that request MUST be skipped. Help, resizing, and exit MUST
  remain available while a request runs, and exit MUST cancel outstanding work.
- **FR-009**: The CLI MUST provide non-interactive session listing with every
  field in FR-002, using the same meanings and session scope as the TUI. It MUST
  make full names and all fields available without terminal-width truncation.
  Both interfaces MUST escape terminal control characters in session names in
  human-readable output and diagnostics, while JSON preserves the original name
  through standard JSON escaping.
- **FR-010**: Successful CLI listing MUST send results to standard output and
  return status 0; operational or partial-result failures MUST send actionable
  diagnostics to standard error and return status 1. Usage and terminal-precondition
  errors MUST return status 2. If an output pipe closes before listing completes,
  the CLI MUST stop promptly and exit with status 141 without a stack trace.
  Redirected results MUST contain no terminal control sequences, and listing MUST
  NOT wait for interactive input.
- **FR-011**: Both interfaces MUST distinguish no sessions from failure to obtain
  sessions. No running server MUST be treated as no sessions when tmux is
  otherwise available. Redirected empty results MUST contain no session records;
  JSON output MUST represent an empty session set as `[]`.
- **FR-012**: Missing tmux, access failures, and unavailable fields MUST be
  explicitly reported without fabricated values. In either interface, a session
  request MUST complete within 5 seconds of starting; otherwise it MUST be
  cancelled and reported as a retrieval failure, even if tmux has already
  produced some output. Session requests MUST NOT impose a fixed limit on the
  number of tmux output bytes or silently truncate them. If system resources
  are exhausted during a request, it MUST fail with a resource diagnostic rather
  than present a truncated list as complete. Isolated malformed fields MUST
  be marked unavailable; an unparseable response or unreliable identity MUST
  fail retrieval. Every such error MUST be reported on standard error in the CLI
  or visibly in the TUI. The TUI MUST show `-` for each
  unavailable value, distinguish unavailable active status from known inactive
  status, and show a warning. After a failed refresh, the TUI MUST retain any
  previous view and visibly identify it as stale. A partial refresh MUST show
  its current available values and mark missing fields unavailable, subject to
  the in-progress crawl rule in FR-005. A CLI result with an unavailable
  field MUST retain the identifiable session and available fields, represent the
  unavailable field as JSON `null` or human-readable `unavailable`, report a
  diagnostic on standard error, and return nonzero.
- **FR-013**: Built-in root and command help MUST explain both entry points,
  arguments, options, defaults, field meanings, examples, and known recovery
  steps. Root and all command help MUST use clear, concise wording and visually
  separate examples from surrounding text. Root help MUST clearly explain
  behavior when no command is provided, including the terminal requirements and
  the non-interactive outcome in FR-017. The TUI MUST have exactly two screens:
  the session-list screen (main)
  and the help screen. `?` MUST display help, `Esc` MUST return to the session-list
  screen, and `q` MUST exit from either screen. TUI help MUST list every supported
  key and explain session navigation, the automatic detail crawl, automatic refresh,
  help scrolling, and exit. On the help screen, `j` MUST scroll down and `k` MUST
  scroll up so all help content is reachable. The only supported TUI keys are
  `?`, `Esc`, `q`, `j`, and `k`; all
  others MUST be ignored, without disabling normal terminal signal handling.
  Required usage information MUST NOT depend on external documentation.
- **FR-014**: Listing, inspection, refresh, and exit MUST NOT create, rename,
  switch, resize, or destroy tmux sessions or windows. Normal exit, cancellation,
  and handled errors MUST restore the terminal's prior usable state. On an
  interrupt, including Ctrl-C in the TUI, both interfaces MUST cancel work,
  restore any changed terminal state, and exit with status 130 if TUI cleanup
  succeeds. On SIGTERM or
  SIGHUP, both interfaces MUST cancel work and clean up, attempt restoration
  when the terminal is available, and exit with status 143 or 129 respectively
  if TUI cleanup succeeds.
  The TUI MUST return status 0 for an intentional `q` exit, including after a
  displayed recoverable error, if cleanup and terminal restoration succeed, and status 1
  for a fatal error that forces exit.
  If attempted terminal restoration fails while exiting, the TUI MUST report the
  failure and return status 1 instead of the original exit status.
  TUI exit cleanup has no fixed deadline; if exit is requested again during
  cleanup, the TUI MUST indicate that cleanup is in progress and report any
  cleanup failure. Any TUI exit cleanup failure MUST be reported and result in
  status 1, including when terminal restoration succeeds.
  When suspended with Ctrl-Z, the TUI MUST restore the terminal before suspension
  and restore and redraw the TUI when resumed.
- **FR-015**: The CLI MUST follow the applicable [CLI Guidelines](https://clig.dev/)
  for the capabilities in this feature, subject to the project constitution.
  Session listing MUST default to human-readable tabular output and support
  `--json` for a formatted JSON array with documented field names, types, and
  units covering FR-002. Machine-readable output MUST preserve full names,
  represent known active status as a boolean (or `null` when unavailable), and
  exclude human-oriented messages. Every JSON session object MUST contain
  `name` (string), `windows` (integer), `uptime` (integer), `width` (integer),
  `height` (integer), and `active` (boolean); an unavailable value MUST be `null`
  in its own field rather than omitted. `uptime` MUST be elapsed whole seconds
  since session creation, rounded down. `width` and `height` MUST be measured
  in terminal cells.
  If human-readable formatting breaks one-record-per-line processing, the CLI
  MUST also offer `--plain` for plain tabular output without that formatting.
- **FR-016**: Root and command help MUST be available through `-h` and `--help`
  without requiring a running tmux server. CLI flags MUST have full-length
  names. Color MUST be disabled independently for non-terminal output streams,
  when `NO_COLOR` is nonempty, when `TERM=dumb`, or when `--no-color` is passed.
  Non-terminal output MUST NOT contain animations or invoke an interactive pager.
  Invalid input MUST produce an actionable diagnostic and nonzero exit status
  without prompting.
- **FR-017**: Running `sagebrush` with no arguments MUST open the TUI only when
  both standard input and standard output are terminals. Otherwise, it MUST
  print concise usage to standard error, leave standard output empty, exit
  nonzero, and direct the user to the explicit CLI listing command. It MUST NOT
  automatically substitute CLI listing or wait for input. Explicit help and CLI
  listing MUST remain available regardless of terminal attachment.
- **FR-018**: Formatting and linting checks MUST pass before commits, merges,
  and releases. Automated tests and a build MUST also pass before merges and
  releases. A failed or unavailable check required for an action MUST report the
  failure and block that action, except that an explicit local commit bypass is
  allowed. Merge and release gates MUST NOT be bypassed. Ordinary local builds
  remain available without passing these gates. Commit checks MUST evaluate the
  exact staged content to be committed, including when unstaged changes differ.
  Formatting checks MUST report violations without automatically changing files.
- **FR-019**: Sagebrush MUST require the tmux client and any running server to
  be version 3.3 or newer. Final 3.3 and lettered patches such as 3.3a and 3.4a
  satisfy the minimum; 3.3 prereleases do not. An older or excluded version MUST be reported as unsupported,
  identifying the component and giving an upgrade instruction, not as an empty
  session set. If either required version cannot be determined, Sagebrush MUST
  report the component whose version could not be verified and MUST NOT retrieve
  a new session list; a TUI refresh follows the failed-refresh rules in FR-012.
  If no server is running, a supported client still yields the empty
  result specified in FR-011. CLI listing MUST return status 1 for unsupported
  or unverifiable versions.
- **FR-020**: The first release MUST support session listing on Linux and
  FreeBSD and MUST validate it on Linux using automated checks where feasible.
  FreeBSD testing is not required for release for now. Manual acceptance and
  participant studies are deferred for now, without waiving constitution-required
  review or coverage-exception checks. macOS and other Unix-like systems are
  best effort and do not require release validation.
- **FR-021**: Session listing MUST NOT require separately installed command-line
  programs other than tmux at runtime. Libraries bundled or linked with
  Sagebrush and normal operating-system services are not additional commands.

### Scope and Configuration Impact

This feature provides read-only session visibility, responsive presentation,
automatically crawling session details within the session-list screen, automatic
refresh, and a help screen. No additional TUI screens are in scope. Session creation,
attachment or switching, renaming, deletion, window management, persistence,
search, custom layouts, and multi-server aggregation are outside this feature.
No persistent user configuration is required; both interfaces use the same
selected server and field definitions. CLI output flags and the standard
`NO_COLOR` and `TERM` environment conventions affect presentation only and MUST
be explained in built-in help.

### Key Entities *(include if feature involves data)*

- **Session**: An existing tmux workspace with an identity that remains stable
  across a rename, a UTF-8 name, a creation time, a set of windows, and one
  current window. Identities must uniquely distinguish sessions in a retrieved
  list; unreliable identity makes the retrieval fail. An undecodable name is
  unavailable rather than replaced. Window count and uptime are derived from
  this state.
- **Current Window**: The window selected within a session, supplying the width
  and height shown for that session even when it is detached.
- **Invoking Context**: The user's tmux client and session context, if present,
  used to determine active status independently of list focus.
- **Session View**: The displayed session information from the latest successful
  load or refresh, together with focused-session identity, freshness status, and
  available terminal dimensions.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: With up to 100 local sessions, users receive the initial session
  list or an explicit error within 2 seconds in at least 95% of 20 measured
  launches per interface on an otherwise idle supported environment.
- **SC-002**: For an unchanged set of sessions, 100% of names, window counts,
  dimensions, and active indicators agree across both interfaces and the
  observed session state when those values are available; ambiguous active
  status is unavailable in both interfaces. Displayed uptimes agree within their
  stated precision.
- **SC-003**: At 120 by 30, 80 by 24, and 40 by 10 terminal cells, every session
  is reachable and its full name and five fields can be inspected without
  horizontal overflow. Resizing between these sizes produces a usable layout
  within 1 second in at least 95% of 20 resize trials.
- **SC-004**: Automated checks verify that built-in help describes how to list
  sessions in both interfaces, interpret active status or its unavailability,
  inspect hidden details, and exit, and that those workflows are operable. The
  former five-person, three-minute usability study is deferred for now;
  automated checks do not establish first-time-user comprehension.
- **SC-005**: In each acceptance case for an empty session set, missing tmux,
  access failure, and failed refresh, users can distinguish absence from failure;
  all CLI cases produce the specified streams and exit outcomes. For populated
  and empty session sets, `--json` output can be consumed as a JSON array without
  stripping messages or styling.
- **SC-006**: With up to 100 sessions at 120 columns or wider, automatic refresh
  displays changed membership and non-crawl details within 5 seconds in at least
  95% of 20 trials. Changes to details in an in-progress crawl appear on its next
  pass as specified in FR-005. A failed refresh displays an explicit error without
  leaving stale data unmarked.
- **SC-007**: For commits, formatting or linting failure or unavailability blocks
  the commit unless explicitly bypassed. For merges and releases, failure or
  unavailability of formatting, linting, tests, or the build blocks the action
  without bypass. Passing all required checks allows the applicable action to
  proceed. Commit checks evaluate staged content correctly in cases where unstaged
  changes would produce a different result. Formatting failures leave files
  unchanged and report the violation.
- **SC-008**: JSON session records use the six specified keys, types, and units;
  unavailable values are `null`. Human-readable uptime follows the specified
  whole-second format, and unavailable fields display as `unavailable` in CLI
  output and `-` in the TUI, with the specified diagnostics.
- **SC-009**: Interrupt, SIGTERM, and SIGHUP cases cancel work and return statuses
  130, 143, and 129 respectively, unless TUI exit cleanup fails,
  in which case the TUI reports the failure and returns 1. TUI suspension restores
  the terminal and resumption redraws the TUI. A normal `q` exit after a recoverable
  error returns 0 when cleanup and restoration succeed; a forced fatal exit or
  cleanup failure returns 1.
  An early closed CLI output pipe results in status 141 without a stack trace.
- **SC-010**: Before the first release, the session-listing acceptance scenarios
  for both interfaces are validated on Linux with supported tmux versions using
  automated checks where feasible. Coverage exceptions follow the constitution;
  non-automatable manual acceptance and participant studies are deferred for now.
  FreeBSD testing is not required for release.

## Assumptions

- Users have a local tmux client and, when running, a server, both version 3.3
  or newer, and permission to inspect the selected server. No network
  service, account, or other separately installed command-line program is required
  for session listing.
- "Current sessions" means all existing sessions in that context, including
  detached sessions, rather than only sessions with attached clients.
- "Active session" means the invoking client's session. A keyboard-focused list
  row is selection for inspection only and does not change the active session.
- "Window size" means the session's current window dimensions, not terminal
  viewport dimensions, individual pane dimensions, or dimensions of every window.
- "Uptime" means elapsed time since session creation, not time since attachment
  or last activity.
- Session information is loaded on entry and automatically refreshed while the
  TUI is open; users do not initiate refresh with a key.
- The 40-by-10 minimum and 100-session performance target are baseline product
  defaults chosen to make responsive behavior measurable; 100 sessions is not
  a listing limit.
- CLI parity applies to session listing and detail data in this feature;
  TUI-only layout and navigation do not require CLI equivalents.
- The CLI Guidelines govern applicable non-interactive and line-oriented command
  behavior, not the full-screen TUI. They do not expand this feature into unrelated
  capabilities or override the constitution's complete built-in-help requirement.
