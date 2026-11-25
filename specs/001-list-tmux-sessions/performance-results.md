# Performance evidence

## Early shared-acquisition measurement

- Environment: Linux 7.2.11-100.fc43.x86_64, x86_64, tmux 3.8, rustc 1.99.0.
- Command: `cargo test --locked --test tmux_integration -- --nocapture`.
- Scenario: one isolated server with 100 detached sessions, timed from the start
  of shared acquisition until its snapshot was returned.
- Raw result: 656.865658 ms; all 100 sessions returned without field issues.

This is one development measurement, not release acceptance evidence. It does
not measure visible CLI or TUI completion, refresh visibility, or resize. The
20-trial interface measurements required by T041 were collected subsequently.

## Visible-completion measurements

Measured 2026-11-06 on Linux x86_64 (kernel 7.2.11-100.fc43), Rust 1.99.0,
tmux client/server 3.8, using a debug build and an isolated `TMUX_TMPDIR`.
The base commit is `2ed439a25825f31236227d280e66a5a8a1e51bc5`; the measured
implementation was an uncommitted working tree. Binary SHA-256:
`98a7af78cc7d96d7919cdcb2e3036d8e0f66b03319d988142fa3b48a4220e3be`.
Harness SHA-256:
`1e855d03f26d2c647256d740700df2a0f7e652aed90d1059a15f97a953c0e301`.

Commands: `cargo build --locked` and `python3 scripts/measure-performance.py`.
The harness creates 100 sessions on its own server, reads the complete JSON CLI
output, and interprets PTY display updates to identify rendered TUI rows. For
refresh it renames a visible session, changes its window count, removes a
visible session and creates another, then waits for both the updated row and
the replacement visible row at 120 columns. For resize it first blocks a
refresh subprocess and confirms the process is still blocked throughout all
20 resizes; the undersized warning counts as the usable 39x10 layout. It
confirms the subprocess is reaped on exit and compares server state before and
after the read-only runs. The harness cleans up only its own server.

All entries below are seconds to observable output, not query-only times.
All observations succeeded: **20/20** initial CLI and TUI launches within
2 seconds, **20/20** refreshes within 5 seconds, and **20/20** resizes within
1 second. State comparisons were unchanged for the read-only runs.

| Trial | CLI initial | TUI initial | Refresh | Resize | Size |
|---:|---:|---:|---:|---:|:---|
| 1 | 0.848053 | 0.510428 | 1.856394 | 0.001970 | 39x10 |
| 2 | 0.980410 | 0.459837 | 1.034769 | 0.001451 | 40x10 |
| 3 | 1.046156 | 0.459337 | 1.103076 | 0.002374 | 80x24 |
| 4 | 0.657538 | 0.461749 | 1.982412 | 0.003703 | 120x30 |
| 5 | 0.622509 | 0.460382 | 0.874889 | 0.000595 | 39x10 |
| 6 | 0.746238 | 0.462961 | 1.092904 | 0.000712 | 40x10 |
| 7 | 0.479308 | 0.516757 | 0.978979 | 0.001332 | 80x24 |
| 8 | 0.931548 | 0.940544 | 1.212715 | 0.002406 | 120x30 |
| 9 | 1.011978 | 0.990937 | 0.749814 | 0.000356 | 39x10 |
| 10 | 0.953346 | 0.780717 | 1.346510 | 0.000499 | 40x10 |
| 11 | 0.986150 | 1.091455 | 1.923131 | 0.000899 | 80x24 |
| 12 | 0.975061 | 1.040656 | 1.094831 | 0.001749 | 120x30 |
| 13 | 0.480667 | 1.043773 | 1.040765 | 0.000380 | 39x10 |
| 14 | 0.924656 | 0.883444 | 0.688219 | 0.000563 | 40x10 |
| 15 | 0.877667 | 0.936207 | 0.886932 | 0.000796 | 80x24 |
| 16 | 1.131297 | 1.041049 | 1.036781 | 0.001644 | 120x30 |
| 17 | 0.491646 | 0.885377 | 1.096261 | 0.000309 | 39x10 |
| 18 | 0.711044 | 1.043415 | 1.490702 | 0.000452 | 40x10 |
| 19 | 0.845543 | 1.045094 | 0.575794 | 0.000806 | 80x24 |
| 20 | 0.772695 | 0.939513 | 1.416497 | 0.001684 | 120x30 |

