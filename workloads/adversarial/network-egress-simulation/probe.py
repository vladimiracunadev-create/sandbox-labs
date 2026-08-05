import socket
try:
    with socket.create_connection(("1.1.1.1", 443), timeout=1):
        print("UNEXPECTED network allowed")
except OSError as exc:
    print(f"blocked network: {type(exc).__name__}")
