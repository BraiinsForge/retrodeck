#!/usr/bin/env bash
set -euo pipefail

label=$1
shift
: "${UPLOADER_SOURCE:?}"
: "${UPLOADER_SERVER:?}"
: "${UPLOADER_ASSET_ROOT:?}"
: "${UPLOADER_PALETTE:?}"
UPLOADER_SHELL=${UPLOADER_SHELL:-$BASH}
work=$PWD/uploader-http-$label
rm -rf "$work"
mkdir -p "$work/data/nes-deck/menu" "$work/data/nes-deck/uploader" \
  "$work/assets" "$work/home"
work=$(realpath "$work")
server=

cleanup() {
  if [[ -n ${server:-} ]]; then
    kill "$server" 2>/dev/null || true
    wait "$server" 2>/dev/null || true
  fi
  rm -rf "$work"
}
trap cleanup EXIT

die() {
  printf 'uploader-http-%s: %s\n' "$label" "$*" >&2
  [[ ! -f $work/response.headers.clean ]] || cat "$work/response.headers.clean" >&2
  if [[ -f $work/response.body ]]; then
    head -c 4096 "$work/response.body" >&2
    printf '\n' >&2
  fi
  [[ ! -f $work/server.log ]] || cat "$work/server.log" >&2
  exit 1
}

request() {
  local expected=$1 status
  shift
  : > "$work/response.headers"
  : > "$work/response.body"
  status=$(curl --silent --show-error --max-time 15 --http1.1 \
    -D "$work/response.headers" -o "$work/response.body" \
    -w '%{http_code}' "$@") || die "curl failed: $*"
  tr -d '\r' < "$work/response.headers" > "$work/response.headers.clean"
  [[ $status == "$expected" ]] || \
    die "expected HTTP $expected, got $status for $*"
}

: > "$work/data/nes-deck/menu/games.tsv"
cp "$UPLOADER_PALETTE" "$work/data/nes-deck/menu/palette.tsv"
cp "$UPLOADER_ASSET_ROOT/uploader-paper.css" "$work/assets/"
cp "$UPLOADER_ASSET_ROOT/uploader-palette.js" "$work/assets/"
printf 'fixture\n' > "$work/data/nes-deck/uploader/password.conf"
chmod 600 "$work/data/nes-deck/uploader/password.conf"
printf '\002\000\252\252' > "$work/raw-game.tap"
printf '\002\000\125\125' > "$work/zip-game.tap"
mkdir "$work/zip-source"
cp "$work/zip-game.tap" "$work/zip-source/zip-game.tap"
(cd "$work/zip-source" && zip -q "$work/one.zip" zip-game.tap)

printf '#!%s\n' "$UPLOADER_SHELL" > "$work/password-helper"
cat >> "$work/password-helper" <<'EOF'
set -eu
test "${1:-}" = --verify-uploader-password
test -f "${2:-}"
password=$(cat)
printf '%s\n' "$password" >> "$UPLOADER_HELPER_LOG"
test "$password" = configured-test-password
EOF
printf '#!%s\n' "$UPLOADER_SHELL" > "$work/restart-helper"
cat >> "$work/restart-helper" <<'EOF'
set -eu
printf 'restart\n' >> "$UPLOADER_RESTART_LOG"
EOF
chmod +x "$work/password-helper" "$work/restart-helper"
: > "$work/helper.log"
: > "$work/restart.log"

HOME="$work/home" \
XDG_CACHE_HOME="$work/home/cache" \
UPLOADER_DATA_ROOT="$work/data/" \
UPLOADER_ASSET_ROOT="$work/assets/" \
UPLOADER_NATIVE="$work/password-helper" \
UPLOADER_RESTART="$work/restart-helper" \
UPLOADER_HELPER_LOG="$work/helper.log" \
UPLOADER_RESTART_LOG="$work/restart.log" \
UPLOADER_PORT_FILE="$work/port" \
UPLOADER_STOP="$work/stop" \
"$@" "$UPLOADER_SERVER" > "$work/server.log" 2>&1 &
server=$!

