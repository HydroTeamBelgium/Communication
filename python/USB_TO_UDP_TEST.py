import os
import threading
import socket
import serial
import time
import platform
import subprocess

# ======================================
# CONFIGURATION
# ======================================
MODE = "udp2usb"  # or "usb2udp"
PORT = 'COM11'
BAUD = 12000000
UDP_IP = '10.42.0.61'
UDP_LOCAL_IP = '10.42.0.1'
UDP_PORT = 4321

USB_RECEIVE_FILENAME = "usb_received_image.jpg"
UDP_RECEIVE_FILENAME = "udp_received_image.jpg"

CHUNK_SIZE = 512  # Common chunk size for both UDP and USB
ABSOLUTE_TIMEOUT = 30.0


# Python struct format: '<' (little-endian), 'I' (uint32), 'H' (uint16)
HEADER_FORMAT = '<4sIHHI'  # Magic (4B) + FrameNum (4B) + Width (2B) + Height (2B) + Size (4B)
HEADER_SIZE = 16  # bytes
FOOTER_MAGIC = b'END!'


# ======================================
# UTILS
# ======================================
def log(msg):
    print(f"[LOG] {msg}")

def save_file(path, data: bytes):
    with open(path, 'wb') as f:
        f.write(data)
    log(f"Saved file: {path} ({len(data)} bytes)")
    open_file(path)  # ← This will open it automatically

def open_file(path):
    """Opens a file with the default image viewer."""
    try:
        if platform.system() == "Windows":
            os.startfile(path)
        elif platform.system() == "Darwin":  # macOS
            subprocess.run(["open", path])
        else:  # Linux and others
            subprocess.run(["xdg-open", path])
        log(f"Opened file: {path}")
    except Exception as e:
        log(f"Could not open file: {e}")

def pack_frame_header(frame_num: int, width: int, height: int, payload_size: int) -> bytes:
    import struct
    return struct.pack(HEADER_FORMAT, b'IMGF', frame_num, width, height, payload_size)

def send_image_with_metadata(image_data: bytes, frame_num=0, mode="udp"):
    # Example: Assume image is 640x480 JPEG
    width, height = 640, 480
    header = pack_frame_header(frame_num, width, height, len(image_data))
    footer = FOOTER_MAGIC  # Optional
    
    # Combine header + image + footer
    full_frame = header + image_data + footer
    
    if mode == "udp":
        send_udp(full_frame)  # Reuse your existing function
    else:
        send_usb(full_frame)


# ======================================
# USB ↔ File
# ======================================
def receive_usb(done_event):
    """Improved USB reception with adaptive timeout and better buffering"""
    with serial.Serial(PORT, BAUD, timeout=0.0001) as ser:
        log("USB receiving started...")
        received = bytearray()
        chunk_size = 64  # USB endpoint size

        start_time = time.time()
        last_data_time = start_time
        consecutive_empty_reads = 0
        max_empty_reads = 50

        while True:
            chunk = ser.read(chunk_size)
            current_time = time.time()

            if chunk:
                received.extend(chunk)
                last_data_time = current_time
                consecutive_empty_reads = 0
                ser.timeout = 0.000001  # Aggressively flush

                while True:
                    flush_chunk = ser.read(chunk_size)
                    if not flush_chunk:
                        break
                    received.extend(flush_chunk)

                ser.timeout = 0.0001

            else:
                consecutive_empty_reads += 1
                if consecutive_empty_reads >= max_empty_reads:
                    log(f"USB timeout after {consecutive_empty_reads} empty reads")
                    break
                if current_time - last_data_time > 1.0 and len(received) > 0:
                    log("USB idle timeout reached")
                    break
                if current_time - start_time > ABSOLUTE_TIMEOUT:
                    log("USB absolute timeout reached")
                    break
                time.sleep(0.00001)

        if received:
            save_file(USB_RECEIVE_FILENAME, received)
        else:
            log("No USB data received")

    done_event.set()

