#!/bin/bash

set -e

WALLET="nockchain-wallet"
SOCK=("--nockchain-socket" "nockchain_miner/nockchain.sock")
SSH="root@nockbox4203"
ADDR="$1"

ssh -T "$SSH" "$WALLET ${SOCK[*]} --color never list-notes-by-pubkey $ADDR" | awk '/^- Name:/{printf "%s", $0; getline; print $0; next}1' | tee txnotes_$ADDR.txt
