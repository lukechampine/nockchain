#!/bin/bash

set -e

WALLET="nockchain-wallet"
WALLET2="./target/release/nockchain-wallet"
SOCK=("--nockchain-socket" "nockchain_miner/nockchain.sock")
SSH="root@nockbox4203"

ssh -T "$SSH" "$WALLET ${SOCK[*]} --export-state-jam walstate.jam --color never list-pubkeys"
scp "$SSH:walstate.jam" ./walstate.jam
exec "$WALLET2" --state-jam ./walstate.jam list-pubkeys
