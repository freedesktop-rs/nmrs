#!/usr/bin/env bash

set -euo pipefail

readonly mode="${1:-all}"
readonly script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly project_root="$(cd "${script_dir}/../.." && pwd)"
readonly runtime_dir="$(mktemp -d "${TMPDIR:-/tmp}/nmrs-integration.XXXXXX")"
readonly networkmanager_log="${runtime_dir}/networkmanager.log"
readonly hostapd_log="${runtime_dir}/hostapd.log"
readonly hostapd_config="${runtime_dir}/hostapd.conf"
readonly networkmanager_config="${runtime_dir}/NetworkManager.conf"
networkmanager_pid=""
hostapd_pid=""
hwsim_station_interface=""

stop_process() {
    local pid="$1"

    if [[ -n "${pid}" ]] && kill -0 "${pid}" 2>/dev/null; then
        kill "${pid}" || true
        wait "${pid}" || true
    fi
}

cleanup() {
    stop_process "${networkmanager_pid}"
    stop_process "${hostapd_pid}"
    rm -rf "${runtime_dir}"
}

print_networkmanager_log() {
    echo "NetworkManager did not become ready. Its log follows:" >&2
    cat "${networkmanager_log}" >&2 || true
}

print_hostapd_log() {
    echo "hostapd did not become ready. Its log follows:" >&2
    cat "${hostapd_log}" >&2 || true
}

setup_hwsim_access_point() {
    local ap_interface
    local -a wifi_interfaces

    mapfile -t wifi_interfaces < <(iw dev | awk '$1 == "Interface" { print $2 }' | sort)
    if (( ${#wifi_interfaces[@]} < 2 )); then
        echo "Expected two mac80211_hwsim interfaces, found ${#wifi_interfaces[@]}" >&2
        iw dev >&2 || true
        exit 1
    fi

    ap_interface="${wifi_interfaces[0]}"
    hwsim_station_interface="${wifi_interfaces[1]}"

    printf '%s\n' \
        "interface=${ap_interface}" \
        'driver=nl80211' \
        'ssid=nmrs-hwsim' \
        'hw_mode=g' \
        'channel=1' \
        'wpa=2' \
        'wpa_passphrase=nmrs-hwsim-password' \
        'wpa_key_mgmt=WPA-PSK' \
        'rsn_pairwise=CCMP' >"${hostapd_config}"

    hostapd "${hostapd_config}" >"${hostapd_log}" 2>&1 &
    hostapd_pid=$!

    for _ in $(seq 1 15); do
        if grep --quiet 'AP-ENABLED' "${hostapd_log}"; then
            break
        fi

        if ! kill -0 "${hostapd_pid}" 2>/dev/null; then
            print_hostapd_log
            exit 1
        fi

        sleep 1
    done

    if ! grep --quiet 'AP-ENABLED' "${hostapd_log}"; then
        print_hostapd_log
        exit 1
    fi

    # Keep NetworkManager away from the runner's interfaces and AP radio.
    printf '%s\n' \
        '[main]' \
        'plugins=keyfile' \
        'no-auto-default=*' \
        'auth-polkit=root-only' \
        'dhcp=internal' \
        '' \
        '[device-hwsim-station]' \
        "match-device=interface-name:${hwsim_station_interface}" \
        'managed=1' \
        'stop-match=yes' \
        '' \
        '[device-default]' \
        'match-device=*' \
        'managed=0' \
        'stop-match=yes' >"${networkmanager_config}"
}

trap cleanup EXIT

case "${mode}" in
    all|integration|wifi-integration|shell) ;;
    *)
        echo "Usage: $0 [all|integration|wifi-integration|shell]" >&2
        exit 2
        ;;
esac

if [[ "${mode}" == "wifi-integration" ]]; then
    setup_hwsim_access_point
fi

mkdir -p /run/dbus
dbus-daemon \
    --config-file="${project_root}/scripts/ci/dbus-system.conf" \
    --fork \
    --nopidfile

if [[ "${mode}" == "wifi-integration" ]]; then
    NetworkManager \
        --config="${networkmanager_config}" \
        --no-daemon \
        --log-level=INFO >"${networkmanager_log}" 2>&1 &
else
    NetworkManager --no-daemon --log-level=INFO >"${networkmanager_log}" 2>&1 &
fi
networkmanager_pid=$!

for _ in $(seq 1 30); do
    if nmcli --terse --fields RUNNING general 2>/dev/null | grep --quiet '^running$'; then
        break
    fi

    if ! kill -0 "${networkmanager_pid}" 2>/dev/null; then
        print_networkmanager_log
        exit 1
    fi

    sleep 1
done

if ! nmcli --terse --fields RUNNING general 2>/dev/null | grep --quiet '^running$'; then
    print_networkmanager_log
    exit 1
fi

nmcli general status

export NMRS_REQUIRE_NETWORKMANAGER=1

if [[ "${mode}" == "wifi-integration" ]]; then
    if ! nmcli --terse --fields DEVICE,TYPE device status | grep --fixed-strings --quiet "${hwsim_station_interface}:wifi"; then
        echo "NetworkManager did not detect ${hwsim_station_interface} as Wi-Fi" >&2
        nmcli device status >&2 || true
        exit 1
    fi

    export NMRS_REQUIRE_WIFI=1
    export NMRS_EXPECT_WIFI_SSID=nmrs-hwsim
fi

case "${mode}" in
    all)
        cargo test --locked --all-features --workspace
        ;;
    integration)
        cargo test --locked --test integration_test --all-features
        ;;
    wifi-integration)
        cargo test --locked --test integration_test --all-features
        ;;
    shell)
        bash
        ;;
esac
