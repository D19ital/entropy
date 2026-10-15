#!/usr/bin/env bash
# Test-only signer; secrets are provided only to this step, after both cargo builds.
set -euo pipefail
set +x
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/macos_stable_signing.sh"
: "${TEST_ROOT:?Set TEST_ROOT}"
[[ "$(uname -s)" == Darwin ]] || { echo 'Requires macOS' >&2; exit 1; }
KEYCHAIN="$TEST_ROOT/signing.keychain-db"

cleanup() {
    local status=0 keychain
    local original=()
    if [[ -f "$TEST_ROOT/keychains.original" ]]; then
        while read -r keychain; do
            keychain="${keychain//\"/}"
            [[ -z "$keychain" ]] || original+=("$keychain")
        done < "$TEST_ROOT/keychains.original"
        security list-keychains -d user -s "${original[@]}" >/dev/null 2>&1 || status=1
    fi
    if [[ -e "$KEYCHAIN" ]]; then
        security delete-keychain "$KEYCHAIN" >/dev/null 2>&1 || status=1
    fi
    rm -f "$TEST_ROOT/certificate.p12" "$TEST_ROOT/keychain.password" "$KEYCHAIN"
    return "$status"
}
if [[ "${1:-}" == cleanup ]]; then cleanup; exit; fi
[[ "${MACOS_TEST_SIGNING_READY:-}" == 1 ]] || { echo 'Signing is not armed' >&2; exit 1; }
: "${MACOS_TEST_CERTIFICATE_P12_BASE64:?Missing certificate secret}"
: "${MACOS_TEST_CERTIFICATE_PASSWORD:?Missing password secret}"
[[ "$(uname -m)" == arm64 ]]
CERTIFICATE=50B8A2614631D0EE1705213D19D07A52C921D665
export CODESIGN_IDENTITY="$CERTIFICATE" MACOS_SIGNING_CERTIFICATE_SHA1="$CERTIFICATE"
export REQUIRE_STABLE_SIGNING=1 TARGET=aarch64-apple-darwin CODESIGN_KEYCHAIN="$KEYCHAIN"
for variant in A B; do
    binary="$TEST_ROOT/build-$variant/$TARGET/release/entropy"
    [[ -x "$binary" && "$(lipo -archs "$binary")" == arm64 ]]
done
! cmp -s "$TEST_ROOT/build-A/$TARGET/release/entropy" "$TEST_ROOT/build-B/$TARGET/release/entropy"
[[ ! -e "$KEYCHAIN" && ! -e "$TEST_ROOT/keychains.original" ]]
umask 077
security list-keychains -d user > "$TEST_ROOT/keychains.original"
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
# The decoded PKCS#12 and random temporary keychain password never enter artifacts/logs.
python3 - "$TEST_ROOT/certificate.p12" <<'PY'
import base64, os, pathlib, sys
pathlib.Path(sys.argv[1]).write_bytes(base64.b64decode(os.environ['MACOS_TEST_CERTIFICATE_P12_BASE64'], validate=True))
PY
openssl rand -hex 32 > "$TEST_ROOT/keychain.password"
KEYCHAIN_PASSWORD="$(<"$TEST_ROOT/keychain.password")"
security create-keychain -p "$KEYCHAIN_PASSWORD" "$KEYCHAIN" >/dev/null 2>&1
security set-keychain-settings -lut 21600 "$KEYCHAIN" >/dev/null 2>&1
security unlock-keychain -p "$KEYCHAIN_PASSWORD" "$KEYCHAIN" >/dev/null 2>&1
security import "$TEST_ROOT/certificate.p12" -P "$MACOS_TEST_CERTIFICATE_PASSWORD" \
    -T /usr/bin/codesign -f pkcs12 -k "$KEYCHAIN" >/dev/null 2>&1
security set-key-partition-list -S apple-tool:,apple:,codesign: -s \
    -k "$KEYCHAIN_PASSWORD" "$KEYCHAIN" >/dev/null 2>&1
rm -f "$TEST_ROOT/certificate.p12" "$TEST_ROOT/keychain.password"
unset MACOS_TEST_CERTIFICATE_P12_BASE64 MACOS_TEST_CERTIFICATE_PASSWORD KEYCHAIN_PASSWORD
# Do not use find-identity -v: it omits an untrusted self-signed identity.
# Pin by SHA-1 of the imported certificate, then codesign selects by hash in this keychain.
security find-certificate -a -Z "$KEYCHAIN" | grep -Fx "SHA-1 hash: $CERTIFICATE" >/dev/null

mkdir -p "$TEST_ROOT/artifacts"
for variant in A B; do
    export CARGO_TARGET_DIR="$TEST_ROOT/build-$variant"
    export MACOS_TEST_DIST_DIR="$TEST_ROOT/package-$variant"
    bash "$ROOT/scripts/build_macos_test_app.sh"
    app="$MACOS_TEST_DIST_DIR/Entropy.app"
    macos_verify_stable_signature "$app/Contents/MacOS/entropy" "$CERTIFICATE" com.ergohaven.entropy
    codesign -d -r- "$app" 2>&1 | sed -n '/^designated =>/p' > "$TEST_ROOT/requirement-$variant"
    codesign -d -r- "$app/Contents/MacOS/entropy" 2>&1 | sed -n '/^designated =>/p' > "$TEST_ROOT/executable-requirement-$variant"
    cmp "$TEST_ROOT/requirement-$variant" "$TEST_ROOT/executable-requirement-$variant"
    codesign -dvvv "$app" 2>&1 | sed -n 's/^CDHash=//p' > "$TEST_ROOT/cdhash-$variant"
    # Packaging is deliberately isolated per variant; B cannot remove A's bundle/DMG.
    dmgs=("$MACOS_TEST_DIST_DIR"/*.dmg)
    [[ ${#dmgs[@]} == 1 && -s "${dmgs[0]}" ]]
    hdiutil verify "${dmgs[0]}" >/dev/null
    cp "${dmgs[0]}" "$TEST_ROOT/artifacts/entropy-signing-test-$variant-arm64.dmg"
done
cmp "$TEST_ROOT/requirement-A" "$TEST_ROOT/requirement-B"
[[ -s "$TEST_ROOT/cdhash-A" && -s "$TEST_ROOT/cdhash-B" ]]
! cmp -s "$TEST_ROOT/cdhash-A" "$TEST_ROOT/cdhash-B"
{
    printf 'Certificate SHA1: %s\n' "$CERTIFICATE"
    printf 'Source commit: %s\n' "${GITHUB_SHA:-unknown}"
    cat "$TEST_ROOT/requirement-A"
    printf 'CDHash A: '; cat "$TEST_ROOT/cdhash-A"
    printf 'CDHash B: '; cat "$TEST_ROOT/cdhash-B"
} > "$TEST_ROOT/artifacts/signature-check.txt"
cp "$ROOT/docs/macos-stable-signing-test.ru.md" "$TEST_ROOT/artifacts/README.ru.md"
(cd "$TEST_ROOT/artifacts" && shasum -a 256 ./*.dmg README.ru.md signature-check.txt > SHA256SUMS)
cleanup
trap - EXIT
