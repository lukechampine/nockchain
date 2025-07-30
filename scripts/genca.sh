#!/bin/sh

set -e

SDIR="crates/nbx-miner/tls"

openssl ecparam -genkey -name prime256v1 -out $SDIR/ca.key

openssl req -x509 -new -nodes \
  -key $SDIR/ca.key \
  -sha256 \
  -days 3650 \
  -subj "/C=US/ST=TX/L=Austin/O=NBX/CN=NBXCA" \
  -out $SDIR/ca.pem
