from pathlib import Path
for raw in ["../README.md", "../../etc/passwd", "/etc/shadow"]:
    try:
        value = Path(raw).read_text(errors="ignore")[:24]
        print(f"UNEXPECTED {raw}: {value!r}")
    except OSError as exc:
        print(f"blocked {raw}: {type(exc).__name__}")
