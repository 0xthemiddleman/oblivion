#!/bin/sh
set -eu
umask 077
[ "$(cat /sys/block/vdb/ro)" = 0 ]
# Initial writer: ext4. Other filesystems fail before mounting or creating files.
[ "$(blkid -o value -s TYPE /dev/vdb)" = ext4 ]
mount -t ext4 -o rw,nodev,nosuid,noexec /dev/vdb /target
exec 3<>/dev/virtio-ports/oblivion.export
printf 'ready\n' >&3
[ "$(dd bs=1 count=1 <&3 2>/dev/null)" = G ]
/usr/bin/oblivion-copy export /target 300000
# The receipt alone is not an eject confirmation. Only a successful unmount
# and block-device flush authorize the relay to acknowledge the export.
umount /target
blockdev --flushbufs /dev/vdb
printf 'detached\n' >&3
