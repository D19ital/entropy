#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT/scripts/macos_stable_signing.sh"

fail() {
	echo "FAIL: $*" >&2
	exit 1
}

expect_success() {
	local description="$1"
	shift
	"$@" || fail "$description"
}

expect_failure() {
	local description="$1"
	shift
	if "$@"; then
		fail "$description"
	fi
}

adhoc_details=$'Signature=adhoc\nTeamIdentifier=not set'
adhoc_requirement='designated => cdhash H"d8b56c9110cf6f078cd45f0c4d19876a3ff9b288"'
certificate_sha1='0123456789abcdef0123456789abcdef01234567'
bundle_id='com.ergohaven.entropy'
self_signed_details=$'Authority=Entropy Open Source Release Signing\nTeamIdentifier=not set'
self_signed_requirement="designated => certificate root = H\"$certificate_sha1\" and identifier \"$bundle_id\""
generated_requirement="$(macos_stable_designated_requirement "$certificate_sha1" "$bundle_id")"
expected_generated_requirement="=designated => anchor = H\"$certificate_sha1\" and identifier \"$bundle_id\""

if [[ "$generated_requirement" != "$expected_generated_requirement" ]]; then
	fail "stable designated requirement generator returned an unexpected requirement"
fi

expect_failure \
	"required stable signing accepted an ad-hoc identity" \
	macos_validate_stable_signing_configuration "-" "1" "" "$bundle_id"
expect_success \
	"local ad-hoc packaging should remain available" \
	macos_validate_stable_signing_configuration "-" "0" "" "$bundle_id"
expect_failure \
	"stable signing accepted a missing certificate hash" \
	macos_validate_stable_signing_configuration \
	"Entropy Open Source Release Signing" \
	"1" \
	"" \
	"$bundle_id"
expect_failure \
	"stable signing accepted an invalid certificate hash" \
	macos_validate_stable_signing_configuration \
	"Entropy Open Source Release Signing" \
	"1" \
	"invalid" \
	"$bundle_id"
expect_success \
	"valid stable signing configuration failed" \
	macos_validate_stable_signing_configuration \
	"Entropy Open Source Release Signing" \
	"1" \
	"$certificate_sha1" \
	"$bundle_id"
expect_failure \
	"ad-hoc signature passed stable validation" \
	macos_validate_stable_signature_output \
	"$adhoc_details" \
	"$adhoc_requirement" \
	"$certificate_sha1" \
	"$bundle_id"
expect_failure \
	"hash-only designated requirement passed stable validation" \
	macos_validate_stable_signature_output \
	"$self_signed_details" \
	"$adhoc_requirement" \
	"$certificate_sha1" \
	"$bundle_id"
expect_failure \
	"wrong certificate passed stable validation" \
	macos_validate_stable_signature_output \
	"$self_signed_details" \
	"$self_signed_requirement" \
	"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" \
	"$bundle_id"
expect_success \
	"self-signed certificate requirement failed stable validation" \
	macos_validate_stable_signature_output \
	"$self_signed_details" \
	"$self_signed_requirement" \
	"$certificate_sha1" \
	"$bundle_id"

expect_failure \
	"extra clauses passed stable requirement validation" \
	macos_validate_stable_signature_output \
	"$self_signed_details" \
	"$self_signed_requirement or identifier \"other.app\"" \
	"$certificate_sha1" \
	"$bundle_id"

mock_codesign_details="$adhoc_details"
mock_codesign_requirement="$adhoc_requirement"
codesign() {
	case "$1" in
	--verify) return 0 ;;
	-dv) printf '%s\n' "$mock_codesign_details" >&2 ;;
	-d) printf '%s\n' "$mock_codesign_requirement" >&2 ;;
	*) fail "unexpected mock codesign arguments: $*" ;;
	esac
}

expect_failure \
	"app verification swallowed ad-hoc signature rejection" \
	macos_verify_stable_signature \
	"/mock/Entropy.app" \
	"$certificate_sha1" \
	"$bundle_id"
mock_codesign_details="$self_signed_details"
mock_codesign_requirement="$self_signed_requirement"
expect_success \
	"app verification rejected a stable self-signed signature" \
	macos_verify_stable_signature \
	"/mock/Entropy.app" \
	"$certificate_sha1" \
	"$bundle_id"

