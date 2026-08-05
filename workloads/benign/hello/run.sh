#!/usr/bin/env sh
set -eu
printf 'hello from sandbox-labs\n'
printf 'cwd=%s\n' "$(pwd)"
printf 'uid=%s gid=%s\n' "$(id -u)" "$(id -g)"
