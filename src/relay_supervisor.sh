# Sourced into PID 1's generated script, with STATE and DCC_SHARE already fixed.
relay_pids=''
relay_start() {
    if [ ! -s "__RT_MOUNT__/relay-ports" ]; then return; fi
    (umask 077; mkdir "$STATE/relay")
    while read -r proxy target; do
        case "$proxy:$target" in *[!0-9:]*|:|*: ) return 1 ;; esac
        dir="$STATE/relay/$proxy"
        mkdir "$dir"
        printf 'starting %s %s 0\n' "$target" "$proxy" > "$dir/status"
        "__DCC_SHARE__/dcc-relay-service" "$dir" "$proxy" "$target" &
        runner=$!
        printf '%s\n' "$runner" > "$dir/runner"
        relay_pids="$relay_pids $runner"
    done < "__RT_MOUNT__/relay-ports"
}
relay_wait_initial() {
    for dir in "$STATE"/relay/[0-9]*; do
        [ -d "$dir" ] || continue
        n=0
        while :; do
            read -r state rest < "$dir/status"
            case "$state" in ready|failed|degraded) break ;; esac
            runner=$(cat "$dir/runner")
            if ! kill -0 "$runner" 2>/dev/null || [ "$n" -ge 50 ]; then
                printf 'failed 0 %s 0\n' "${dir##*/}" > "$dir/status"
                break
            fi
            n=$((n + 1)); sleep 0.2
        done
        read -r state rest < "$dir/status"
        if [ "$state" != ready ]; then : > "$STATE/relay-start-failed"; fi
    done
}
relay_stop() {
    [ -d "$STATE/relay" ] || return 0
    : > "$STATE/relay/shutdown"
    for runner in $relay_pids; do wait "$runner" 2>/dev/null || :; done
}
