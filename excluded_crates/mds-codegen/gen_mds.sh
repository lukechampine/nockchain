#!/bin/sh

DIR=$(git rev-parse --show-toplevel)
SDIR=$(dirname "$0")

cd "$SDIR"
cargo run > "$DIR/crates/nbx-tip5/src/tip5/mds_generated.rs"
