#!/bin/sh
set -eu

if ! cargo fmt --all -- --check; then
    printf '%s\n' "Sagebrush: CI formatting check failed or was unavailable." >&2
    exit 1
fi
if ! cargo clippy --locked --all-targets --all-features -- -D warnings; then
    printf '%s\n' "Sagebrush: CI lint check failed or was unavailable." >&2
    exit 1
fi
if ! cargo test --locked --all-targets --all-features; then
    printf '%s\n' "Sagebrush: CI tests failed or were unavailable." >&2
    exit 1
fi
if ! cargo build --locked; then
    printf '%s\n' "Sagebrush: CI build failed or was unavailable." >&2
    exit 1
fi
