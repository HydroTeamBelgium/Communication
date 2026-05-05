#!/usr/bin/env python3
"""
Installation guide for can_receiver.py

This script has no external dependencies beyond Python 3.7+
It uses only the standard library: socket, struct, sys, datetime

Installation:
1. Python 3.7 or later is required
2. No pip packages needed - uses only stdlib

Usage:
    python3 can_receiver.py [--port 9999] [--listen 0.0.0.0]

Network Setup:
- Nucleo 1 broadcasts CAN data to UDP 255.255.255.255:9999
- This script listens on 0.0.0.0:9999 by default
- Make sure your computer and Nucleo 1 are on the same network
- Broadcast packets may need to be enabled in your network settings

For Windows:
    python can_receiver.py

For Linux/Mac:
    python3 can_receiver.py

Troubleshooting:
- If no data received, check:
  1. Nucleo 1 is powered and running
  2. CAN bus has at least 2 nodes (Nucleo 1 as reader, Nucleo 2 as sender)
  3. Network allows broadcast traffic (port 9999)
  4. Firewall not blocking UDP port 9999
"""

# Test imports
import socket
import struct
import sys
from datetime import datetime

print("✓ All required modules available")
print("✓ Python version:", sys.version)
print("\nYou can now run: python3 can_receiver.py")
