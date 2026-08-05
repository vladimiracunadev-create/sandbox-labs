#!/usr/bin/env python3
"""Diagnóstico sin privilegios para los laboratorios Linux."""
from __future__ import annotations

import json
import os
import platform
import shutil
from pathlib import Path


def command(name: str) -> dict[str, object]:
    path = shutil.which(name)
    return {"name": name, "available": path is not None, "path": path}


def main() -> None:
    checks = {
        "platform": platform.platform(),
        "is_linux": platform.system() == "Linux",
        "is_wsl": "microsoft" in platform.release().lower(),
        "cgroup_v2": Path("/sys/fs/cgroup/cgroup.controllers").exists(),
        "kvm": os.access("/dev/kvm", os.R_OK | os.W_OK),
        "commands": [command(name) for name in ["unshare", "bwrap", "runsc", "wasmtime", "firecracker"]],
    }
    print(json.dumps(checks, indent=2, ensure_ascii=False))


if __name__ == "__main__":
    main()
