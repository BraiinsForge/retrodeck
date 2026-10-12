#!/usr/bin/env bash

# Report one Deck's installed Retro Deck health without changing remote state.

set -euo pipefail
export LC_ALL=C

usage() {
  echo "Usage: $0 [--config PATH] [root@DECK-IP]" >&2
  exit 2
}

script_dir=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
repo_root=$(CDPATH='' cd -- "$script_dir/.." && pwd)
config_library=$script_dir/lib/deck-config.sh
config=$repo_root/deck.conf
target_override=

while [[ $# -gt 0 ]]; do
  case $1 in
    --config)
      [[ $# -ge 2 ]] || usage
      config=$2
      shift 2
      ;;
    -*) usage ;;
    *)
      [[ -z $target_override ]] || usage
      target_override=$1
      shift
      ;;
  esac
done

[[ -f $config_library && ! -L $config_library ]] || {
  echo "Deck configuration library is missing or unsafe: $config_library" >&2
  exit 1
}
# shellcheck source=ops/lib/deck-config.sh disable=SC1091
source "$config_library"
deck_config_load "$config" "$target_override"
target=$DECK_SSH_TARGET

for command in ssh timeout; do
  command -v "$command" >/dev/null 2>&1 || {
    echo "Missing required command: $command" >&2
    exit 1
  }
done

echo "Checking Retro Deck at $target..."
if ! timeout 20 ssh -o BatchMode=yes -o ConnectTimeout=7 -o LogLevel=ERROR \
  "$target" sh -s <<'REMOTE'
set -u

failures=0
report() {
  printf '%-14s %s\n' "$1" "$2"
}
require_process() {
  label=$1
  process=$2
  process_id=$(pidof "$process" 2>/dev/null || :)
  if [ -n "$process_id" ]; then
    report "$label" "running (PID $process_id)"
  else
    report "$label" "NOT RUNNING ($process)"
    failures=$((failures + 1))
  fi
}
interface_address() {
  ip -4 address show dev "$1" 2>/dev/null |
    awk '/inet / { print $2; exit }'
}

if grep -q ' /mnt/data ' /proc/mounts; then
  report DATA '/mnt/data mounted'
else
  report DATA '/mnt/data NOT MOUNTED'
  failures=$((failures + 1))
fi

if [ -x /mnt/data/nes-deck/menu/deck-menu ] &&
   [ -x /mnt/data/nes-deck/uploader/rom-uploader ] &&
   [ -x /mnt/data/nes-deck/chiptune-deck ] &&
   [ -x /mnt/data/nes-deck/ecl/bin/ecl.bin ]; then
  report RUNTIMES 'dashboard, uploader, chiptunes, and Lisp interpreter installed'
else
  report RUNTIMES 'APPLICATION RUNTIME MISSING'
  failures=$((failures + 1))
fi

# Presentation state is written by deployment, never inferred from a fork.
dashboard_expected=1
presentation_mode=$(cat /mnt/data/nes-deck/presentation 2>/dev/null || :)
case "$presentation_mode" in
  stock-bmc)
    presentation='stock BMC compositor'
    if /etc/init.d/bmc-compositor status >/dev/null 2>&1; then
      report PRESENTATION "$presentation running"
    else
      report PRESENTATION "$presentation NOT RUNNING"
      failures=$((failures + 1))
    fi
    if [ -f /run/current-profile/lib/bmc-widgets/retro-deck/manifest.json ] &&
       [ -x /run/current-profile/lib/bmc-widgets/retro-deck/bin/retro-deck ]; then
      report WIDGET 'installed in active Nix profile'
    else
      report WIDGET 'MISSING FROM ACTIVE PROFILE'
      failures=$((failures + 1))
    fi
    if widget_ids=$(/mnt/data/nes-deck/langs/python \
       /mnt/data/nes-deck/install-bmc-scene.py --widget-ids \
       /etc/bmc/config.json 2>/dev/null); then
      report WIDGET-KEY "$widget_ids"
      if enabled_widget_ids=$(/mnt/data/nes-deck/langs/python \
         /mnt/data/nes-deck/install-bmc-scene.py --enabled-widget-ids \
         /etc/bmc/config.json 2>/dev/null); then
        [ -n "$enabled_widget_ids" ] || dashboard_expected=0
      else
        failures=$((failures + 1))
      fi
    else
      report WIDGET-KEY 'CONFIGURED INSTANCE MISSING OR INVALID'
      failures=$((failures + 1))
    fi
    ;;
  framebuffer)
    presentation='direct framebuffer'
    if /etc/init.d/nes-deck status >/dev/null 2>&1; then
      report PRESENTATION "$presentation service running"
    else
      report PRESENTATION "$presentation service NOT RUNNING"
      failures=$((failures + 1))
    fi
    ;;
  *) report PRESENTATION 'MISSING OR INVALID INSTALLATION MODE'
     failures=$((failures + 1)) ;;
