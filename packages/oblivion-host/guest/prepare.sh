#!/bin/sh
set -eu
umask 077
mountpoint -q /vault
mkdir -p /vault/home /vault/workspace /vault/state /vault/config /tmp/oblivion-cache
chown 1000:1000 /vault/home /vault/workspace /vault/state /vault/config /tmp/oblivion-cache
chmod 0700 /vault/home /vault/workspace /vault/state /vault/config /tmp/oblivion-cache
