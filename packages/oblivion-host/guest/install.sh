#!/bin/sh
set -eu
cp /usr/lib/oblivion/guest/oblivion-vault.service /etc/systemd/system/
cp /usr/lib/oblivion/guest/oblivion-conversation.service /etc/systemd/system/
cp /usr/lib/oblivion/guest/oblivion-session-network.service /etc/systemd/system/
chmod 0755 /usr/lib/oblivion/guest/*.sh /usr/libexec/oblivion
mkdir -p /etc/modules-load.d /etc/udev/rules.d
printf 'virtio_net\nvirtio_console\n' > /etc/modules-load.d/oblivion-session.conf
printf '%s\n' 'SUBSYSTEM=="virtio-ports", ATTR{name}=="oblivion.session", OWNER="root", GROUP="root", MODE="0600"' > /etc/udev/rules.d/70-oblivion-session.rules
printf '/dev/vdb /vault ext4 nodev,nosuid,noatime 0 0\n' >> /etc/fstab
systemctl enable oblivion-vault.service oblivion-conversation.service
