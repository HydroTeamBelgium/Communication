import threading
import socket
import serial
import time
from tkinter import filedialog

# Configuration
PORT = 'COM11'  # Change this if needed
BAUD = 115200
UDP_LOOPBACK_PORT = 4321  # Must match what the Nucleo uses

def receive_udp(usb_done_event):
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind(('10.42.0.1', UDP_LOOPBACK_PORT))
    sock.settimeout(1.0)  # Set timeout to avoid blocking indefinitely
    print(f"[UDP] Listening on port {UDP_LOOPBACK_PORT}...")

    total_received = 0
    expected_size = None
    received_data = bytearray()
    timeout_after_usb = 2.0  # Seconds to wait after USB is done

    start_wait = None

    while True:
        try:
            data, addr = sock.recvfrom(4096)
            if not expected_size:
                expected_size = int.from_bytes(data[:4], 'big')
                received_data.extend(data[4:])
            else:
                received_data.extend(data)

            total_received = len(received_data)
            print(f"[UDP] Received {len(data)} bytes (total: {total_received}/{expected_size})")

            # Reset post-USB timer if more data comes in
            start_wait = None

            if total_received >= expected_size:
                print("[UDP] Received full data.")
                break

        except socket.timeout:
            if usb_done_event.is_set():
                if start_wait is None:
                    start_wait = time.time()
                elif time.time() - start_wait > timeout_after_usb:
                    print("[UDP] Timed out waiting for remaining data.")
                    break

    sock.close()

    output_path = "received_image.jpg"
    with open(output_path, 'wb') as f:
        f.write(received_data)
    print(f"[UDP] Image saved to {output_path} ({len(received_data)} bytes received)")

def send_usb(image_data: bytes, usb_done_event):
    with serial.Serial(PORT, BAUD, timeout=2) as ser:
        print(f"[USB] Connected to {PORT}")
        ser.write(len(image_data).to_bytes(4, 'big'))

        ser.write(image_data)

    usb_done_event.set()  # Notify UDP thread

if __name__ == "__main__":
    image_path = filedialog.askopenfilename(title="Select image")
    if not image_path:
        print("No image selected.")
        exit()

    with open(image_path, 'rb') as f:
        image_data = f.read()
    print(f"Loaded {len(image_data)} bytes from {image_path}")

    usb_done_event = threading.Event()

    # Start UDP receiving
    udp_thread = threading.Thread(target=receive_udp, args=(usb_done_event,), daemon=True)
    udp_thread.start()

    # Send USB data
    send_usb(image_data, usb_done_event)

    # Wait for UDP thread
    udp_thread.join()
    print("Done.")