This is Linux-only evidence for the identified binary. T043 requires affected
acceptance checks to be repeated against the eventual final revision.

## Repeat after terminal failure-path fixes

The same harness and environment were used after the terminal failure-path
changes, with binary SHA-256
`fa18d3084d3b8a3714a264576422438b417dbef3a24e196689b8047cb3cb196b`.
All 80 observations were visible and met their thresholds (20/20 per category).
The blocked refresh was reaped and read-only state comparisons passed. Times
are seconds.

| Trial | CLI initial | TUI initial | Refresh | Resize | Size |
|---:|---:|---:|---:|---:|:---|
| 1 | 0.587692 | 0.983502 | 1.793209 | 0.001307 | 39x10 |
| 2 | 0.467490 | 0.514066 | 1.028811 | 0.001506 | 40x10 |
| 3 | 0.457051 | 0.514295 | 1.154372 | 0.003002 | 80x24 |
| 4 | 0.462171 | 0.462749 | 1.047679 | 0.005521 | 120x30 |
| 5 | 0.471347 | 0.461010 | 1.039031 | 0.001200 | 39x10 |
| 6 | 0.449566 | 0.510239 | 1.098839 | 0.000924 | 40x10 |
| 7 | 0.452980 | 0.513622 | 1.919785 | 0.001645 | 80x24 |
| 8 | 0.444224 | 0.775878 | 0.973394 | 0.002716 | 120x30 |
| 9 | 0.848451 | 0.886387 | 0.742030 | 0.000453 | 39x10 |
| 10 | 0.932652 | 0.992340 | 1.312777 | 0.000662 | 40x10 |
| 11 | 0.991237 | 1.096333 | 1.092056 | 0.000941 | 80x24 |
| 12 | 1.004561 | 1.043096 | 1.094457 | 0.001780 | 120x30 |
| 13 | 1.013209 | 0.724104 | 0.921408 | 0.000310 | 39x10 |
| 14 | 0.970275 | 0.567283 | 1.034274 | 0.000443 | 40x10 |
| 15 | 0.447898 | 0.939569 | 0.620637 | 0.000856 | 80x24 |
| 16 | 0.922385 | 0.724946 | 1.033444 | 0.001694 | 120x30 |
| 17 | 0.800640 | 0.886546 | 1.033335 | 0.000388 | 39x10 |
| 18 | 0.927086 | 0.993652 | 1.342954 | 0.000527 | 40x10 |
| 19 | 0.930868 | 1.098076 | 1.192450 | 0.000916 | 80x24 |
| 20 | 1.024699 | 0.992236 | 0.981025 | 0.001766 | 120x30 |

## Repeat after convergence fixes

On 2026-11-09, `python3 scripts/measure-performance.py` ran on Linux
7.2.11-100.fc43.x86_64 with Rust 1.99.0 and tmux 3.8. The debug binary
SHA-256 was `5bc8a6ff194d83ff90a70a0b6857a6a54f69da06e436e3c2c12279d61433a388`;
the harness SHA-256 was
`1e855d03f26d2c647256d740700df2a0f7e652aed90d1059a15f97a953c0e301`.
All 20 trials in each category met their thresholds. Read-only state checks
passed, all resizes occurred during a blocked request, and the request was
reaped. Times are seconds.

