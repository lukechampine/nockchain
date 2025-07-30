#!/bin/sh

set -e

SDIR="crates/nbx-miner/tls"

openssl ecparam -genkey -name prime256v1 -out $SDIR/server.key

openssl req -new \
  -key $SDIR/server.key \
  -sha256 \
  -subj "/C=US/ST=TX/L=Austin/O=NBX/CN=0.0.0.0" \
  -addext "subjectAltName = IP:0.0.0.0" \
  -out $SDIR/server.csr

openssl x509 -req \
  -in $SDIR/server.csr \
  -CA $SDIR/ca.pem -CAkey $SDIR/ca.key -CAcreateserial \
  -sha256 \
  -days 825 \
  -extfile <(printf "subjectAltName=IP:0.0.0.0") \
  -out $SDIR/server.crt

cat $SDIR/server.crt $SDIR/ca.pem > $SDIR/server_chain.pem
