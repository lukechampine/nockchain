#!/bin/bash

set -e

WALLET="nockchain-wallet"
SOCK=("--nockchain-socket" "nockchain_miner/nockchain.sock")
SSH="root@nockbox4203"
TX="$1"

ssh -T "$SSH" "mkdir -p script-tx"
scp "txs/$TX.tx" "$SSH:script-tx/$TX.tx"
ssh -T "$SSH" "$WALLET ${SOCK[*]} send-tx script-tx/$TX.tx"