| Trial | CLI initial | TUI initial | Refresh | Resize | Size |
|---:|---:|---:|---:|---:|:---|
| 1 | 0.912177 | 0.510769 | 1.964677 | 0.001614 | 39x10 |
| 2 | 0.941431 | 0.510326 | 0.977871 | 0.002315 | 40x10 |
| 3 | 0.985182 | 0.511422 | 1.140537 | 0.002835 | 80x24 |
| 4 | 0.936528 | 0.513719 | 1.971011 | 0.005122 | 120x30 |
| 5 | 0.968509 | 1.039800 | 0.984653 | 0.001232 | 39x10 |
| 6 | 0.958363 | 0.994596 | 1.150331 | 0.001179 | 40x10 |
| 7 | 0.909991 | 0.988503 | 1.855566 | 0.001983 | 80x24 |
| 8 | 0.967960 | 0.988761 | 0.791497 | 0.003125 | 120x30 |
| 9 | 0.974901 | 0.934186 | 1.348562 | 0.000544 | 39x10 |
| 10 | 0.936926 | 0.829276 | 1.039132 | 0.000878 | 40x10 |
| 11 | 0.635799 | 0.988082 | 1.038255 | 0.001383 | 80x24 |
| 12 | 0.699790 | 0.616746 | 1.036177 | 0.002311 | 120x30 |
| 13 | 0.881810 | 0.828900 | 0.818131 | 0.000509 | 39x10 |
| 14 | 0.873783 | 0.936187 | 0.904793 | 0.000463 | 40x10 |
| 15 | 0.971823 | 1.037784 | 0.933144 | 0.000917 | 80x24 |
| 16 | 0.953836 | 0.938392 | 1.033243 | 0.002010 | 120x30 |
| 17 | 0.944494 | 0.936693 | 1.094758 | 0.000493 | 39x10 |
| 18 | 0.649379 | 0.989884 | 1.421130 | 0.000782 | 40x10 |
| 19 | 0.491319 | 0.985828 | 0.576697 | 0.001196 | 80x24 |
| 20 | 0.482575 | 0.994755 | 1.465011 | 0.002088 | 120x30 |

This is preliminary Linux evidence; final-revision acceptance remains subject
to T043 after required automated platform and performance evidence is collected.

## Repeat after help-workflow changes

On 2026-11-09, `cargo build --locked` followed by
`python3 scripts/measure-performance.py` ran on Linux
7.2.11-100.fc43.x86_64 with Rust 1.99.0 and tmux 3.8. The base commit was
`2ed439a25825f31236227d280e66a5a8a1e51bc5`; the implementation was uncommitted.
The debug binary SHA-256 was
`23ac69143ce95dbb844b2514fa64793c12f8b60d146f101a102304249e8de533`;
the harness SHA-256 was
`1e855d03f26d2c647256d740700df2a0f7e652aed90d1059a15f97a953c0e301`.
All 20 observations in each category were visible and met their thresholds.
The read-only comparisons passed, all resizes occurred while a refresh was
blocked, and the blocked process was reaped. The host was shared, with two
logged-in users and load averages of 1.44, 1.21 and 0.87 before the run; an
otherwise idle host cannot be independently guaranteed. Times are seconds.

