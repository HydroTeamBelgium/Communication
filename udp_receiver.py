# testcode for communication between laptop and STM32, For this to work, you need to have your wifi turned of, and change your ip Adress
# works together wit STP_to_laptop_test
import socket
import struct
import time

# Configure UDP socket
UDP_IP = "10.42.0.100"  # Your laptop's IP
UDP_PORT = 4321         # Must match PORT in STM32 code

def udp_receiver():
    
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.bind(('0.0.0.0', UDP_PORT))
    
    print(f"Listening for UDP packets on {UDP_IP}:{UDP_PORT}")
    
    try:
        while True:
            data, addr = sock.recvfrom(1024)
            counter = struct.unpack('<i', data)[0]  # Little-endian 32-bit integer
            print(f"Received counter value: {counter} from {addr[0]}")
    except KeyboardInterrupt:
        print("\nStopping receiver...")
    finally:
        sock.close()


def udp_sender():
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    counter = 0
    
    try:
        while True:
            data = struct.pack('<i', counter)  
            # Send to STM32's IP and port
            sock.sendto(data, ('10.42.0.61', 4321))
            print(f"Sent: {counter} to 10.42.0.61:4321")
            counter += 1
            time.sleep(1)
    except KeyboardInterrupt:
        print("\nStopping sender...")
    finally:
        sock.close()

if __name__ == "__main__":
    udp_sender()