esac
# End presentation check.

if [ "$dashboard_expected" -eq 1 ]; then
  require_process DASHBOARD deck-menu
else
  report DASHBOARD 'disabled in BMC configuration'
fi
# Probe the uploader's HTTP endpoint, not just its process.
if wget -q -O /dev/null -T 5 http://127.0.0.1:8080/ 2>/dev/null; then
  report UPLOADER 'running (HTTP 8080)'
else
  report UPLOADER 'NOT RUNNING (HTTP 8080)'
  failures=$((failures + 1))
fi

if [ -x /etc/init.d/deck-wifi ] &&
   /etc/init.d/deck-wifi status >/dev/null 2>&1; then
  report WIFI-WATCHER running
else
  report WIFI-WATCHER 'NOT RUNNING'
  failures=$((failures + 1))
fi

wlan_address=$(interface_address wlan0)
wireguard_address=$(interface_address wg0)
ssid=
if [ -x /usr/sbin/iw ]; then
  ssid=$(/usr/sbin/iw dev wlan0 link 2>/dev/null |
    awk '/^[[:space:]]*SSID: / { sub(/^[[:space:]]*SSID: /, ""); print; exit }' |
    tr -cd ' -~')
elif [ -x /usr/bin/iwinfo ]; then
  ssid=$(/usr/bin/iwinfo wlan0 info 2>/dev/null |
    awk 'NR == 1 { sub(/^.*ESSID: "/, ""); sub(/"$/, ""); print }' |
    tr -cd ' -~')
fi
report SSID "${ssid:-name unavailable}"
if [ -n "$wlan_address" ]; then
  report WLAN "$wlan_address"
else
  report WLAN 'NO IPv4 ADDRESS'
  failures=$((failures + 1))
fi
if [ -n "$wireguard_address" ]; then
  report WIREGUARD "$wireguard_address"
else
  report WIREGUARD 'NO IPv4 ADDRESS'
  failures=$((failures + 1))
fi

if [ -r /var/run/deck-wifi/status ]; then
  wifi_status=$(sed -n '1p' /var/run/deck-wifi/status)
  report WIFI-STATE "${wifi_status:-EMPTY STATUS}"
else
  report WIFI-STATE 'STATUS FILE MISSING'
  failures=$((failures + 1))
fi

if [ -d /mnt/data ]; then
  disk=$(df -h /mnt/data 2>/dev/null | awk 'NR == 2 { print $3 " used, " $4 " free" }')
  report STORAGE "${disk:-usage unavailable}"
fi

log=/mnt/data/nes-deck/log/deck-menu.log
if [ -r "$log" ]; then
  printf '\nRecent dashboard log:\n'
  tail -n 20 "$log"
else
  report LOG 'dashboard log is missing'
  failures=$((failures + 1))
fi

printf '\n'
if [ "$failures" -eq 0 ]; then
  report RESULT 'HEALTHY'
  exit 0
fi
report RESULT "$failures REQUIRED CHECKS FAILED"
exit 1
REMOTE
then
  echo "Retro Deck health check failed or timed out at $target." >&2
  exit 1
fi