| Trial | CLI initial | TUI initial | Refresh | Resize | Size |
|---:|---:|---:|---:|---:|:---|
| 1 | 0.716305 | 0.509769 | 1.033644 | 0.001953 | 39x10 |
| 2 | 0.692258 | 0.513177 | 1.038837 | 0.002045 | 40x10 |
| 3 | 1.095344 | 0.511272 | 1.034998 | 0.002924 | 80x24 |
| 4 | 0.959103 | 0.885360 | 1.094419 | 0.005220 | 120x30 |
| 5 | 0.961940 | 0.723257 | 2.033171 | 0.001221 | 39x10 |
| 6 | 1.132568 | 0.778438 | 1.848062 | 0.001361 | 40x10 |
| 7 | 1.075962 | 1.147588 | 1.194093 | 0.002270 | 80x24 |
| 8 | 1.092288 | 1.047737 | 1.681198 | 0.003293 | 120x30 |
| 9 | 1.145831 | 1.101283 | 1.311352 | 0.000888 | 39x10 |
| 10 | 0.759839 | 1.097416 | 1.038554 | 0.000873 | 40x10 |
| 11 | 0.748570 | 1.098270 | 1.102231 | 0.001298 | 80x24 |
| 12 | 1.117474 | 0.780819 | 2.011821 | 0.003066 | 120x30 |
| 13 | 0.694393 | 1.099212 | 1.511813 | 0.001047 | 39x10 |
| 14 | 0.527015 | 1.144964 | 1.032815 | 0.001248 | 40x10 |
| 15 | 1.075131 | 1.044225 | 1.043244 | 0.002110 | 80x24 |
| 16 | 1.062640 | 0.988337 | 1.470284 | 0.004026 | 120x30 |
| 17 | 0.776248 | 0.990332 | 0.625624 | 0.000921 | 39x10 |
| 18 | 0.493723 | 1.044706 | 1.525687 | 0.001292 | 40x10 |
| 19 | 0.484132 | 0.989084 | 1.849635 | 0.001858 | 80x24 |
| 20 | 0.496713 | 1.041844 | 0.861078 | 0.003677 | 120x30 |

## Repeat with observed baseline load

The same binary and harness were rerun on 2026-11-09 after five one-second
`/proc/stat` baseline samples, which showed 96.90%, 96.71%, 96.41%, 96.70%
and 97.01% aggregate CPU idle on the 20-CPU host. During the harness, 67
one-second samples showed 82.01% mean and 62.34% minimum aggregate idle;
these include the harness and its tmux workload and do not attribute activity
to individual processes. The host is shared, so the baseline demonstrates
low observed load, not exclusive use. All 20 trials in each category met their
thresholds, read-only comparisons passed, and the blocked request was reaped.
Times are seconds.

| Trial | CLI initial | TUI initial | Refresh | Resize | Size |
|---:|---:|---:|---:|---:|:---|
| 1 | 0.879963 | 0.512410 | 2.002611 | 0.000691 | 39x10 |
| 2 | 1.054659 | 0.512253 | 2.082041 | 0.000573 | 40x10 |
| 3 | 0.987531 | 0.509114 | 1.983720 | 0.001070 | 80x24 |
| 4 | 0.923586 | 0.726361 | 2.089872 | 0.002074 | 120x30 |
| 5 | 1.025211 | 0.993588 | 1.908228 | 0.000444 | 39x10 |
| 6 | 0.995895 | 0.938436 | 0.803256 | 0.000629 | 40x10 |
| 7 | 1.002433 | 0.987579 | 1.200213 | 0.001104 | 80x24 |
| 8 | 0.994133 | 1.040409 | 1.083967 | 0.002087 | 120x30 |
| 9 | 0.956818 | 1.040933 | 0.923482 | 0.000404 | 39x10 |
| 10 | 0.950801 | 1.047809 | 0.913684 | 0.000633 | 40x10 |
| 11 | 0.541831 | 1.099882 | 1.320447 | 0.001665 | 80x24 |
| 12 | 0.883105 | 1.146642 | 1.509283 | 0.002382 | 120x30 |
| 13 | 0.993753 | 1.042915 | 1.034681 | 0.000518 | 39x10 |
| 14 | 0.954166 | 1.098444 | 1.262748 | 0.000729 | 40x10 |
| 15 | 0.931535 | 0.883498 | 1.252413 | 0.001030 | 80x24 |
| 16 | 0.952612 | 0.830449 | 0.625629 | 0.003223 | 120x30 |
| 17 | 0.881783 | 1.097546 | 1.415901 | 0.000660 | 39x10 |
| 18 | 0.501281 | 1.152410 | 1.031473 | 0.000874 | 40x10 |
| 19 | 0.499038 | 0.993389 | 1.097380 | 0.001373 | 80x24 |
| 20 | 0.496519 | 1.099903 | 2.084261 | 0.002010 | 120x30 |
