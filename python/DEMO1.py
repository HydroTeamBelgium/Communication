import os
import threading
import serial
import time
import platform
import subprocess

# ======================================
# CONFIGURATION
# ======================================
PORT = 'COM11'
BAUD = 12000000

USB_RECEIVE_FILENAME = "usb_received_image.jpg"
CHUNK_SIZE = 512
ABSOLUTE_TIMEOUT = 30.0


# ======================================
# UTILS
# ======================================
def log(msg):
    print(f"[LOG] {msg}")


def save_file(path, data: bytes):
    with open(path, 'wb') as f:
        f.write(data)
    log(f"Saved file: {path} ({len(data)} bytes)")
    open_file(path)


def open_file(path):
    try:
        if platform.system() == "Windows":
            os.startfile(path)
        elif platform.system() == "Darwin":
            subprocess.run(["open", path])
        else:
            subprocess.run(["xdg-open", path])
        log(f"Opened file: {path}")
    except Exception as e:
        log(f"Could not open file: {e}")


# ======================================
# USB HANDLING
# ======================================
def receive_usb(done_event):
    with serial.Serial(PORT, BAUD, timeout=0.0001) as ser:
        log("USB receiving started...")
        received = bytearray()
        chunk_size = 64

        start_time = time.time()
        last_data_time = start_time
        consecutive_empty_reads = 0
        max_empty_reads = 500

        while True:
            chunk = ser.read(chunk_size)
            current_time = time.time()

            if chunk:
                received.extend(chunk)
                last_data_time = current_time
                consecutive_empty_reads = 0
                ser.timeout = 0.000001

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
    try:
        with serial.Serial(PORT, BAUD, timeout=0) as ser:
            log(f"Sending {len(image_data)} bytes over USB")
            for i in range(0, len(image_data), CHUNK_SIZE):
                ser.write(image_data[i:i + CHUNK_SIZE])
                if (i // CHUNK_SIZE) % 100 == 0:
                    log(f"USB sent {i + CHUNK_SIZE} bytes")
            ser.flush()
            log("USB sending complete")
    except Exception as e:
        log(f"USB send error: {e}")
    finally:
        done_event.set()


# ======================================
# MAIN ENTRY POINT
# ======================================
def main():
    MODE = "2"  # Default to send mode
    
    if MODE == '1':
        image_path = os.path.join(os.path.dirname(__file__), 'test_image.jpg')
        if not os.path.isfile(image_path):
            log("Image file required for 'send' mode and must exist.")
            return

        with open(image_path, 'rb') as f:
            image_data = f.read()

        log(f"Loaded image ({len(image_data)} bytes)")
        input("Press Enter to send once the receiver is ready...")

        done_event = threading.Event()
        send_usb(image_data, done_event)
        done_event.wait()
        log("Send complete.")

    elif MODE == '2':
        done_event = threading.Event()
        log("Receiver is ready. Waiting for USB data...")
        receive_usb(done_event)
        done_event.wait()
        log("Receive complete.")



if __name__ == "__main__":
    main()
