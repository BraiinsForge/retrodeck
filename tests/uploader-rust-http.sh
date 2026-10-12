#!/usr/bin/env bash
set -euo pipefail
root=$(cd "$(dirname "$0")/.." && pwd)
binary=${1:-$root/native/target/debug/rom-uploader}
binary=$(realpath "$binary")
work=$(mktemp -d)
server=
cleanup() {
  if [[ -n ${server:-} ]]; then kill "$server" 2>/dev/null || true; wait "$server" 2>/dev/null || true; fi
  rm -rf "$work"
}
trap cleanup EXIT
mkdir -p "$work/data/nes-deck/menu" "$work/data/nes-deck/uploader" "$work/assets"
: > "$work/data/nes-deck/menu/games.tsv"
cp "$root/deploy/menu/palette.tsv" "$work/data/nes-deck/menu/"
cp "$root/deploy/uploader/"uploader-{paper.css,palette.js} "$work/assets/"
printf '%s\n' configured-test-password | python3 "$root/ops/lib/set-uploader-password.py" "$work/data/nes-deck/uploader/password.conf"
"$binary" --check-uploader-password-config "$work/data/nes-deck/uploader/password.conf"
printf '%s' configured-test-password | "$binary" --verify-uploader-password "$work/data/nes-deck/uploader/password.conf"
if printf wrong-password | "$binary" --verify-uploader-password "$work/data/nes-deck/uploader/password.conf"; then exit 1; fi
chmod 644 "$work/data/nes-deck/uploader/password.conf"
if "$binary" --check-uploader-password-config "$work/data/nes-deck/uploader/password.conf" 2>/dev/null; then exit 1; fi
chmod 600 "$work/data/nes-deck/uploader/password.conf"
printf '\002\000\252\252' > "$work/raw-game.tap"
printf '\002\000\125\125' > "$work/zip-game.tap"
mkdir "$work/zip-source"
cp "$work/zip-game.tap" "$work/zip-source/zip-game.tap"
(cd "$work/zip-source" && zip -q "$work/one.zip" zip-game.tap)
printf '#!/bin/sh\nprintf "restart\\n" >> "%s/restart.log"\n' "$work" > "$work/restart-helper"
chmod +x "$work/restart-helper"
: > "$work/restart.log"
"$binary" --data-root "$work/data" --assets "$work/assets" --address 127.0.0.1 --port 0 --ready-file "$work/port" --restart-program "$work/restart-helper" > "$work/server.log" 2>&1 &
server=$!
for ((attempt=0; attempt<100; attempt++)); do [[ -s $work/port ]] && break; kill -0 "$server" || break; sleep 0.1; done
[[ -s $work/port ]] || { cat "$work/server.log"; exit 1; }
port=$(cat "$work/port")
origin=http://127.0.0.1:$port
die() { printf 'uploader-rust-http: %s\n' "$*" >&2; cat "$work/server.log" >&2; head -c 4096 "$work/response.body" >&2; exit 1; }
request() {
  local expected=$1 status
  shift
  status=$(curl --silent --show-error --max-time 15 --http1.1 -D "$work/response.headers" -o "$work/response.body" -w '%{http_code}' "$@") || die 'curl failed'
  tr -d '\r' < "$work/response.headers" > "$work/response.headers.clean"
  [[ $status == "$expected" ]] || die "expected HTTP $expected, got $status"
}
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
request 400 -H "Origin: $origin" \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  --data 'password=%FF' "$origin/login"
head -c 513 /dev/zero > "$work/oversize-login"
request 400 -H "Origin: $origin" \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  --data-binary "@$work/oversize-login" "$origin/login"
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

# Rejections must leave both the ROM directory and catalog unchanged.
(cd "$work/zip-source" && zip -q "$work/traversal.zip" ../zip-game.tap)
ln -s ../zip-game.tap "$work/zip-source/link.tap"
(cd "$work/zip-source" && zip -qy "$work/link.zip" link.tap)
(cd "$work/zip-source" && zip -qP fixture-password "$work/encrypted.zip" zip-game.tap)
cp "$work/raw-game.tap" "$work/zip-source/second.tap"
(cd "$work/zip-source" && zip -q "$work/multiple.zip" zip-game.tap second.tap)
printf 'not a zip' > "$work/malformed.zip"
for rejected in traversal link encrypted multiple malformed; do
  request 422 -b "$work/cookies" -H "Origin: $origin" \
    -F "csrf=$csrf" -F system=zx -F "title=Rejected $rejected" \
    -F "rom=@$work/$rejected.zip;filename=$rejected.zip" "$origin/upload"
