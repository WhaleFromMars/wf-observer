#!/usr/bin/env bash

set -euo pipefail

if (( $# != 5 )); then
    echo "usage: $0 VERSION SOURCE_SHA256 LINUX_SHA256 WINDOWS_SHA256 OUTPUT" >&2
    exit 2
fi

version=$1
source_sha256=$2
linux_sha256=$3
windows_sha256=$4
output=$5
repository_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)

if [[ ! "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo "invalid stable CLI version: $version" >&2
    exit 2
fi

for checksum in "$source_sha256" "$linux_sha256" "$windows_sha256"; do
    if [[ ! "$checksum" =~ ^[[:xdigit:]]{64}$ ]]; then
        echo "invalid SHA-256 checksum: $checksum" >&2
        exit 2
    fi
done

mkdir -p \
    "$output/aur/wf-observer" \
    "$output/aur/wf-observer-bin" \
    "$output/scoop"

render() {
    local source=$1
    local destination=$2

    sed \
        -e "s/@VERSION@/$version/g" \
        -e "s/@SOURCE_SHA256@/$source_sha256/g" \
        -e "s/@LINUX_SHA256@/$linux_sha256/g" \
        -e "s/@WINDOWS_SHA256@/$windows_sha256/g" \
        "$source" > "$destination"
}

render \
    "$repository_root/packaging/aur/wf-observer/PKGBUILD.in" \
    "$output/aur/wf-observer/PKGBUILD"
render \
    "$repository_root/packaging/aur/wf-observer-bin/PKGBUILD.in" \
    "$output/aur/wf-observer-bin/PKGBUILD"
render \
    "$repository_root/packaging/scoop/wf-observer.json.in" \
    "$output/scoop/wf-observer.json"

bash -n "$output/aur/wf-observer/PKGBUILD"
bash -n "$output/aur/wf-observer-bin/PKGBUILD"
jq --exit-status . "$output/scoop/wf-observer.json" >/dev/null

if grep --recursive --extended-regexp '@[A-Z0-9_]+@' "$output"; then
    echo "unresolved package template placeholder" >&2
    exit 1
fi
