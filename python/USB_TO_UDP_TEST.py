import threading
import socket
import serial
import time
from tkinter import filedialog

# ===== Configuration =====
MODE = "usb2udp"  # Change to "udp2usb" for the reverse direction
PORT = 'COM11'
BAUD = 115200
UDP_LOOPBACK_PORT = 4321


# ===== usb2udp logic (image from laptop to Nucleo) =====
def receive_udp(usb_done_event):
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind(('10.42.0.1', UDP_LOOPBACK_PORT))
    sock.settimeout(1.0)
    print(f"[UDP] Listening on port {UDP_LOOPBACK_PORT}...")

    total_received = 0
    expected_size = None
    received_data = bytearray()
    timeout_after_usb = 2.0

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


# ===== udp2usb logic (image from UDP to laptop over USB) =====
def receive_usb(usb_done_event, expected_size):
    with serial.Serial(PORT, BAUD, timeout=2) as ser:
        print(f"[USB] Waiting for {expected_size} bytes over USB...")
        
        received = bytearray()
        start = time.time()
        while len(received) < expected_size and (time.time() - start) < 10:
            to_read = min(64, expected_size - len(received))
            chunk = ser.read(to_read)
            if chunk:
                received.extend(chunk)
                print(f"[USB] Received {len(received)}/{expected_size} bytes")

        with open("usb_received_image.jpg", "wb") as f:
            f.write(received)

        if len(received) == expected_size:
            print("[USB] Image saved as 'usb_received_image.jpg' (complete)")
        else:
            print(f"[USB] Incomplete image saved (got {len(received)} / {expected_size})")

    usb_done_event.set()



def send_udp(image_data: bytes):
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    remote = ('10.42.0.61', UDP_LOOPBACK_PORT)

    sock.sendto(len(image_data).to_bytes(4, 'big') + image_data[:508], remote)
    for i in range(508, len(image_data), 512):
        chunk = image_data[i:i+512]
        sock.sendto(chunk, remote)
        time.sleep(0.001)

    sock.close()
    print("[UDP] Sent image to Nucleo over UDP.")


# ===== Main Logic =====
if __name__ == "__main__":
    usb_done_event = threading.Event()

    if MODE == "usb2udp":
        image_path = filedialog.askopenfilename(title="Select image")
        if not image_path:
            print("No image selected.")
            exit()

        with open(image_path, 'rb') as f:
            image_data = f.read()
        print(f"Loaded {len(image_data)} bytes from {image_path}")

        udp_thread = threading.Thread(target=receive_udp, args=(usb_done_event,), daemon=True)
        udp_thread.start()

        send_usb(image_data, usb_done_event)
        udp_thread.join()

    elif MODE == "udp2usb":
        image_path = filedialog.askopenfilename(title="Select image to send over UDP")
        if not image_path:
            print("No image selected.")
            exit()

        with open(image_path, 'rb') as f:
            image_data = f.read()
        print(f"Loaded {len(image_data)} bytes from {image_path}")

        usb_thread = threading.Thread(target=receive_usb, args=(usb_done_event, len(image_data)), daemon=True)
        usb_thread.start()

        send_udp(image_data)

        usb_thread.join()

    else:
        print("Invalid MODE. Use 'usb2udp' or 'udp2usb'.")

    print("Done.")
