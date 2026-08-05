from pathlib import Path
for raw in ["/workspace", "/tmp", "/etc/hostname", "/etc/shadow"]:
    path = Path(raw)
    try:
        data = path.read_bytes()[:32] if path.is_file() else b""
        print(f"{raw}: exists={path.exists()} readable=True bytes={len(data)}")
    except OSError as exc:
        print(f"{raw}: blocked={type(exc).__name__}")
