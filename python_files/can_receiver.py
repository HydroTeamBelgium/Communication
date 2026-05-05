#!/usr/bin/env python3
"""
CAN Data UDP Broadcast Receiver

Receives CAN frame data broadcast by Nucleo 1 via UDP on port 9999
and displays it in a human-readable format.

Usage:
    python3 can_receiver.py [--port 9999] [--listen 0.0.0.0]

Press Ctrl+C to stop.
"""

import socket
import struct
import sys
from datetime import datetime
from typing import Tuple, Optional

# Message type constants (must match Rust code)
MSG_TYPE_BYTES = 0x01
MSG_TYPE_POT = 0x02
MSG_TYPE_CAN_FRAME = 0x03
MSG_TYPE_ECU_JSON = 0x04


class CanFrameData:
    """Represents CAN frame data received over UDP"""

    def __init__(self, can_id: int, data: bytes, dlc: int):
        self.can_id = can_id
        self.data = data
        self.dlc = dlc
        self.timestamp = datetime.now()

    @staticmethod
    def from_bytes(payload: bytes) -> Optional["CanFrameData"]:
        """Deserialize from 13-byte payload: [CAN_ID:4][DATA:8][DLC:1]"""
        if len(payload) < 13:
            return None

        can_id = struct.unpack(">I", payload[0:4])[0]
        data = payload[4:12]
        dlc = payload[12]

        if dlc > 8:
            return None

        return CanFrameData(can_id, data, dlc)

    def __str__(self) -> str:
        """Format as human-readable string"""
        data_hex = " ".join(f"{b:02X}" for b in self.data[: self.dlc])
        return f"CAN ID: 0x{self.can_id:03X} | DLC: {self.dlc} | Data: {data_hex} | Time: {self.timestamp.strftime('%H:%M:%S.%f')[:-3]}"


class EcuJsonData:
    """Represents ECU SCS JSON logging data received over UDP"""

    def __init__(self, json_str: str):
        self.json_str = json_str
        self.timestamp = datetime.now()

    @staticmethod
    def from_bytes(payload: bytes) -> Optional["EcuJsonData"]:
        """Deserialize from variable-length payload: [LEN:1][JSON:...]"""
        if len(payload) < 1:
            return None

        length = payload[0]
        if len(payload) < 1 + length:
            return None

        try:
            json_str = payload[1 : 1 + length].decode("utf-8")
            return EcuJsonData(json_str)
        except UnicodeDecodeError:
            return None

    def __str__(self) -> str:
        """Format as human-readable string"""
        return f"ECU JSON: {self.json_str} | Time: {self.timestamp.strftime('%H:%M:%S.%f')[:-3]}"


