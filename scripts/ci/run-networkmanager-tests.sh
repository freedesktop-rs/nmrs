#!/usr/bin/env bash

set -euo pipefail

readonly mode="${1:-all}"
readonly script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly project_root="$(cd "${script_dir}/../.." && pwd)"
readonly runtime_dir="$(mktemp -d "${TMPDIR:-/tmp}/nmrs-integration.XXXXXX")"
readonly dbus_log="${runtime_dir}/dbus.log"
readonly udev_log="${runtime_dir}/udev.log"
readonly networkmanager_log="${runtime_dir}/networkmanager.log"
readonly hostapd_log="${runtime_dir}/hostapd.log"
readonly dnsmasq_log="${runtime_dir}/dnsmasq.log"
readonly wired_dnsmasq_log="${runtime_dir}/wired-dnsmasq.log"
readonly wpa_supplicant_log="${runtime_dir}/wpa_supplicant.log"
readonly hostapd_config="${runtime_dir}/hostapd.conf"
readonly networkmanager_config="${runtime_dir}/NetworkManager.conf"
readonly dnsmasq_leases="${runtime_dir}/dnsmasq.leases"
readonly wired_dnsmasq_leases="${runtime_dir}/wired-dnsmasq.leases"
readonly hwsim_ssid="nmrs-hwsim"
readonly hwsim_password="nmrs-hwsim-password"
readonly hwsim_gateway="192.168.250.1"
readonly wired_client_interface="nmrs-client"
readonly wired_server_interface="nmrs-server"
readonly wired_gateway="192.168.251.1"
readonly wireguard_interface="wg-nmrs-agent"
dbus_pid=""
udev_pid=""
networkmanager_pid=""
hostapd_pid=""
dnsmasq_pid=""
wired_dnsmasq_pid=""
wpa_supplicant_pid=""
hwsim_station_interface=""
wired_veth_created=false

stop_process() {
    local pid="$1"

    if [[ -n "${pid}" ]] && kill -0 "${pid}" 2>/dev/null; then
        kill "${pid}" || true
        wait "${pid}" || true
    fi
}

