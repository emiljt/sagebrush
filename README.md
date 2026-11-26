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

## Hosted checks and releases

The quality-check workflow runs formatting, Clippy, tests, and a build on pull
requests and pushes, with tmux available for integration tests.

Releases follow [Semantic Versioning](https://semver.org/) (`vMAJOR.MINOR.PATCH`)
and [Conventional Commits](https://www.conventionalcommits.org/) for PR/commit
history and `--generate-notes` release notes (categorized via
`.github/release.yml`):

1. Ensure the target commit is on `master` and `Cargo.toml` `version` matches
   the release version.
2. Push a matching `v*` tag (restricted to Repository Admins by the
   `release-tags` ruleset; existing tags cannot be moved or deleted by anyone
   under `release-tags-immutable`).
3. `release-gate` runs only after `checks` passes, verifies SemVer format,
   `Cargo.toml` version match, and `master` ancestry, enters the `release`
   environment (`v*` custom tag policy, `can_admins_bypass: false`), builds the
   release binary, and publishes `.tar.gz` + `.sha256` with `gh release create
   --verify-tag --generate-notes`.