# Security is unavailable on Linux: assert exact argv, including paths with spaces.
mock_root="$(mktemp -d "${TMPDIR:-/tmp}/entropy-keychain-mock.XXXXXX")"
trap 'rm -rf "$mock_root"' EXIT
printf '    "/Users/test/login.keychain-db"\n    "/path with spaces/custom.keychain-db"\n' > "$mock_root/original"
security() {
    [[ "$*" != *default-keychain* && "$*" != *trusted-cert* ]] || fail 'trust/default keychain mutation'
    printf '<%s>' "$@" >> "$mock_root/security.log"
    printf '\n' >> "$mock_root/security.log"
}
macos_set_test_keychain_search_list "$mock_root/original" "$mock_root/temp signing.keychain-db"
macos_set_test_keychain_search_list "$mock_root/original"
expected="<list-keychains><-d><user><-s></Users/test/login.keychain-db></path with spaces/custom.keychain-db><$mock_root/temp signing.keychain-db>"$'\n''<list-keychains><-d><user><-s></Users/test/login.keychain-db></path with spaces/custom.keychain-db>'
[[ "$(<"$mock_root/security.log")" == "$expected" ]] || fail 'registration/restoration changed original paths'

# The probe must try the SAME explicit-keychain command on both sides of registration.
codesign() {
    printf '<%s>' "$@" >> "$mock_root/codesign.log"
    printf '\n' >> "$mock_root/codesign.log"
    case "$1" in
        --force)
            if [[ ! -f "$mock_root/registered" ]]; then
                echo 'The specified item could not be found in the keychain.' >&2
                return 1
            fi ;;
        --verify)
            [[ "$*" == *--strict* ]] || return 9
            [[ "$*" == *--deep* || "$*" == *'-R='* ]] || return 9 ;;
        -dv) printf '%s\n' "$self_signed_details" >&2 ;;
        -d) printf '%s\n' "designated => certificate root = H\"$certificate_sha1\" and identifier \"com.ergohaven.signing-diagnostic\"" >&2 ;;
        *) return 8 ;;
    esac
}
security() {
    [[ "$1" == list-keychains && "$2" == -d && "$3" == user && "$4" == -s ]] || return 7
    [[ "${5:-}" == /Users/test/login.keychain-db && "${6:-}" == '/path with spaces/custom.keychain-db' ]] || return 6
    if [[ $# == 6 ]]; then
        rm -f "$mock_root/registered"
    else
        [[ $# == 7 && "$7" == "$mock_root/temp signing.keychain-db" ]] || return 5
        touch "$mock_root/registered"
    fi
}
# Simulate create-keychain having automatically registered it: probe must reset first.
touch "$mock_root/registered"
macos_probe_test_keychain "$mock_root/diagnostic" "$certificate_sha1" \
    "$mock_root/temp signing.keychain-db" "$mock_root/original" > "$mock_root/probe.log"
[[ "$(<"$mock_root/probe.log")" == *'before registration: sign status=1; The specified item could not be found in the keychain.'* ]] || fail 'missing before error evidence'
[[ "$(<"$mock_root/probe.log")" == *'after registration: sign status=0'* ]] || fail 'missing after evidence'
[[ "$(<"$mock_root/probe.log")" == *'after registration: strict requirement verification=passed'* ]] || fail 'missing strict verification'
sign_calls=()
while IFS= read -r line; do
    [[ "$line" != '<--force>'* ]] || sign_calls+=("$line")
done < "$mock_root/codesign.log"
[[ ${#sign_calls[@]} == 2 && "${sign_calls[0]}" == "${sign_calls[1]}" && "${sign_calls[0]}" == *'<--keychain>'* ]] || fail 'probe did not retry identical explicit-keychain signing'

# A failed retry must stay a job failure, not be hidden by diagnostic printing.
rm -f "$mock_root/registered"
security() { return 0; }
expect_failure 'probe swallowed retry failure' macos_probe_test_keychain \
    "$mock_root/diagnostic" "$certificate_sha1" "$mock_root/temp signing.keychain-db" "$mock_root/original"

echo "macOS stable signing unit tests passed (MOCK codesign/security; no real macOS signing)"