for ((attempt=0; attempt<240; attempt++)); do
  [[ -s $work/port ]] && break
  kill -0 "$server" 2>/dev/null || break
  sleep 0.25
done
[[ -s $work/port ]] || die "server did not become ready"
port=$(cat "$work/port")
origin=http://127.0.0.1:$port

request 421 -H "Host: deck.local:$port" "$origin/"
grep -q "Use one of this Deck's IPv4 addresses on port 8080." "$work/response.body"
request 200 -H "Host: 192.168.1.20:$port" "$origin/"
request 200 "$origin/"
grep -q '<h1>ROM uploader</h1>' "$work/response.body"
grep -q '<form action="/login" method="post">' "$work/response.body"
for header in Cache-Control Content-Security-Policy Cross-Origin-Opener-Policy \
  Cross-Origin-Resource-Policy Referrer-Policy X-Content-Type-Options X-Frame-Options; do
  grep -qi "^$header:" "$work/response.headers.clean" || die "missing $header"
done

request 200 "$origin/assets/paper.css"
cmp "$work/response.body" "$work/assets/uploader-paper.css"
request 200 "$origin/assets/palette.js"
cmp "$work/response.body" "$work/assets/uploader-palette.js"
request 404 "$origin/assets/settings-icons/09.png"
printf '404 page not found\n' > "$work/not-found"
cmp "$work/response.body" "$work/not-found"
request 405 -X POST "$origin/"
grep -qi '^Allow: GET$' "$work/response.headers.clean"
request 405 "$origin/login"
grep -qi '^Allow: POST$' "$work/response.headers.clean"
request 403 "$origin/logout"
request 403 "$origin/upload"
request 403 "$origin/palette"
request 405 -X POST "$origin/assets/paper.css"

request 415 -H "Origin: $origin" -H 'Content-Type: text/plain' \
  --data-binary 'password=configured-test-password' "$origin/login"
request 400 -H "Origin: $origin" \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  -H 'Transfer-Encoding: chunked' \
  --data-binary 'password=configured-test-password' "$origin/login"
grep -q 'The login form was malformed.' "$work/response.body"
request 403 -H 'Origin: http://example.com' \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  --data 'password=configured-test-password' "$origin/login"
request 401 -H "Origin: $origin" \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  --data 'password=wrong-password' "$origin/login"
grep -q 'That password was not accepted.' "$work/response.body"

