#!/bin/sh
# One mapping. A persistent session leader prevents process-group ID reuse during
# cleanup, even if the listener dies while connection workers remain alive.
set -eu
SHARE=/usr/local/share/dcc
STATE=/run/dcc
identity() {
    [ -r "/proc/$1/stat" ] || return 1
    record=$(cat "/proc/$1/stat") || return 1
    record=${record##*) }
    printf '%s\n' "$record" | awk '{print $20}'
}
if [ "${1-}" = --group ]; then
    dir=$2; proxy=$3; target=$4
    trap ':' TERM INT
    printf '%s\n' "$$" > "$dir/group.tmp"
    mv "$dir/group.tmp" "$dir/group"
    mkfifo "$dir/error.pipe"
    { head -c 16384 > "$dir/error"; cat > /dev/null; } < "$dir/error.pipe" &
    "$SHARE/dcc-relay" "$proxy" "$target" > /dev/null 2> "$dir/error.pipe" &
    listener=$!
    printf '%s\n' "$listener" > "$dir/listener.tmp"
    mv "$dir/listener.tmp" "$dir/listener"
    result=0
    wait "$listener" || result=$?
    printf '%s\n' "$result" > "$dir/exited.tmp"
    mv "$dir/exited.tmp" "$dir/exited"
    # Only the owner terminates this anchor, after signaling all group members.
    while :; do sleep 0.2 || :; done
fi
[ "$#" -eq 3 ] || exit 64
dir=$1; proxy=$2; target=$3
group=''; group_identity=''; child=''
status() {
    printf '%s %s %s %s\n' "$1" "$target" "$proxy" "$attempt" > "$dir/status.tmp"
    mv "$dir/status.tmp" "$dir/status"
}
owned_group() {
    [ -n "$group_identity" ] && [ -n "$group" ] && [ "$group" -gt 1 ] &&
        [ "$(identity "$group" 2>/dev/null || :)" = "$group_identity" ]
}
cleanup() {
    [ -n "$group" ] || return 0
    if owned_group; then
        kill -TERM "-$group" 2>/dev/null || :
        sleep 2
        # An unexpectedly lost identity means cleanup is uncertain. Do not
        # signal a potentially recycled group or start a replacement listener.
        owned_group || return 1
        kill -KILL "-$group" 2>/dev/null || return 1
    else
        return 1
    fi
    if [ -n "$child" ]; then wait "$child" 2>/dev/null || :; fi
    group=''; child=''
}
trap 'cleanup; exit 0' TERM INT
ready() {
    [ -f "$dir/listener" ] && [ ! -f "$dir/exited" ] || return 1
    listener=$(cat "$dir/listener")
    case "$listener" in ''|*[!0-9]*) return 1 ;; esac
    hex=$(printf '%04X' "$proxy")
    for fd in /proc/"$listener"/fd/*; do
        link=$(readlink "$fd" 2>/dev/null || :)
        case "$link" in socket:\[*\]) inode=${link#socket:[}; inode=${inode%]} ;; *) continue ;; esac
        if awk -v addr="00000000:$hex" -v inode="$inode" \
            '$2 == addr && $4 == "0A" && $10 == inode { found=1 } END { exit !found }' \
            "/proc/$listener/net/tcp" 2>/dev/null; then return 0; fi
    done
    return 1
}
attempt=0
while :; do
    if [ -f "$STATE/stopping" ] || [ -f "$STATE/relay/shutdown" ]; then break; fi
    generation="$dir/generation-$attempt"
    mkdir "$generation"
    service_dir=$dir
    dir=$generation
    setsid "$SHARE/dcc-relay-service" --group "$dir" "$proxy" "$target" &
    child=$!
    group=$child
    group_identity=$(identity "$group" 2>/dev/null || :)
    if [ -z "$group_identity" ]; then
        wait "$child" 2>/dev/null || :
        dir=$service_dir; status failed; exit 1
    fi
    bound=0
    n=0
    while [ "$n" -lt 25 ]; do
        [ ! -f "$STATE/relay/shutdown" ] || break
        [ ! -f "$dir/exited" ] || break
        if [ -f "$dir/group" ] && [ "$(cat "$dir/group")" != "$group" ]; then break; fi
        if ready; then bound=1; break; fi
        n=$((n + 1)); sleep 0.2
    done
    if [ "$bound" -eq 1 ]; then
        dir=$service_dir; status ready; dir=$generation
        while [ ! -f "$dir/exited" ] && owned_group && [ ! -f "$STATE/relay/shutdown" ]; do sleep 0.2; done
    fi
    if [ "$bound" -eq 1 ]; then dir=$service_dir; status recovering; dir=$generation; fi
    if ! cleanup; then dir=$service_dir; status degraded; exit 1; fi
    dir=$service_dir
    if [ -f "$STATE/stopping" ] || [ -f "$STATE/relay/shutdown" ]; then break; fi
    if [ "$attempt" -eq 0 ] && [ "$bound" -eq 0 ]; then status failed; break; fi
    if [ "$attempt" -ge 3 ]; then status degraded; break; fi
    attempt=$((attempt + 1))
    status recovering
    sleep 1
done
# Preserve the last failed/ready state for diagnostics; supervisor observes runner
# termination as degraded if no terminal failure status was written.
exit 0
