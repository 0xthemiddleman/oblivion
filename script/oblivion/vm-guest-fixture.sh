#!/bin/sh
# Laboratory console only. Optional chat probe executes the actual CLI, not the TUI.
set -eu
trap '[ "$?" = 0 ] || printf "OBLIVION_GUEST_PROBE_FAILED\n"' EXIT
printf 'OBLIVION_GUEST_READY\n'
while IFS= read -r command; do
    case "$command" in
        probe)
            [ "$(id -u)" = 1000 ]
            [ ! -e /run/oblivion/general ]
            [ ! -e /dev/mapper/oblivion-general ]
            [ ! -r /dev/vdb ]
            [ "$(wc -l < /proc/swaps)" = 1 ]
            if touch /etc/oblivion-write 2>/dev/null; then exit 1; fi
            grep -q ' / squashfs ro,' /proc/mounts
            grep -q ' /tmp tmpfs ' /proc/mounts
            grep -q ' /var tmpfs ' /proc/mounts
            [ "$(ls /sys/class/net)" = lo ]
            systemctl show systemd-logind.service --property=ActiveState,SubState,Result
            systemctl show systemd-udevd.service --property=ActiveState,SubState,Result
            cat /proc/bus/input/devices
            ls -l /dev/input || true
            ls -d /sys/firmware/devicetree/base/gpio-keys || true
            cat /proc/modules
            printf 'OBLIVION_GUEST_CONFINED\n'
            ;;
        write-a)
            printf OBLIVION_VM_A_CANARY_4d02 > /vault/workspace/document.txt
            printf OBLIVION_VM_TEMP_CANARY_d9ae > /tmp/session-temporary
            printf OBLIVION_VM_TEMP_CANARY_d9ae > /var/tmp/session-temporary
            setsid sh -c 'while :; do sleep 1; done' > /dev/null 2>&1 &
            printf '%s' "$!" > /tmp/detached-worker
            sync
            printf 'OBLIVION_A_WRITTEN\n'
            ;;
        check-b)
            [ ! -e /vault/workspace/document.txt ]
            [ ! -e /tmp/session-temporary ]
            [ ! -e /var/tmp/session-temporary ]
            [ ! -e /tmp/detached-worker ]
            printf OBLIVION_VM_B_CANARY_b0ca > /vault/workspace/other.txt
            sync
            printf 'OBLIVION_B_ISOLATED\n'
            ;;
        check-a)
            [ "$(cat /vault/workspace/document.txt)" = OBLIVION_VM_A_CANARY_4d02 ]
            [ ! -e /vault/workspace/other.txt ]
            [ ! -e /tmp/session-temporary ]
            [ ! -e /var/tmp/session-temporary ]
            [ ! -e /tmp/detached-worker ]
            printf 'OBLIVION_A_RESTORED\n'
            ;;
        imports-empty)
            [ -z "$(ls -A /vault/imports)" ]
            [ ! -r /vault/.oblivion-inbox ]
            printf 'OBLIVION_IMPORTS_EMPTY\n'
            ;;
        imports-check)
            set -- /vault/imports/*
            [ "$#" = 1 ]
            [ "$(cat "$1/files/documenti/note.txt")" = OBLIVION_USB_SELECTED_CANARY_82f1 ]
            [ -d "$1/files/documenti/vuota" ]
            [ ! -e "$1/files/unselected.txt" ]
            [ ! -r /vault/.oblivion-inbox ]
            if touch /vault/imports/chat-created 2>/dev/null; then exit 1; fi
            printf OBLIVION_USB_EDIT_CANARY_1b72 > "$1/files/documenti/note.txt"
            sync
            printf 'OBLIVION_IMPORT_VERIFIED\n'
            ;;
        imports-resumed)
            set -- /vault/imports/*
            [ "$#" = 1 ]
            [ "$(cat "$1/files/documenti/note.txt")" = OBLIVION_USB_EDIT_CANARY_1b72 ]
            [ ! -r /vault/.oblivion-inbox ]
            printf 'OBLIVION_IMPORT_RESUMED\n'
            ;;
        chat-a|chat-b|chat-resume)
            python3 /usr/libexec/oblivion-chat-probe "$command"
            ;;
        *) exit 1 ;;
    esac
done
