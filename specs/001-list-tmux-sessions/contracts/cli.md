# CLI Contract

**Scope**: Public commands for [List tmux Sessions](../spec.md).
Built-in help must explain this entire user-facing contract without requiring
this document or an internet connection.

## Commands

```text
sagebrush [--no-color]
sagebrush tui [--no-color]
sagebrush list [--json | --plain] [--no-color]
sagebrush -h | --help
sagebrush -V | --version
sagebrush <command> -h | --help
```

`--no-color` is global and may appear before or after a subcommand. With no
subcommand, it changes presentation only; it does not select listing.

| Invocation | Behavior |
|------------|----------|
| No subcommand, stdin/stdout both terminals | Open TUI |
| No subcommand, either stream not a terminal | Concise usage on stderr; empty stdout; exit 2 |
| `tui`, stdin/stdout both terminals | Open TUI |
| `tui`, either stream not a terminal | Actionable usage error; exit 2 |
| `list` | Human-readable full-field table, no prompts or full-screen mode |
| `list --json` | Formatted JSON array |
| `list --plain` | Headerless tab-separated text, one record per line |
| `list --json --plain` | Conflicting options; usage diagnostic on stderr; exit 2 |
| Help/version | stdout and exit 0, before tmux access or terminal initialization |
| Unknown flags/commands or extra arguments | Actionable usage diagnostic on stderr; exit 2 |

Inside tmux, listing uses the invoking server, including detached sessions;
outside tmux it uses the default server. An unresolvable inherited server
context is an error, not a fallback. Listing never creates a server. There are
no session mutation, filtering, custom socket, or persistent configuration
options in this feature.

## Data and ordering

Every output format exposes name, total window count, creation-based uptime,
current-window dimensions, and invoking-context active status. Sort records by
numeric session ID ascending; do not reorder based on focus, attachment, locale,
or uptime. Session IDs are internal ordering/identity, not extra required output
fields in this first public schema.

Outside tmux, all active values are false. Inside tmux, at most one row is true.
If a required value is unavailable, do not substitute false/zero/empty text;
follow the partial-result rules below.

### Human table

Headers: `NAME`, `WINDOWS`, `UPTIME`, `SIZE`, `ACTIVE`.
Use whole-second uptime with `d`, `h`, `m`, `s` and no spaces, retaining
intermediate zeros and omitting leading/trailing zero units (`0s`, `1d`,
`23h0m59s`). Use `WIDTHxHEIGHT` for size, and `yes`/`no` for active.
Render unavailable fields as `unavailable`. Do not truncate names or fields to
terminal width, and do not use a pager. Wide output may extend past the viewport
but remains complete in the stream.

Escape backslash as `\\`, tab as `\t`, newline as `\n`, carriage return as `\r`,
and remaining terminal control characters as Unicode escapes. Escape controls
in diagnostics as well. Printable Unicode remains readable. Never emit
user-controlled terminal escape sequences.

### Plain records

The same five fields appear in the same order, separated by a literal tab and
terminated by one newline per session. No headers, alignment padding, color,
progress messages, or empty-state prose. Text escaping matches the human table.
Use `--json` when exact original strings or typed fields are needed.

### JSON

Successful stdout is exactly one pretty-printed JSON array followed by a newline,
with no headings, progress text, styling, or diagnostics:

```json
[
  {
    "name": "work",
    "windows": 3,
    "uptime": 3672,
    "width": 120,
    "height": 30,
    "active": true
  }
]
```

| Field | Complete-result type and invariant |
|-------|------------------------------------|
| `name` | String, original full name; normal JSON escaping |
| `windows` | Integer >=1 |
| `uptime` | Integer >=0, creation-to-capture whole seconds |
| `width` | Integer >=1, terminal cells |
| `height` | Integer >=1, terminal cells |
| `active` | Boolean |

All six keys are always present. Each unavailable field is null independently,
including each dimension. JSON output
uses raw typed data rather than human-formatted durations or escaped-text-table
values. Treat key names, types, units, and empty-array semantics as compatibility
contracts; document any future breaking changes rather than silently changing them.

## Empty, partial and failed results

| Outcome | stdout | stderr | Status |
|---------|--------|--------|--------|
| Complete populated result | Requested data format | No routine diagnostics | 0 |
| Empty result, terminal human table | Brief `No tmux sessions found.` | Empty | 0 |
| Empty result, redirected table or plain | Empty | Empty | 0 |
| Empty result, JSON | `[]` plus newline | Empty | 0 |
| Partial result with known row IDs | Available rows, unavailable/null fields | Explain affected fields and recovery | 1 |
| Fatal query/parse/access/timeout failure | Empty; no success-shaped fallback | Cause and recovery when known | 1 |
| Usage/precondition error | Empty | Concise explanation, usage and help hint | 2 |
| Ctrl-C cancellation | No further data | Brief cancellation message when appropriate | 130 |
| SIGHUP / SIGTERM | No further data | Diagnostic when appropriate | 129 / 143 |
| Early closed output pipe | No further data | No stack trace | 141 |

Buffer a listing before writing stdout; a fatal query failure must not leave a
prefix of a JSON array or an apparent complete table. Handle output-write failures
explicitly. An early closed pipe terminates promptly without a Rust panic or
stack trace; never report a truncated write as successful output.

A recognized absent server or no sessions is empty, not an access error. Missing
tmux, unresolvable server context, unsupported or unverifiable client/server
versions, permission failures, malformed output, resource exhaustion, and
timeouts must never be normalized to empty. The session request has a 5-second
deadline and no fixed session-count or tmux-output byte limit. Both client and
running server must be tmux 3.3 or newer (3.3a and 3.4a accepted; 3.3 prereleases
rejected); no server requires only a verified client. Fatal queries leave stdout
empty; never emit truncated results as complete.

## Help, environment and color

Root help explains purpose, default TUI behavior, explicit `tui`/`list`, and
examples. `list --help` explains output flags, all field definitions, escaping,
ordering, unavailable/null/empty behavior, status codes and representative pipes.
`tui --help` explains terminal requirements and how to reach keyboard help.
Help must include actionable recovery for missing tmux, server access, terminal
size, failed refresh, and invalid input. Version/help must work without tmux.

Document that no configuration file is read. Sagebrush uses the invoking server
inside tmux and the default server outside it, without a separate server-selection
flag. `NO_COLOR`, `TERM=dumb`, and `--no-color` disable color.
Non-terminal stdout/stderr each independently disable styling on that stream.
JSON and plain never contain styling, regardless of terminal attachment.

Do not auto-page or animate CLI listing. Do not query remote services, send
telemetry, or write session data to disk.
