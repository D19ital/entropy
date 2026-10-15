#!/usr/bin/env bash
# Test only: never imports a key. Restore the temporary title edit on exit.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
: "${TEST_ROOT:?Set TEST_ROOT outside the checkout}"
[[ "$(uname -s)" == Darwin && "$(uname -m)" == arm64 ]] || {
    echo 'Requires native ARM64 macOS (no Rosetta)' >&2; exit 1;
}
[[ "$(rustc -vV)" == *'host: aarch64-apple-darwin'* ]] || exit 1
[[ ! -e "$TEST_ROOT" ]] || { echo 'TEST_ROOT must be fresh' >&2; exit 1; }
umask 077
mkdir -p "$TEST_ROOT"
cp "$ROOT/src/main.rs" "$TEST_ROOT/main.rs.original"
restore_title() { cp "$TEST_ROOT/main.rs.original" "$ROOT/src/main.rs"; }
trap restore_title EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
cd "$ROOT"
for variant in A B; do
    # APP_TITLE is a Rust const, not an environment variable. Keep bundle names fixed.
    python3 - "$TEST_ROOT/main.rs.original" "$ROOT/src/main.rs" "$variant" <<'PY'
import pathlib, sys
original, output, variant = sys.argv[1:]
text = pathlib.Path(original).read_text()
needle = 'const APP_TITLE: &str = "Entropy";'
assert text.count(needle) == 1, 'APP_TITLE mechanism changed; review experiment'
pathlib.Path(output).write_text(text.replace(needle, f'const APP_TITLE: &str = "Entropy — signing test {variant}";'))
PY
    # Reuse compilation cache, but save A before cargo overwrites entropy for B.
    CARGO_TARGET_DIR="$TEST_ROOT/cargo" cargo build --locked --release --target aarch64-apple-darwin
    destination="$TEST_ROOT/build-$variant/aarch64-apple-darwin/release"
    mkdir -p "$destination"
    cp "$TEST_ROOT/cargo/aarch64-apple-darwin/release/entropy" "$destination/entropy"
    [[ "$(lipo -archs "$destination/entropy")" == arm64 ]]
done
! cmp -s "$TEST_ROOT/build-A/aarch64-apple-darwin/release/entropy" "$TEST_ROOT/build-B/aarch64-apple-darwin/release/entropy"
echo 'Built two different native ARM64 executables; no key imported.'
