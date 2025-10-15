#!/bin/sh

set -e

ADDR="$1"
RECIPIENT="$2"
AMT="$3"
SKIP="$4"
BLOCK_HEIGHT="$5"

WALLET="./target/release-native/nockchain-wallet"
SOCK=("--nockchain-socket" "miner4/miner.sock")

tr -d '\000' < notes-$ADDR.csv > notes-clean-$ADDR.csv
{ head -n 1 notes-clean-$ADDR.csv && tail -n +2 notes-clean-$ADDR.csv | sort -t',' -k4,4n; } | python -c 'import csv, json, sys; print(json.dumps([dict(r) for r in csv.DictReader(sys.stdin)]))' |
    jq 'map({ name_first, name_last, assets: .assets|tonumber, block_height: .block_height|tonumber, source_hash })' > notes-json-$ADDR.json

# exit

# exec "$WALLET" ${SOCK[*]} list-notes-by-pubkey -p "$ADDR" | tee txnotes.txt
#cat txnotes_$ADDR.txt | grep -a "\(Assets\|Block Height\|Signers\|Name\)" > txfiltered
# echo "[ $(cat txfiltered | sed 's/- Name: \(.*\)/{ "name": "\1", /g' | sed 's/\.//g' | sed 's/- Assets: \(.*\)/"assets": \1,/g' | sed 's/- Block Height: \(.*\)/"block height": \1,/g' | sed 's/- Signers: \(.*\)/"signers": "\1" },/g') ]" | sed 's/\(.*\), ]/\1 ]/g' > tx.json

jq --argjson threshold "$AMT" --argjson bh "$BLOCK_HEIGHT" --argjson skip "$SKIP" '
  (
    map(select(.["block_height"] <= $bh - 100))
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
' notes-json-$ADDR.json > txtosend.json

# python3 -c 'import json,random,sys; a=json.load(sys.stdin); random.shuffle(a); json.dump(a,sys.stdout)' <txtosend0.json>txtosend.json
NAMES="$(jq -r '[.[] | "[\(.name_first) \(.name_last)]"] | join(",")' txtosend.json)"
ASSETS="$(jq -r '[.[] | .assets] | join(",")' txtosend.json)"
RECIPIENTS="$(jq -r --arg str $RECIPIENT '([.[] | $str]) | join(",")' txtosend.json)"

echo "Notes to send: $(jq 'length' txtosend.json)"

# echo "$NAMES" "$ASSETS" "$RECIPIENTS"

RUST_LOG=error exec "$WALLET" create-tx --names "$NAMES" --recipients "$RECIPIENTS" --gifts "$ASSETS" --fee 0 # | grep -a "saving draft to"
