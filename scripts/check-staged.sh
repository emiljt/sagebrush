#!/bin/sh
set -eu

if [ "${SAGEBRUSH_SKIP_CHECKS:-0}" = "1" ]; then
    printf '%s\n' "Sagebrush: staged checks explicitly bypassed." >&2
    exit 0
fi

root=$(git rev-parse --show-toplevel) || {
    printf '%s\n' "Sagebrush: cannot locate the Git repository." >&2
    exit 1
}
git -C "$root" write-tree >/dev/null || {
    printf '%s\n' "Sagebrush: cannot snapshot the Git index; resolve conflicts first." >&2
    exit 1
}
snapshot=$(mktemp -d "${TMPDIR:-/tmp}/sagebrush-staged.XXXXXXXX") || {
    printf '%s\n' "Sagebrush: cannot create staged snapshot." >&2
    exit 1
}
trap 'rm -rf -- "$snapshot"' EXIT
trap 'exit 1' HUP INT TERM
git -C "$root" checkout-index --all --prefix="$snapshot/" || {
    printf '%s\n' "Sagebrush: cannot export staged content." >&2
    exit 1
}
cd "$snapshot"
if ! cargo fmt --all -- --check; then
    printf '%s\n' "Sagebrush: staged formatting check failed or was unavailable." >&2
    exit 1
fi
if ! cargo clippy --locked --all-targets --all-features -- -D warnings; then
    printf '%s\n' "Sagebrush: staged lint check failed or was unavailable." >&2
    exit 1
fi
