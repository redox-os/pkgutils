#!/bin/fish
redoxer build
redoxer exec --folder ./target/x86_64-unknown-redox/debug/pkg-cli /root/pkg-cli $argv
