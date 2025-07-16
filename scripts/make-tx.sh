#!/bin/sh

set -e

ADDR="$1"
RECIPIENT="$2"
AMT="$3"
SKIP="$4"
BLOCK_HEIGHT="$5"

WALLET="./target/release/nockchain-wallet"
SOCK=("--nockchain-socket" "miner4/miner.sock")


# exec "$WALLET" ${SOCK[*]} list-notes-by-pubkey -p "$ADDR" | tee txnotes.txt
cat txnotes_$ADDR.txt | grep -a "\(assets\|block height\|signers\|name\)" > txfiltered
echo "[ $(cat txfiltered | sed 's/- name: \(.*\)/{ "name": "\1", /g' | sed 's/\.//g' | sed 's/- assets: 0i\(.*\)/"assets": \1,/g' | sed 's/- block height: 0i\(.*\)/"block height": \1,/g' | sed 's/- signers: \(.*\)/"signers": "\1" },/g') ]" | sed 's/\(.*\), ]/\1 ]/g' > tx.json

jq --argjson threshold "$AMT" --argjson bh "$BLOCK_HEIGHT" --argjson skip "$SKIP" '
  (
    map(select(.["block height"] <= $bh - 100))
    | .[$skip:]
    | reduce .[] as $i (
        {prev:0,sum:0,items:[]};
        if .sum<$threshold then
          {prev:.sum,sum:(.sum+$i.assets),items:(.items+[$i])}
        else
          .
        end
      )
  ) as $acc
  | $acc.items
  | if ($acc.sum>$threshold) and (length>0) then
      .[0:-1] + [ (.[-1] | .assets=($threshold-$acc.prev)) ]
    else
      .
    end
' tx.json > txtosend.json

# python3 -c 'import json,random,sys; a=json.load(sys.stdin); random.shuffle(a); json.dump(a,sys.stdout)' <txtosend0.json>txtosend.json
NAMES="$(jq -r '[.[] | .name] | join(",")' txtosend.json)"
ASSETS="$(jq -r '[.[] | .assets] | join(",")' txtosend.json)"
RECIPIENTS="$(jq -r --arg str $RECIPIENT '([.[] | $str]) | join(",")' txtosend.json)"

echo "Notes to send: $(jq 'length' txtosend.json)"

# echo "$NAMES" "$ASSETS" "$RECIPIENTS"

RUST_LOG=error exec "$WALLET" simple-spend --names "$NAMES" --recipients "$RECIPIENTS" --gifts "$ASSETS" --fee 0 # | grep -a "saving draft to"