done
printf '\002\000\125\126' > "$work/bad-checksum.tap"
request 422 -b "$work/cookies" -H "Origin: $origin" \
  -F "csrf=$csrf" -F system=zx -F 'title=Bad Checksum' \
  -F "rom=@$work/bad-checksum.tap;filename=bad.tap" "$origin/upload"
request 400 -b "$work/cookies" -H "Origin: $origin" \
  -F "csrf=$csrf" -F system=zx -F 'title=Two Files' \
  -F "rom=@$work/raw-game.tap" -F "rom=@$work/zip-game.tap" "$origin/upload"
head -c 10485761 /dev/zero > "$work/too-big.gba"
request 422 -b "$work/cookies" -H "Origin: $origin" \
  -F "csrf=$csrf" -F system=gba -F 'title=Oversize Raw' \
  -F "rom=@$work/too-big.gba" "$origin/upload"
head -c 12582913 /dev/zero > "$work/too-big-request.gba"
request 400 -b "$work/cookies" -H "Origin: $origin" \
  -F "csrf=$csrf" -F system=gba -F 'title=Oversize Request' \
  -F "rom=@$work/too-big-request.gba" "$origin/upload"
head -c 8388609 /dev/zero > "$work/zip-source/too-big.nes"
(cd "$work/zip-source" && zip -q "$work/expansion.zip" too-big.nes)
request 422 -b "$work/cookies" -H "Origin: $origin" \
  -F "csrf=$csrf" -F system=nes -F 'title=Oversize Zip' \
  -F "rom=@$work/expansion.zip" "$origin/upload"
cmp "$work/expected-games.tsv" "$work/data/nes-deck/uploads/games.tsv"
[[ $(find "$work/data/roms" -type f | wc -l) -eq 2 ]]
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

# Repeated/unknown palette fields and CSRF failures cannot alter saved state.
cp "$override" "$work/saved-palette"
request 400 -b "$work/cookies" -H "Origin: $origin" \
  "${palette_args[@]}" --data-urlencode 'accent=#112233' "$origin/palette"
request 400 -b "$work/cookies" -H "Origin: $origin" \
  "${palette_args[@]}" --data-urlencode 'unknown=#112233' "$origin/palette"
request 403 -b "$work/cookies" -H "Origin: $origin" \
  "${palette_args[@]}" --data-urlencode "csrf=$csrf" "$origin/palette"
cmp "$work/saved-palette" "$override"
request 403 -b "$work/cookies" -H 'Origin: http://example.com' \
  --data-urlencode "csrf=$csrf" "$origin/palette/reset"
request 200 -b "$work/cookies" -H "Origin: $origin" \
  --data-urlencode "csrf=$csrf" "$origin/palette/reset"
[[ ! -e $override ]]
[[ $(wc -l < "$work/restart.log") -eq 4 ]]
request 200 -b "$work/cookies" "$origin/"
base_background=$(awk -F '\t' '$1 == "background" { print $2 }' "$work/data/nes-deck/menu/palette.tsv")
grep -q "name=\"background\" value=\"$base_background\"" "$work/response.body"
# Saving succeeds independently of an init-script restart failure.
printf '#!/bin/sh\nexit 7\n' > "$work/restart-helper"
request 422 -b "$work/cookies" -H "Origin: $origin" \
  "${palette_args[@]}" "$origin/palette"
[[ -s $override && $(stat -c '%a' "$override") == 600 ]]
request 200 -b "$work/cookies" -H "Origin: $origin" \
  -F "csrf=$csrf" -F system=zx -F 'title=Restart Failed' \
  -F "rom=@$work/raw-game.tap;filename=retry.tap" "$origin/upload"
cmp "$work/raw-game.tap" "$work/data/roms/zx/restart-failed.tap"
grep -q 'upload-zx-restart-failed' "$work/data/nes-deck/uploads/games.tsv"
request 303 -b "$work/cookies" -H "Origin: $origin" \
  --data-urlencode "csrf=$csrf" "$origin/logout"
grep -qi '^Set-Cookie: deck_rom_session=;' "$work/response.headers.clean"
request 200 -b "$work/cookies" "$origin/"
grep -q '<form action="/login" method="post">' "$work/response.body"
if grep -q '<form action="/upload"' "$work/response.body"; then
  die 'logged-out session can still upload'
fi

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

kill -TERM "$server"
timeout 20 tail --pid="$server" -f /dev/null || die 'server did not stop'
wait "$server" || die 'server exited unsuccessfully'
server=
printf 'uploader-rust-http: OK\n'
