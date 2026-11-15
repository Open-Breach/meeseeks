#!/bin/sh
# The first and only program of the emulated machine in-vm.sh boots: the
# least a headless Plasma needs, the job in-vm.sh wrote into the repository's
# testing/out, and off again. in-vm.sh packs it into the machine's image.
export PATH=/usr/sbin:/usr/bin:/sbin:/bin HOME=/root
mountpoint -q /proc || mount -t proc proc /proc
mountpoint -q /sys || mount -t sysfs sys /sys
mountpoint -q /dev || mount -t devtmpfs dev /dev
mkdir -p /dev/pts /dev/shm
mountpoint -q /dev/pts || mount -t devpts devpts /dev/pts
mount -t tmpfs tmpfs /dev/shm
mount -t tmpfs tmpfs /run
mount -t tmpfs tmpfs /tmp
modprobe vgem
modprobe 9pnet_virtio; modprobe 9p
mkdir -p /opt/meeseeks
mount -t 9p -o trans=virtio,version=9p2000.L,msize=262144 repo /opt/meeseeks
hostname vm
ip link set lo up 2>/dev/null
echo "vm: up after $(cut -d' ' -f1 /proc/uptime) s; /dev/dri has: $(ls /dev/dri 2>/dev/null | tr '\n' ' ')"
sh /opt/meeseeks/testing/out/vm-job.sh > /opt/meeseeks/testing/out/vm-job.log 2>&1
echo "vm: job ended with $? after $(cut -d' ' -f1 /proc/uptime) s"
sync
poweroff -f
