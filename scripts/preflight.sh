#!/usr/bin/env bash
# Mirror of .github/workflows/ci.yml — run before pushing to avoid the
# round-trip of "push → wait for CI → red → fix → push again".
#
# If any check fails, exit non-zero so the pre-push hook blocks the push.
# Run manually with: ./scripts/preflight.sh
# Skip in an emergency with: SKIP_PREFLIGHT=1 git push  (not recommended)
set -euo pipefail

# Resolve repo root regardless of CWD when invoked from a hook.
REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

if [[ "${SKIP_PREFLIGHT:-0}" == "1" ]]; then
    printf '\033[33m[preflight] SKIP_PREFLIGHT=1 — skipping checks\033[0m\n' >&2
    exit 0
fi

step() { printf '\033[36m[preflight] %s\033[0m\n' "$*" >&2; }
fail() { printf '\033[31m[preflight] FAILED: %s\033[0m\n' "$*" >&2; exit 1; }

step 'cargo fmt --all --check'
# shellcheck disable=SC2016  # backticks here are rendered as literal text, not shell exec
cargo fmt --all --check || fail 'rustfmt: run `cargo fmt --all` and recommit'

step 'cargo machete'
cargo machete || fail 'unused dependencies: remove them and recommit'

step 'cargo deny check'
cargo deny check 2>&1 || fail 'cargo-deny: fix the advisory/license/ban issues above'

step 'cargo clippy --workspace --all-targets --features ascii-agents-core/test-renderer -- -D warnings'
cargo clippy --workspace --all-targets \
    --features ascii-agents-core/test-renderer \
    -- -D warnings \
    || fail 'clippy: fix the warnings above and recommit'

step 'cargo test --workspace --features ascii-agents-core/test-renderer'
cargo test --workspace --features ascii-agents-core/test-renderer \
    || fail 'tests: fix the failing tests above and recommit'

# Stamp so pre-push can skip redundant re-run.
STAMP_DIR="${REPO_ROOT}/target/.preflight"
mkdir -p "$STAMP_DIR"
git rev-parse HEAD > "$STAMP_DIR/last-commit" 2>/dev/null || true

printf '\033[32m[preflight] all checks passed\033[0m\n' >&2
