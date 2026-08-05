#!/usr/bin/env python3
"""Sonda local: por defecto solo resuelve localhost; no escanea la red."""
from __future__ import annotations

import argparse
import socket

parser = argparse.ArgumentParser()
parser.add_argument("host", nargs="?", default="localhost")
args = parser.parse_args()

try:
    results = socket.getaddrinfo(args.host, None)
    addresses = sorted({item[4][0] for item in results})
    print(f"allowed/resolved {args.host}: {', '.join(addresses)}")
except OSError as exc:
    print(f"blocked/unresolved {args.host}: {exc}")
