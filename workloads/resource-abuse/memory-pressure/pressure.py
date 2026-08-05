import os, time
limit = min(int(os.environ.get("SANDBOX_TEST_MB", "96")), 256)
chunks=[]
try:
    for _ in range(limit):
        chunks.append(bytearray(1024 * 1024))
    print(f"memory allocated={len(chunks)}MB")
    time.sleep(0.2)
except MemoryError:
    print("memory blocked")
