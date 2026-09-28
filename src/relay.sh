#!/bin/sh
# Fixed TCP proxy interface; never interpret project input as socat options.
set -eu
fail() { printf '%s\n' "dcc-relay: $*" >&2; exit 1; }
if [ "${1-}" = --check ]; then
    for tool in socat setsid readlink awk; do
        command -v "$tool" >/dev/null 2>&1 || fail "missing $tool; rebuild the image"
    done
    help=$(socat -hh 2>&1) || fail 'cannot inspect socat capabilities'
    for capability in TCP4-LISTEN TCP4-CONNECT fork; do
        printf '%s\n' "$help" | grep -qi -- "$capability" || fail "socat lacks $capability"
    done
    printf '%s\n' "$help" | grep -q -- '^[[:space:]]*-t[<[:space:]]' || fail 'socat lacks -t'
    exit 0
fi
[ "$#" -eq 2 ] || fail 'expected PROXY TARGET'
for port in "$@"; do
    case "$port" in ''|*[!0-9]*) fail 'invalid port' ;; esac
    [ "$port" -ge 1 ] && [ "$port" -le 65535 ] || fail 'port out of range'
done
proxy=$(command -v socat) || fail 'socat unavailable; rebuild the image'
exec "$proxy" -t 2 "TCP4-LISTEN:$1,bind=0.0.0.0,reuseaddr,fork" "TCP4:127.0.0.1:$2"
