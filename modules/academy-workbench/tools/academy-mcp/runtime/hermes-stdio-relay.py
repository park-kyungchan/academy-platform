#!/usr/bin/env python3
"""Forward MCP stdio bytes to the local owner bridge socket; no MCP parsing."""
import os
import socket
import sys
import threading

path = '/opt/data/academy-mcp-relay/bridge.sock'
sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
sock.connect(path)

def receive():
    try:
        while data := sock.recv(65536):
            view = memoryview(data)
            while view:
                view = view[os.write(1, view):]
    finally:
        sock.close()

reader = threading.Thread(target=receive, daemon=True)
reader.start()
try:
    while data := os.read(0, 65536):
        sock.sendall(data)
    sock.shutdown(socket.SHUT_WR)
except (BrokenPipeError, ConnectionResetError, OSError):
    pass
reader.join()