def send_usb(image_data: bytes, done_event):
    """Sends image data over USB serial."""
    try:
        with serial.Serial(PORT, BAUD, timeout=0) as ser:
            log(f"Sending {len(image_data)} bytes over USB")
            for i in range(0, len(image_data), CHUNK_SIZE):
                ser.write(image_data[i:i+CHUNK_SIZE])
                if (i // CHUNK_SIZE) % 100 == 0:
                    log(f"USB sent {i+CHUNK_SIZE} bytes")
            ser.flush()
            log("USB sending complete")
    except Exception as e:
        log(f"USB send error: {e}")
    finally:
        done_event.set()

# ======================================
# UDP ↔ File
# ======================================
def receive_udp(done_event):
    """Receives UDP packets and writes to a file."""
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind((UDP_LOCAL_IP, UDP_PORT))
    sock.settimeout(0.0005)

    received = bytearray()
    start_time = time.time()
    last_data_time = start_time
    consecutive_timeouts = 0
    max_consecutive_timeouts = 500

    log(f"Listening for UDP on {UDP_LOCAL_IP}:{UDP_PORT}")

    while True:
        try:
            data, _ = sock.recvfrom(8192)
            received.extend(data)
            last_data_time = time.time()
            consecutive_timeouts = 0

        except socket.timeout:
            consecutive_timeouts += 1
            now = time.time()
            if done_event.is_set() and now - last_data_time > 0.2:
                log("UDP sender done, exiting receiver")
                break
            if consecutive_timeouts > max_consecutive_timeouts:
                log("UDP timed out from inactivity")
                break
            if now - start_time > ABSOLUTE_TIMEOUT:
                log("UDP absolute timeout")
                break
            time.sleep(0.00001)

    sock.close()
    if received:
        save_file(UDP_RECEIVE_FILENAME, received)
    else:
        log("No UDP data received")

def send_udp(image_data: bytes):
    """Improved UDP sending with adaptive pacing."""
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.setsockopt(socket.SOL_SOCKET, socket.SO_SNDBUF, 65536)

    remote = (UDP_IP, UDP_PORT)
    total_chunks = (len(image_data) + CHUNK_SIZE - 1) // CHUNK_SIZE
    log(f"Sending {len(image_data)} bytes via UDP in {total_chunks} chunks")

    for i in range(0, len(image_data), CHUNK_SIZE):
        chunk = image_data[i:i+CHUNK_SIZE]
        chunk_num = i // CHUNK_SIZE + 1
        try:
            sock.sendto(chunk, remote)
            if chunk_num <= 10:
                time.sleep(0.001)
            elif chunk_num <= 100:
                time.sleep(0.0005)
            else:
                time.sleep(0.0001)
            if chunk_num % 100 == 0 or chunk_num == total_chunks:
                log(f"Sent chunk {chunk_num}/{total_chunks}")
        except Exception as e:
            log(f"UDP send error @ chunk {chunk_num}: {e}")
            time.sleep(0.01)

    sock.close()
    log("UDP sending complete")


# ======================================
# MAIN
# ======================================
def main():
    usb_done_event = threading.Event()

    # Change this to your fixed image path
    image_path = os.path.join(os.path.dirname(__file__), 'test_image.jpg')
    MODE = input("Enter mode (1 / 2) for USB2UDP or UDP2USB: ")

    if not os.path.isfile(image_path):
        log(f"Image not found at: {image_path}")
        return

    with open(image_path, 'rb') as f:
        image_data = f.read()
    log(f"Loaded image ({len(image_data)} bytes)")

    if MODE == "1":
        udp_thread = threading.Thread(target=receive_udp, args=(usb_done_event,))
        udp_thread.start()

        send_usb(image_data, usb_done_event)
        udp_thread.join()

    elif MODE == "2":
        usb_thread = threading.Thread(target=receive_usb, args=(usb_done_event,))
        usb_thread.start()

        time.sleep(0.1)  # Let USB read get ready
        send_udp(image_data)
        usb_thread.join()

    else:
        log("Invalid MODE. Choose '1' or '2'.")

    log("Transfer complete.")


if __name__ == "__main__":
    main()
