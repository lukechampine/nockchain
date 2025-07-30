#!/bin/sh

set -e

SDIR="crates/nbx-miner/tls"

openssl ecparam -genkey -name prime256v1 -out $SDIR/client.key

openssl req -new \
  -key $SDIR/client.key \
  -sha256 \
  -subj "/C=US/ST=TX/L=Austin/O=NBX/CN=dummy-client" \
  -out $SDIR/client.csr

openssl x509 -req \
  -in $SDIR/client.csr \
  -CA $SDIR/ca.pem -CAkey $SDIR/ca.key -CAcreateserial \
  -sha256 \
  -days 825 \
  -out $SDIR/client.crt

cat $SDIR/client.crt $SDIR/ca.pem > $SDIR/client_chain.pem
