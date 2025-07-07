#!/bin/bash

set -e

WALLET="./target/release/nockchain-wallet"
SOCK=("--nockchain-socket" "miner4/miner.sock")
ADDR="$1"

exec "$WALLET" ${SOCK[*]} list-notes-by-pubkey -p "$ADDR" | tee txnotes_$ADDR.txt
