#!/bin/bash
set -e

AUTH_KEYS=/home/postlab/.ssh/authorized_keys

if [ -f /tmp/authorized_keys ]; then
    cp /tmp/authorized_keys "$AUTH_KEYS"
    chown postlab:postlab "$AUTH_KEYS"
    chmod 600 "$AUTH_KEYS"
else
    echo "WARNING: /tmp/authorized_keys not found — key auth will not work." >&2
    echo "Run: make docker-keygen  then  make docker-up" >&2
fi

exec /usr/sbin/sshd -D -e
