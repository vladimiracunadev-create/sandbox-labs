import os, subprocess
count=min(int(os.environ.get("SANDBOX_TEST_PROCESSES", "8")), 32)
children=[]
try:
    for _ in range(count):
        children.append(subprocess.Popen(["python3", "-c", "pass"]))
    for child in children: child.wait(timeout=2)
    print(f"processes completed={len(children)}")
except OSError as exc:
    print(f"processes blocked: {type(exc).__name__}")
