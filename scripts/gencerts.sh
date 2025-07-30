#!/bin/sh

set -e

bash scripts/genca.sh
bash scripts/genserver.sh
bash scripts/genclient.sh
