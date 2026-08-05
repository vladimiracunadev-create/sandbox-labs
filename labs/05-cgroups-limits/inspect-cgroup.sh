#!/usr/bin/env bash
set -euo pipefail
printf 'self cgroup:\n'; cat /proc/self/cgroup
if [ -r /sys/fs/cgroup/cgroup.controllers ]; then
  printf '\ncgroup v2 controllers:\n'; cat /sys/fs/cgroup/cgroup.controllers
else
  printf '\ncgroup v2 no detectado\n'
fi