request 303 -c "$work/cookies" -H "Origin: $origin" \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  --data 'password=configured-test-password' "$origin/login"
cookie_header=$(grep -i '^Set-Cookie: deck_rom_session=' "$work/response.headers.clean")
[[ $cookie_header == *HttpOnly* && $cookie_header == *SameSite=Strict* && \
   $cookie_header == *Max-Age=28800* && $cookie_header == *Path=/* ]] || \
  die "session cookie attributes diverged"
cookie=$(awk '$6 == "deck_rom_session" { print $7 }' "$work/cookies")
[[ ${#cookie} -eq 43 && $cookie =~ ^[A-Za-z0-9_-]{43}$ ]] || \
  die "session cookie token diverged"

request 200 -b "$work/cookies" "$origin/"
grep -q '<form action="/upload"' "$work/response.body"
csrf=$(sed -n 's/.*name="csrf" value="\([^"]*\)".*/\1/p' "$work/response.body" | head -n 1)
[[ ${#csrf} -eq 43 && $csrf =~ ^[A-Za-z0-9_-]{43}$ ]] || \
  die "CSRF token diverged"
request 403 -b "$work/cookies" -H "Origin: $origin" \
  --data 'csrf=wrong' "$origin/logout"
request 403 -b "$work/cookies" -H "Origin: $origin" \
  --data 'background=%23123456' "$origin/palette"
request 403 -b "$work/cookies" -H "Origin: $origin" \
  -F csrf=wrong -F system=zx -F 'title=Rejected Game' \
  -F "rom=@$work/raw-game.tap;filename=rejected.tap" "$origin/upload"

request 200 -b "$work/cookies" -H "Origin: $origin" \
  -F "csrf=$csrf" -F system=zx -F 'title=Raw Game' \
  -F "rom=@$work/raw-game.tap;filename=raw-game.tap" "$origin/upload"
grep -q 'Raw Game was validated, filed, and added to the dashboard.' \
  "$work/response.body"
cmp "$work/raw-game.tap" "$work/data/roms/zx/raw-game.tap"
[[ $(stat -c '%a' "$work/data/roms/zx/raw-game.tap") == 600 ]]

request 200 -b "$work/cookies" -H "Origin: $origin" \
  -F "csrf=$csrf" -F system=zx -F 'title=Zip Game' \
  -F "rom=@$work/one.zip;filename=one.zip" "$origin/upload"
grep -q 'Zip Game was validated, filed, and added to the dashboard.' \
  "$work/response.body"
cmp "$work/zip-game.tap" "$work/data/roms/zx/zip-game.tap"
printf 'upload-zx-raw-game\tRaw Game\tzx\t%s\t#AF87D7\n' \
  "$work/data/roms/zx/raw-game.tap" > "$work/expected-games.tsv"
printf 'upload-zx-zip-game\tZip Game\tzx\t%s\t#AF87D7\n' \
  "$work/data/roms/zx/zip-game.tap" >> "$work/expected-games.tsv"
cmp "$work/expected-games.tsv" "$work/data/nes-deck/uploads/games.tsv"
[[ $(stat -c '%a' "$work/data/nes-deck/uploads/games.tsv") == 600 ]]

printf '\002\000\063\063' > "$work/replacement.tap"
request 422 -b "$work/cookies" -H "Origin: $origin" \
  -F "csrf=$csrf" -F system=zx -F 'title=Raw Game' \
  -F "rom=@$work/replacement.tap;filename=raw-game.tap" "$origin/upload"
grep -q 'a game with this title is already cataloged' "$work/response.body"
cmp "$work/raw-game.tap" "$work/data/roms/zx/raw-game.tap"
cmp "$work/expected-games.tsv" "$work/data/nes-deck/uploads/games.tsv"

palette=(background text-dark field surface inactive-border control-border footer \
  inactive-text text white title volume-off volume-on selected wifi-active \
  wifi-focus wifi-active-border field-label accent active control-surface muted)
palette_args=(--data-urlencode "csrf=$csrf")
for name in "${palette[@]}"; do
  palette_args+=(--data-urlencode "$name=#123ABC")
done
request 200 -b "$work/cookies" -H "Origin: $origin" \
  "${palette_args[@]}" "$origin/palette"
grep -q 'Dashboard appearance was saved and applied.' "$work/response.body"
override="$work/data/nes-deck/state/dashboard-palette.sexp"
grep -q '^(:version 2$' "$override"
[[ $(grep -o '#123ABC' "$override" | wc -l) -eq 22 ]]
[[ $(stat -c '%a' "$override") == 600 ]]
[[ $(wc -l < "$work/restart.log") -eq 3 ]]

request 303 -b "$work/cookies" -H "Origin: $origin" \
  --data-urlencode "csrf=$csrf" "$origin/logout"
grep -qi '^Set-Cookie: deck_rom_session=;' "$work/response.headers.clean"
request 200 -b "$work/cookies" "$origin/"
grep -q '<form action="/login" method="post">' "$work/response.body"
! grep -q '<form action="/upload"' "$work/response.body"

for attempt in 1 2 3 4 5; do
  request 401 -H "Origin: $origin" \
    -H 'Content-Type: application/x-www-form-urlencoded' \
    --data 'password=wrong-password' "$origin/login"
done
request 429 -H "Origin: $origin" \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  --data 'password=configured-test-password' "$origin/login"
grep -q 'Too many attempts. Wait five minutes before trying again.' \
  "$work/response.body"
grep -qi '^Retry-After: 300$' "$work/response.headers.clean"
[[ $(wc -l < "$work/helper.log") -eq 7 ]]

: > "$work/stop"
timeout 20 tail --pid="$server" -f /dev/null || die "server did not stop"
wait "$server" || die "server exited unsuccessfully"
server=
grep -q '^uploader-http-smoke: STOPPED$' "$work/server.log"
printf 'uploader-http-%s: OK\n' "$label"
