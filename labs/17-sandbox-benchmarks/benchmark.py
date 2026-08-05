#!/usr/bin/env python3
"""Benchmark de comandos benignos; no impone aislamiento por sí mismo."""
from __future__ import annotations

import argparse
import json
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument("--repeat", type=int, default=5)
parser.add_argument("command", nargs="+", default=["true"])
args = parser.parse_args()

samples = []
for _ in range(max(1, args.repeat)):
    started = time.perf_counter_ns()
    result = subprocess.run(args.command, capture_output=True, text=True, timeout=30, check=False)
    samples.append({"duration_ms": (time.perf_counter_ns() - started) / 1_000_000, "exit_code": result.returncode})
print(json.dumps({"command": args.command, "samples": samples}, indent=2))
