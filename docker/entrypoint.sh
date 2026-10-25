#!/bin/sh
set -eu
umask 077
data_dir="${YAMANAMI_DATA_DIR:-/var/lib/yamanami}"
binary="${YAMANAMI_BIN:-yamanami}"
endpoint="${YAMANAMI_ADMIN_ENDPOINT:-http://127.0.0.1:9090}"
apps_dir="${YAMANAMI_APPS_DIR:-/opt/yamanami/apps}"
token_file="$data_dir/admin-token"
ready_file="$data_dir/demo-ready"
load_token() {
    if [ -z "${YAMANAMI_TOKEN:-}" ]; then
        if [ ! -s "$token_file" ]; then
            mkdir -p "$data_dir"
            od -An -N32 -tx1 /dev/urandom | tr -d ' \n' > "$token_file"
        fi
        YAMANAMI_TOKEN="$(cat "$token_file")"
    fi
    export YAMANAMI_TOKEN
}
ctl() { "$binary" ctl --endpoint "$endpoint" "$@"; }
case "${1:-}" in
    health)
        [ -s "$token_file" ] || [ -n "${YAMANAMI_TOKEN:-}" ] || exit 1
        load_token
        if [ "${YAMANAMI_DEPLOY_DEMO:-0}" = 1 ]; then [ -f "$ready_file" ] || exit 1; fi
        ctl list >/dev/null
        exit 0 ;;
    ctl)
        load_token
        shift
        exec "$binary" ctl --endpoint "$endpoint" "$@" ;;
esac
load_token
if [ "${1:-}" != serve ] || [ "${YAMANAMI_DEPLOY_DEMO:-0}" != 1 ]; then
    exec "$binary" "$@"
fi
rm -f "$ready_file"
"$binary" "$@" &
server_pid=$!
cleanup() {
    trap - EXIT INT TERM
    kill -INT "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
}
trap cleanup EXIT
trap 'exit 143' TERM
trap 'exit 130' INT
attempt=0
until ctl list >/dev/null 2>&1; do
    if ! kill -0 "$server_pid" 2>/dev/null; then
        wait "$server_pid"
        exit 1
    fi
    attempt=$((attempt + 1))
    if [ "$attempt" -ge 600 ]; then
        echo "Yamanami startup did not finish within 600 seconds" >&2
        exit 1
    fi
    sleep 1
done
deploy_if_changed() {
    app="$1"
    path="$2"
    wasm="$apps_dir/$app.wasm"
    expected="$(sha256sum "$wasm" | cut -d ' ' -f 1)"
    info="$(ctl info "$app" 2>/dev/null || true)"
    if printf '%s' "$info" | grep -Fq "\"sha256\": \"$expected\"" &&
       printf '%s' "$info" | grep -Fq "\"path\": \"$path\""; then
        echo "Unchanged: $app"
    else
        echo "Deploying: $app"
        ctl deploy "$wasm" --app "$app" --path "$path"
    fi
}
deploy_if_changed index /
deploy_if_changed calculator-rust /rust
deploy_if_changed calculator-cpp /cpp
deploy_if_changed calculator-python /python
deploy_if_changed calculator-typescript /typescript
touch "$ready_file"
echo "Yamanami demo is ready"
set +e
wait "$server_pid"
status=$?
set -e
exit "$status"
