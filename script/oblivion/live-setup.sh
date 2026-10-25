#!/bin/sh
set -eu
groupadd --gid 1000 oblivion-manager
useradd --uid 1000 --gid 1000 --no-create-home --home-dir /run/oblivion-manager --shell /usr/sbin/nologin oblivion-manager
for entry in '1100 oblivion-vm' '1101 oblivion-gateway-vm' '1102 oblivion-import-vm' '1103 oblivion-export-vm'; do
    set -- $entry
    groupadd --gid "$1" "$2"
    useradd --uid "$1" --gid "$1" --no-create-home --shell /usr/sbin/nologin "$2"
done
passwd -l root
chmod 0600 /etc/oblivion/controller.json
mkdir -p /persist /etc/systemd/system/oblivion-manager.service.d /etc/modules-load.d
mkdir -p /etc/network
# Keep the base network configuration explicit even without ifupdown.
printf 'auto lo\niface lo inet loopback\n' > /etc/network/interfaces
# This first boot image has no USB provisioner yet. The archive destination is
# read-only so the manager cannot silently create a "persistent" vault in RAM.
printf 'tmpfs /persist tmpfs ro,nosuid,nodev,noexec,size=4k,mode=0700 0 0\n' >> /etc/fstab
cat > /etc/systemd/system/oblivion-manager.service.d/live.conf <<'EOF'
[Service]
Environment=OBLIVION_STORAGE_UNAVAILABLE=1
# PrivateDevices keeps physical devices hidden; expose only this console.
BindPaths=/dev/tty1
DeviceAllow=/dev/tty1 rw
EOF
printf 'loop\ndm_crypt\nkvm\n' > /etc/modules-load.d/oblivion-live.conf
# The guest's minimal initrd is not enough for booting physical USB or SATA.
printf 'MODULES=most\nCOMPRESS=gzip\nRESUME=none\n' > /etc/initramfs-tools/conf.d/oblivion
printf 'squashfs\noverlay\next4\niso9660\nloop\n' > /etc/initramfs-tools/modules
update-initramfs -u -k all
# No live-config autologin, SSH, network setup, disk persistence or swap probing.
systemctl enable oblivion-controller.service
ln -sf /usr/lib/systemd/system/multi-user.target /etc/systemd/system/default.target
printf 'Oblivion development boot image\n' > /etc/issue
rm -f /var/log/*log /var/log/wtmp /var/log/btmp /var/log/lastlog