cleanup() {
    local exit_code=$?

    stop_process "${networkmanager_pid}"
    stop_process "${wpa_supplicant_pid}"
    stop_process "${dnsmasq_pid}"
    stop_process "${wired_dnsmasq_pid}"
    stop_process "${hostapd_pid}"
    if [[ "${wired_veth_created}" == true ]] && ip link show "${wired_client_interface}" >/dev/null 2>&1; then
        ip link delete "${wired_client_interface}" || true
    fi
    stop_process "${udev_pid}"
    stop_process "${dbus_pid}"

    if (( exit_code != 0 )); then
        echo "Integration harness failed; service logs follow:" >&2
        for log_file in \
            "${dbus_log}" \
            "${udev_log}" \
            "${networkmanager_log}" \
            "${wpa_supplicant_log}" \
            "${hostapd_log}" \
            "${dnsmasq_log}" \
            "${wired_dnsmasq_log}"; do
            if [[ -s "${log_file}" ]]; then
                printf '\n===== %s =====\n' "$(basename "${log_file}")" >&2
                cat "${log_file}" >&2 || true
            fi
        done
    fi

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

print_wpa_supplicant_log() {
    echo "wpa_supplicant did not become ready. Its log follows:" >&2
    cat "${wpa_supplicant_log}" >&2 || true
}

print_dnsmasq_log() {
    echo "dnsmasq did not become ready. Its log follows:" >&2
    cat "${dnsmasq_log}" >&2 || true
}

start_dbus() {
    mkdir -p /run/dbus
    rm -f /run/dbus/system_bus_socket

    dbus-daemon \
        --config-file="${project_root}/scripts/ci/dbus-system.conf" \
        --nofork \
        --nopidfile >"${dbus_log}" 2>&1 &
    dbus_pid=$!

    for _ in $(seq 1 15); do
        if dbus-send \
            --system \
            --dest=org.freedesktop.DBus \
            --type=method_call \
            --print-reply \
            /org/freedesktop/DBus \
            org.freedesktop.DBus.ListNames >/dev/null 2>&1; then
            return
        fi

        if ! kill -0 "${dbus_pid}" 2>/dev/null; then
            echo "The isolated system D-Bus exited before becoming ready" >&2
            cat "${dbus_log}" >&2 || true
            exit 1
        fi

        sleep 1
    done

    echo "The isolated system D-Bus did not become ready" >&2
    cat "${dbus_log}" >&2 || true
    exit 1
}

start_udev() {
    local udevd

    if [[ -x /usr/lib/systemd/systemd-udevd ]]; then
        udevd=/usr/lib/systemd/systemd-udevd
    elif [[ -x /lib/systemd/systemd-udevd ]]; then
        udevd=/lib/systemd/systemd-udevd
    else
        echo "systemd-udevd is required for deterministic veth initialization" >&2
        exit 1
    fi

    mkdir -p /run/udev
    "${udevd}" --debug --resolve-names=never >"${udev_log}" 2>&1 &
    udev_pid=$!

    for _ in $(seq 1 15); do
        if udevadm control --ping >/dev/null 2>&1; then
            return
        fi
        if ! kill -0 "${udev_pid}" 2>/dev/null; then
            echo "The private udev daemon exited before becoming ready" >&2
            cat "${udev_log}" >&2 || true
            exit 1
        fi
        sleep 1
    done

    echo "The private udev daemon did not become ready" >&2
    cat "${udev_log}" >&2 || true
    exit 1
}

start_wpa_supplicant() {
    mkdir -p /run/wpa_supplicant

    wpa_supplicant \
        -u \
        -s \
        -O /run/wpa_supplicant \
        -f "${wpa_supplicant_log}" > /dev/null 2>&1 &
    wpa_supplicant_pid=$!

    for _ in $(seq 1 15); do
        if dbus-send \
            --system \
            --dest=org.freedesktop.DBus \
            --type=method_call \
            --print-reply \
            /org/freedesktop/DBus \
            org.freedesktop.DBus.NameHasOwner \
            string:fi.w1.wpa_supplicant1 2>/dev/null | grep --quiet 'boolean true'; then
            return
        fi

        if ! kill -0 "${wpa_supplicant_pid}" 2>/dev/null; then
            print_wpa_supplicant_log
            exit 1
        fi

        sleep 1
    done

    print_wpa_supplicant_log
    exit 1
}

setup_hwsim_access_point() {
    local ap_interface
    local -a wifi_interfaces

    mapfile -t wifi_interfaces < <(
        iw dev | awk '$1 == "Interface" { print $2 }' |
            while read -r interface; do
                if ethtool -i "${interface}" 2>/dev/null | grep --fixed-strings --quiet 'driver: mac80211_hwsim'; then
                    printf '%s\n' "${interface}"
                fi
            done | sort
    )
    if (( ${#wifi_interfaces[@]} != 2 )); then
        echo "Expected exactly two mac80211_hwsim interfaces, found ${#wifi_interfaces[@]}" >&2
        iw dev >&2 || true
        exit 1
    fi

    ap_interface="${wifi_interfaces[0]}"
    hwsim_station_interface="${wifi_interfaces[1]}"

    printf '%s\n' \
        "interface=${ap_interface}" \
        'driver=nl80211' \
        "ssid=${hwsim_ssid}" \
        'hw_mode=g' \
        'channel=1' \
        'wpa=2' \
        "wpa_passphrase=${hwsim_password}" \
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

    ip link set "${ap_interface}" up
    ip address replace "${hwsim_gateway}/24" dev "${ap_interface}"

    # NetworkManager's activation does not complete until DHCP succeeds. Run a
    # DHCP-only dnsmasq bound to the hwsim AP interface; port=0 avoids exposing
    # a DNS listener in the host network namespace used by the WiFi container.
    dnsmasq \
        --no-daemon \
        --conf-file=/dev/null \
        --interface="${ap_interface}" \
        --bind-interfaces \
        --port=0 \
        --dhcp-authoritative \
        --dhcp-range=192.168.250.10,192.168.250.50,255.255.255.0,1h \
        --dhcp-option=3,"${hwsim_gateway}" \
        --dhcp-leasefile="${dnsmasq_leases}" \
        --log-dhcp >"${dnsmasq_log}" 2>&1 &
    dnsmasq_pid=$!

    for _ in $(seq 1 10); do
        if grep --quiet 'DHCP, IP range' "${dnsmasq_log}"; then
            break
        fi

        if ! kill -0 "${dnsmasq_pid}" 2>/dev/null; then
            print_dnsmasq_log
            exit 1
        fi

        sleep 1
    done

    if ! kill -0 "${dnsmasq_pid}" 2>/dev/null; then
        print_dnsmasq_log
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
        '[keyfile]' \
        "unmanaged-devices=*,except:interface-name:${hwsim_station_interface}" \
        '' \
        '[device-hwsim-station]' \
        "match-device=interface-name:=${hwsim_station_interface}" \
        'managed=1' >"${networkmanager_config}"
}

setup_veth_network() {
    # The pair normally disappears with its container namespace. Remove a
    # stale pair defensively for interactive/reused-container runs.
    if ip link show "${wired_client_interface}" >/dev/null 2>&1; then
        ip link delete "${wired_client_interface}"
    elif ip link show "${wired_server_interface}" >/dev/null 2>&1; then
        ip link delete "${wired_server_interface}"
    fi
    ip link add "${wired_client_interface}" type veth peer name "${wired_server_interface}"
    wired_veth_created=true
    ip address replace "${wired_gateway}/24" dev "${wired_server_interface}"
    ip link set "${wired_server_interface}" up
    ip link set "${wired_client_interface}" up
    udevadm trigger --action=add --subsystem-match=net
    udevadm settle --timeout=10

    dnsmasq \
        --no-daemon \
        --conf-file=/dev/null \
        --interface="${wired_server_interface}" \
        --bind-interfaces \
        --port=0 \
        --dhcp-authoritative \
        --dhcp-range=192.168.251.10,192.168.251.50,255.255.255.0,1h \
        --dhcp-option=3,"${wired_gateway}" \
        --dhcp-leasefile="${wired_dnsmasq_leases}" \
        --log-dhcp >"${wired_dnsmasq_log}" 2>&1 &
    wired_dnsmasq_pid=$!

    for _ in $(seq 1 10); do
        if grep --quiet 'DHCP, IP range' "${wired_dnsmasq_log}"; then
            break
        fi
        if ! kill -0 "${wired_dnsmasq_pid}" 2>/dev/null; then
            echo "Wired dnsmasq exited before becoming ready" >&2
            cat "${wired_dnsmasq_log}" >&2 || true
            exit 1
        fi
        sleep 1
    done
    if ! kill -0 "${wired_dnsmasq_pid}" 2>/dev/null; then
        echo "Wired dnsmasq did not become ready" >&2
        cat "${wired_dnsmasq_log}" >&2 || true
        exit 1
    fi

    # Docker's eth0 stays visible but unmanaged. Only the private veth client
    # may be selected by the isolated NetworkManager wired test.
    printf '%s\n' \
        '[main]' \
        'plugins=keyfile' \
        'no-auto-default=*' \
        'auth-polkit=root-only' \
        'dhcp=internal' \
        '' \
        '[keyfile]' \
        "unmanaged-devices=*,except:interface-name:${wired_client_interface},except:interface-name:${wireguard_interface}" \
        '' \
        '[device-veth-client]' \
        "match-device=interface-name:=${wired_client_interface}" \
        'managed=1' \
        '' \
        '[device-wireguard]' \
        "match-device=interface-name:=${wireguard_interface}" \
        'managed=1' >"${networkmanager_config}"
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
elif [[ "${mode}" == "all" || "${mode}" == "integration" ]]; then
    start_udev
    setup_veth_network
fi

start_dbus

if [[ "${mode}" == "wifi-integration" ]]; then
    start_wpa_supplicant

    NetworkManager \
        --config="${networkmanager_config}" \
        --no-daemon \
        --log-level=INFO >"${networkmanager_log}" 2>&1 &
elif [[ "${mode}" == "all" || "${mode}" == "integration" ]]; then
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
    nmcli device set "${hwsim_station_interface}" managed yes
    nmcli radio wifi on

    for _ in $(seq 1 30); do
        station_state="$(nmcli --terse --fields DEVICE,TYPE,STATE device status | awk -F: -v interface="${hwsim_station_interface}" '$1 == interface && $2 == "wifi" { print $3; exit }')"
        if [[ "${station_state}" == "disconnected" || "${station_state}" == "connected" ]]; then
            break
        fi

        sleep 1
    done

    if [[ "${station_state:-}" != "disconnected" && "${station_state:-}" != "connected" ]]; then
        echo "NetworkManager did not make ${hwsim_station_interface} ready for scanning" >&2
        nmcli device status >&2 || true
        nmcli -f GENERAL.DEVICE,GENERAL.TYPE,GENERAL.STATE,GENERAL.REASON,GENERAL.NM-MANAGED \
            device show "${hwsim_station_interface}" >&2 || true
        print_networkmanager_log
        print_wpa_supplicant_log
        exit 1
    fi

    export NMRS_REQUIRE_WIFI=1
    export NMRS_EXPECT_WIFI_SSID="${hwsim_ssid}"
    export NMRS_WIFI_PASSWORD="${hwsim_password}"
    export NMRS_WIFI_INTERFACE="${hwsim_station_interface}"
elif [[ "${mode}" == "all" || "${mode}" == "integration" ]]; then
    nmcli device set "${wired_client_interface}" managed yes

    for _ in $(seq 1 30); do
        wired_state="$(nmcli --terse --fields DEVICE,TYPE,STATE device status | awk -F: -v interface="${wired_client_interface}" '$1 == interface && $2 == "ethernet" { print $3; exit }')"
        if [[ "${wired_state}" == "disconnected" || "${wired_state}" == "connected" ]]; then
            break
        fi
        sleep 1
    done

    if [[ "${wired_state:-}" != "disconnected" && "${wired_state:-}" != "connected" ]]; then
        echo "NetworkManager did not make ${wired_client_interface} ready" >&2
        nmcli device status >&2 || true
        nmcli -f GENERAL.DEVICE,GENERAL.TYPE,GENERAL.STATE,GENERAL.REASON,GENERAL.NM-MANAGED \
            device show "${wired_client_interface}" >&2 || true
        print_networkmanager_log
        exit 1
    fi

    export NMRS_REQUIRE_WIRED=1
    export NMRS_WIRED_INTERFACE="${wired_client_interface}"
fi

case "${mode}" in
    all)
        cargo test --locked --all-features --workspace
        cargo test --locked --test integration_test --all-features \
            networkmanager_ -- --ignored --test-threads=1
        cargo test --locked --test integration_test --all-features \
            wired_ -- --ignored --test-threads=1
        ;;
    integration)
        cargo test --locked --test integration_test --all-features \
            networkmanager_ -- --ignored --test-threads=1
        cargo test --locked --test integration_test --all-features \
            wired_ -- --ignored --test-threads=1
        ;;
    wifi-integration)
        cargo test --locked --test integration_test --all-features \
            wifi_ -- --ignored --test-threads=1
        ;;
    shell)
        bash
        ;;
esac
