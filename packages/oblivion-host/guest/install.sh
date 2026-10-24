#!/bin/sh
set -eu
cp /usr/lib/oblivion/guest/oblivion-vault.service /etc/systemd/system/
cp /usr/lib/oblivion/guest/oblivion-conversation.service /etc/systemd/system/
chmod 0755 /usr/lib/oblivion/guest/prepare.sh /usr/libexec/oblivion
printf '/dev/vdb /vault ext4 nodev,nosuid,noatime 0 0\n' >> /etc/fstab
systemctl enable oblivion-vault.service oblivion-conversation.service
