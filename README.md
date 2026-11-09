# Sagebrush

Sagebrush is a read-only tmux session browser, currently under development.

## Local commit checks

Install the repository hook with `git config core.hooksPath .githooks`. It
checks formatting and Clippy on a temporary export of the exact staged content,
without changing the worktree or index. Run `cargo fmt --all` yourself before
staging if formatting is needed.

To explicitly bypass the local commit checks for a single commit, run
`SAGEBRUSH_SKIP_CHECKS=1 git commit`. This does not bypass merge or release
requirements.

## Hosted checks

The quality-check workflow runs formatting, Clippy, tests, and a build on pull
requests and pushes, with tmux available for integration tests. A manually
dispatched run also reaches the `release` environment only after those checks.
Repository maintainers must configure branch rules to require the checks and
protect every release path, including the `release` environment, with no
administrative bypass. The workflow alone does not enforce those host settings
or publish a release; their configuration and verification are tracked in T045.
