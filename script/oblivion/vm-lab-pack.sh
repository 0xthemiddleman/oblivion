#!/bin/sh
set -eu
mkdir -p /out /host/usr/lib/oblivion/vm/qemu-root/usr/bin
cp /host/boot/vmlinuz-* /out/kernel
cp /host/boot/initrd.img-* /out/initrd
chroot /host /usr/bin/dpkg-query -W -f='${Package}\t${Version}\n' > /out/host-packages.tsv
chroot /guest /usr/bin/dpkg-query -W -f='${Package}\t${Version}\n' > /out/guest-packages.tsv
cp /out/kernel /host/usr/lib/oblivion/vm/kernel
cp /out/initrd /host/usr/lib/oblivion/vm/initrd
chmod 0644 /host/usr/lib/oblivion/vm/kernel /host/usr/lib/oblivion/vm/initrd
# The hypervisor's root contains public executables/libraries, no configuration,
# profile, host devices or session filesystems. systemd binds just the active disk.
for tree in usr/share/qemu usr/share/qemu-efi-aarch64 usr/share/seabios usr/share/ipxe; do
    if [ -e "/host/$tree" ]; then
        mkdir -p "/host/usr/lib/oblivion/vm/qemu-root/$(dirname "$tree")"
        cp -a "/host/$tree" "/host/usr/lib/oblivion/vm/qemu-root/$tree"
    fi
done
for binary in /host/usr/bin/qemu-system-* /host/usr/lib/*/qemu/*.so; do
    [ -f "$binary" ] || continue
    relative=${binary#/host}
    (cd /host && cp -L --parents ".$relative" /host/usr/lib/oblivion/vm/qemu-root/)
    chroot /host /usr/bin/ldd "$relative" | awk '/=> \// {print $3} /^[[:space:]]*\// {print $1}' | while read -r library; do
        (cd /host && cp -L --parents ".$library" /host/usr/lib/oblivion/vm/qemu-root/)
    done
done
for directory in dev proc sys run/oblivion-session tmp; do mkdir -p "/host/usr/lib/oblivion/vm/qemu-root/$directory"; done
# Mount targets must exist before packing the immutable hypervisor root.
touch /host/usr/lib/oblivion/vm/qemu-root/kernel /host/usr/lib/oblivion/vm/qemu-root/initrd /host/usr/lib/oblivion/vm/qemu-root/root.squashfs
rm -rf /guest/boot /guest/var/cache/apt /guest/var/lib/apt/lists /host/var/cache/apt /host/var/lib/apt/lists
mkdir -p /guest/boot
mksquashfs /guest /host/usr/lib/oblivion/vm/guest.squashfs -noappend -comp zstd -processors 2 -quiet
mksquashfs /host /out/host.squashfs -noappend -comp zstd -processors 2 -quiet
cd /sources
find packages script -type f -print0 | sort -z | xargs -0 sha256sum > /out/sources.sha256
cd /out
sha256sum kernel initrd host.squashfs host-packages.tsv guest-packages.tsv sources.sha256 > SHA256SUMS
