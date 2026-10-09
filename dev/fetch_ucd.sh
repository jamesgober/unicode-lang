#!/bin/sh
# Download pinned Unicode Character Database files into dev/ucd/ and verify
# each one against the SHA-256 recorded in dev/ucd.sha256.
#
#   sh dev/fetch_ucd.sh                         # every file the generator reads
#   sh dev/fetch_ucd.sh NormalizationTest.txt   # just the conformance suite
#
# dev/ucd/ is gitignored: the generated src/tables.rs is committed, the
# multi-megabyte source data is not. CI runs this for NormalizationTest.txt so
# tests/conformance.rs always exercises the full official suite. A file whose
# hash does not match is deleted and the script fails; nothing unverified is
# left in dev/ucd/.
#
# The version is pinned here and must match UNICODE_VERSION in src/tables.rs
# (tests/conformance.rs checks the NormalizationTest.txt header against it).
set -eu

VERSION="16.0.0"
BASE="https://www.unicode.org/Public/${VERSION}/ucd"

cd "$(dirname "$0")/.."
SUMS="dev/ucd.sha256"

if command -v sha256sum >/dev/null 2>&1; then
    sha() { sha256sum "$1" | awk '{print $1}'; }
elif command -v shasum >/dev/null 2>&1; then
    sha() { shasum -a 256 "$1" | awk '{print $1}'; }
else
    echo "fetch_ucd: need sha256sum or shasum" >&2
    exit 1
fi

if [ "$#" -eq 0 ]; then
    set -- $(awk '{ sub(".*/", "", $2); print $2 }' "$SUMS")
fi

mkdir -p dev/ucd
for name in "$@"; do
    expected=$(awk -v p="dev/ucd/${name}" '$2 == p { print $1 }' "$SUMS")
    if [ -z "$expected" ]; then
        echo "fetch_ucd: no pinned SHA-256 for ${name} in ${SUMS}" >&2
        exit 1
    fi
    part="dev/ucd/${name}.part"
    curl -fsSL --retry 5 --retry-delay 3 -o "$part" "${BASE}/${name}"
    actual=$(sha "$part")
    if [ "$actual" != "$expected" ]; then
        rm -f "$part"
        echo "fetch_ucd: SHA-256 mismatch for ${name}: expected ${expected}, got ${actual}" >&2
        exit 1
    fi
    mv -f "$part" "dev/ucd/${name}"
    echo "fetch_ucd: ${name} (UCD ${VERSION}) verified"
done
