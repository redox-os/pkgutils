#!/usr/bin/env bash

build=release
create_flag=
remote=0

for arg in "$@"; do
    case $arg in
        -d) build=debug ;;
        -c) create_flag=-c ;;
        -r) remote=1 ;;
    esac
done

set -ex

rm -rf target/test
mkdir -p target/test

if [[ "$build" == debug ]]; then
    cargo build --all-features
else
    cargo build --release --all-features
fi

if [[ "$remote" -eq 1 ]]; then
    PKEY="https://static.redox-os.org/pkg/id_ed25519.pub.toml"
    ARCHIVE="https://static.redox-os.org/pkg/x86_64-unknown-redox/shared-mime-info.pkgar"
    EXTRACT_DIR="target/test/remote_src"
    SPLIT_DATA=""
    mkdir -p "$EXTRACT_DIR"
else
    PKEY="target/test/public.toml"
    SKEY="target/test/secret.toml"
    ARCHIVE="target/test/src.pkgar"
    EXTRACT_DIR="target/test/src"
    SPLIT_DATA="target/test/src.pkgar_data"

    time target/$build/pkgar-keys gen \
        --skey "$SKEY" \
        --pkey "$PKEY" \
        --plaintext

    time target/$build/pkgar \
        create $create_flag \
        --skey "$SKEY" \
        --archive "$ARCHIVE" \
        pkgar/src

    stat -c %s "$ARCHIVE"
fi

time target/$build/pkgar \
    list \
    --pkey "$PKEY" \
    --archive "$ARCHIVE"

time target/$build/pkgar \
    split \
    --pkey "$PKEY" \
    --archive "$ARCHIVE" \
    target/test/src.pkgar_head \
    $SPLIT_DATA

stat -c %s target/test/src.pkgar_head $SPLIT_DATA

time target/$build/pkgar \
    list \
    --pkey "$PKEY" \
    --archive target/test/src.pkgar_head

time target/$build/pkgar \
    extract \
    --pkey "$PKEY" \
    --archive "$ARCHIVE" \
    "$EXTRACT_DIR"

if [[ "$remote" -eq 0 ]]; then
    diff -ruwN pkgar/src "$EXTRACT_DIR"
fi

time target/$build/pkgar \
    replace \
    --pkey "$PKEY" \
    --old-archive "$ARCHIVE" \
    --archive "$ARCHIVE" \
    "$EXTRACT_DIR"

if [[ "$remote" -eq 0 ]]; then
    diff -ruwN pkgar/src "$EXTRACT_DIR"
fi

time target/$build/pkgar \
    verify \
    --pkey "$PKEY" \
    --archive "$ARCHIVE" \
    "$EXTRACT_DIR"

time target/$build/pkgar \
    verify \
    --pkey "$PKEY" \
    --archive target/test/src.pkgar_head \
    "$EXTRACT_DIR"

time target/$build/pkgar \
    remove \
    --pkey "$PKEY" \
    --archive "$ARCHIVE" \
    "$EXTRACT_DIR"

if [[ "$(find "$EXTRACT_DIR" '!' -type d)" ]]; then
    exit 1
fi