class CanReceiver:
    """UDP receiver for CAN broadcast data"""

    def __init__(self, port: int = 9999, listen_addr: str = "0.0.0.0"):
        self.port = port
        self.listen_addr = listen_addr
        self.socket = None
        self.frame_count = 0
        self.error_count = 0

    def setup(self) -> bool:
        """Create and bind UDP socket"""
        try:
            self.socket = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            self.socket.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)

            # Enable receiving broadcast messages
            self.socket.setsockopt(socket.SOL_SOCKET, socket.SO_BROADCAST, 1)

            self.socket.bind((self.listen_addr, self.port))
            print(
                f"✓ Listening on {self.listen_addr}:{self.port} for CAN broadcast data"
            )
            print(f"✓ Waiting for CAN frames from Nucleo 1...\n")
            return True
        except Exception as e:
            print(f"✗ Failed to setup socket: {e}", file=sys.stderr)
            return False

    def receive_and_process(self) -> None:
        """Main receive loop"""
        if not self.socket:
            print("✗ Socket not initialized", file=sys.stderr)
            return

        try:
            while True:
                try:
                    data, addr = self.socket.recvfrom(1024)

                    if len(data) < 1:
                        continue

                    msg_type = data[0]

                    if msg_type == MSG_TYPE_ECU_JSON:
                        ecu_json = EcuJsonData.from_bytes(data[1:])
                        if ecu_json:
                            self.frame_count += 1
                            print(
                                f"[{self.frame_count:6d}] {ecu_json} (from {addr[0]}:{addr[1]})"
                            )
                        else:
                            self.error_count += 1
                            print(
                                f"[ERROR] Failed to parse ECU JSON from {addr}",
                                file=sys.stderr,
                            )
                    elif msg_type == MSG_TYPE_CAN_FRAME:
                        frame = CanFrameData.from_bytes(data[1:])
                        if frame:
                            self.frame_count += 1
                            print(
                                f"[{self.frame_count:6d}] {frame} (from {addr[0]}:{addr[1]})"
                            )
                        else:
                            self.error_count += 1
                            print(
                                f"[ERROR] Failed to parse CAN frame from {addr}",
                                file=sys.stderr,
                            )
                    elif msg_type == MSG_TYPE_POT:
                        if len(data) >= 5:
                            voltage = struct.unpack(">f", data[1:5])[0]
                            print(
                                f"[{self.frame_count + 1:6d}] POT Reading: {voltage:.3f}V (from {addr[0]}:{addr[1]})"
                            )
                    elif msg_type == MSG_TYPE_BYTES:
                        if len(data) >= 17:
                            payload = data[1:17]
                            try:
                                text = payload.decode("utf-8").rstrip("\x00")
                                print(
                                    f"[{self.frame_count + 1:6d}] Bytes: {text} (from {addr[0]}:{addr[1]})"
                                )
                            except:
                                print(
                                    f"[{self.frame_count + 1:6d}] Bytes (raw): {payload.hex()} (from {addr[0]}:{addr[1]})"
                                )
                    else:
                        print(
                            f"[WARNING] Unknown message type: 0x{msg_type:02X}",
                            file=sys.stderr,
                        )

                except struct.error as e:
                    self.error_count += 1
                    print(f"[ERROR] Parsing error: {e}", file=sys.stderr)
                except Exception as e:
                    self.error_count += 1
                    print(f"[ERROR] {e}", file=sys.stderr)

        except KeyboardInterrupt:
            print(f"\n\n✓ Stopped. Received {self.frame_count} frames, {self.error_count} errors")
        finally:
            if self.socket:
                self.socket.close()


def main():
    """Main entry point"""
    import argparse

    parser = argparse.ArgumentParser(
        description="CAN Data UDP Broadcast Receiver for Nucleo 1",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Listen on all interfaces, port 9999 (default)
  python3 can_receiver.py

  # Listen on specific interface
  python3 can_receiver.py --listen 192.168.1.100

  # Listen on custom port
  python3 can_receiver.py --port 5000

  # Verbose mode (show all statistics)
  python3 can_receiver.py --verbose
        """,
    )

    parser.add_argument(
        "--port",
        type=int,
        default=9999,
        help="UDP port to listen on (default: 9999)",
    )
    parser.add_argument(
        "--listen",
        default="0.0.0.0",
        help="Address to listen on (default: 0.0.0.0 - all interfaces)",
    )
    parser.add_argument(
        "--verbose",
        action="store_true",
        help="Enable verbose output",
    )

    args = parser.parse_args()

    print("=" * 70)
    print("CAN Data UDP Broadcast Receiver")
    print("=" * 70)
    print(f"Configuration:")
    print(f"  Listen Address: {args.listen}")
    print(f"  Port: {args.port}")
    print(f"  Broadcast Addr: 255.255.255.255 (from Nucleo 1)")
    print("=" * 70)
    print()

    receiver = CanReceiver(port=args.port, listen_addr=args.listen)

    if receiver.setup():
        receiver.receive_and_process()
    else:
        sys.exit(1)


if __name__ == "__main__":
    main()
