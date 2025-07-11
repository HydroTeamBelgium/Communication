# Demo for ethernet connection between nucleo and laptop. It sends an image and reconstructs it with metrics
import socket
import threading
import struct
import os
import subprocess
import platform
import time
from collections import defaultdict


STM32_IP = "10.42.0.61"
STM32_PORT = 4321

# Where you expect echo (e.g., send it to STM32 and STM32 bounces it back to us)
DEST_IP = "10.42.0.1"
DEST_PORT = 4321

IMAGE_PATH = "test_image.jpg"
OUTPUT_PATH = "received.jpg"
CHUNK_SIZE = 800  # Reduced to leave more room for network headers
SEND_DELAY = 0.001  # 1ms delay between packets
MAX_RETRIES = 3
TIMEOUT = 2.0

received_chunks = {}
expected_chunks = 0
lock = threading.Lock()
stop_event = threading.Event()

# Metrics
class TransferMetrics:
    def __init__(self):
        self.start_time = None
        self.end_time = None
        self.total_bytes_sent = 0
        self.total_bytes_received = 0
        self.packets_sent = 0
        self.packets_received = 0
        self.retries_per_chunk = defaultdict(int)
        self.send_times = {}  # chunk_id -> send_time
        self.receive_times = {}  # chunk_id -> receive_time
        self.round_trip_times = []
        self.failed_chunks = set()
        self.duplicate_chunks = defaultdict(int)
        self.out_of_order_chunks = 0
        self.last_received_chunk_id = -1
        
    def get_stats(self):
        duration = (self.end_time or time.time()) - (self.start_time or time.time())
        total_retries = sum(self.retries_per_chunk.values())
        
        # Calculate RTT stats
        if self.round_trip_times:
            avg_rtt = sum(self.round_trip_times) / len(self.round_trip_times)
            min_rtt = min(self.round_trip_times)
            max_rtt = max(self.round_trip_times)
        else:
            avg_rtt = min_rtt = max_rtt = 0
        
        # Calculate throughput
        throughput_mbps = (self.total_bytes_received * 8) / (duration * 1_000_000) if duration > 0 else 0
        
        return {
            'duration': duration,
            'total_bytes_sent': self.total_bytes_sent,
            'total_bytes_received': self.total_bytes_received,
            'packets_sent': self.packets_sent,
            'packets_received': self.packets_received,
            'packet_loss_rate': (self.packets_sent - self.packets_received) / self.packets_sent * 100 if self.packets_sent > 0 else 0,
            'total_retries': total_retries,
            'failed_chunks': len(self.failed_chunks),
            'duplicate_chunks': sum(self.duplicate_chunks.values()),
            'out_of_order_chunks': self.out_of_order_chunks,
            'avg_rtt_ms': avg_rtt * 1000,
            'min_rtt_ms': min_rtt * 1000,
            'max_rtt_ms': max_rtt * 1000,
            'throughput_mbps': throughput_mbps,
            'efficiency': (self.packets_received / self.packets_sent * 100) if self.packets_sent > 0 else 0
        }

metrics = TransferMetrics()

def open_file(path):
    """Opens a file with the default image viewer."""
    try:
        if platform.system() == "Windows":
            os.startfile(path)
        elif platform.system() == "Darwin":  # macOS
            subprocess.run(["open", path])
        else:  # Linux and others
            subprocess.run(["xdg-open", path])
        print(f"Opened file: {path}")
    except Exception as e:
        print(f"Could not open file: {e}")

def send_image():
    global expected_chunks
    
    metrics.start_time = time.time()
    
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
        with open(IMAGE_PATH, "rb") as f:
            chunk_id = 0
            total_size = os.path.getsize(IMAGE_PATH)
            sent_bytes = 0
            
            while True:
                chunk = f.read(CHUNK_SIZE - 10)  # 6 bytes header + 4 bytes for chunk_id
                if not chunk:
                    break

                # Packet = header(6) + chunk_id(4) + data
                ip_bytes = socket.inet_aton(DEST_IP)
                port_bytes = struct.pack("!H", DEST_PORT)
                id_bytes = struct.pack("!I", chunk_id)

                packet = ip_bytes + port_bytes + id_bytes + chunk
                
                send_success = False
                for retry in range(MAX_RETRIES):
                    try:
                        send_time = time.time()
                        sock.sendto(packet, (STM32_IP, STM32_PORT))
                        
                        # Record metrics
                        metrics.packets_sent += 1
                        metrics.total_bytes_sent += len(packet)
                        metrics.send_times[chunk_id] = send_time
                        if retry > 0:
                            metrics.retries_per_chunk[chunk_id] = retry
                        
                        sent_bytes += len(chunk)
                        # progress = (sent_bytes / total_size) * 100
                        # retry_info = f" (retry {retry + 1})" if retry > 0 else ""
                        # print(f"Sent chunk {chunk_id}, {len(chunk)} bytes ({progress:.1f}%){retry_info}")
                        send_success = True
                        break
                    except Exception as e:
                        print(f"Send error on chunk {chunk_id}, retry {retry + 1}: {e}")
                        if retry == MAX_RETRIES - 1:
                            print(f"Failed to send chunk {chunk_id} after {MAX_RETRIES} retries")
                            metrics.failed_chunks.add(chunk_id)
                        time.sleep(0.1)
                
                chunk_id += 1
                
                # Add delay to prevent overwhelming the STM32
                time.sleep(SEND_DELAY)
            
            # Send end marker
            ip_bytes = socket.inet_aton(DEST_IP)
            port_bytes = struct.pack("!H", DEST_PORT)
            id_bytes = struct.pack("!I", chunk_id)
            end_packet = ip_bytes + port_bytes + id_bytes  # Empty payload
            send_time = time.time()
            sock.sendto(end_packet, (STM32_IP, STM32_PORT))
            metrics.packets_sent += 1
            metrics.total_bytes_sent += len(end_packet)
            metrics.send_times[chunk_id] = send_time
            
            expected_chunks = chunk_id
            # print(f"Sent {chunk_id} chunks total")

def receive_response():
    with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as sock:
        sock.bind(("", DEST_PORT))
        sock.settimeout(TIMEOUT)
        print(f"Listening on UDP port {DEST_PORT} for responses...")

        while not stop_event.is_set():
            try:
                data, addr = sock.recvfrom(2048)
                receive_time = time.time()
                
                if len(data) < 4:
                    continue  # Invalid

                chunk_id = struct.unpack("!I", data[:4])[0]
                payload = data[4:]

                with lock:
                    # Check for duplicates
                    if chunk_id in received_chunks:
                        metrics.duplicate_chunks[chunk_id] += 1
                        continue
                    
                    # Check for out-of-order delivery
                    if chunk_id < metrics.last_received_chunk_id:
                        metrics.out_of_order_chunks += 1
                    else:
                        metrics.last_received_chunk_id = chunk_id
                    
                    received_chunks[chunk_id] = payload
                    metrics.packets_received += 1
                    metrics.total_bytes_received += len(data)
                    metrics.receive_times[chunk_id] = receive_time
                    
                    # Calculate RTT if we have send time
                    if chunk_id in metrics.send_times:
                        rtt = receive_time - metrics.send_times[chunk_id]
                        metrics.round_trip_times.append(rtt)
                    
                    # print(f"Received chunk {chunk_id}, {len(payload)} bytes")

                # Check if this is the end marker (empty payload)
                if len(payload) == 0:
                    # print("Received end marker")
                    metrics.end_time = time.time()
                    stop_event.set()
                    break
                    
            except socket.timeout:
                continue
            except Exception as e:
                print(f"Receive error: {e}")

def reconstruct_image():
    print(f"Waiting for all chunks... Expected: {expected_chunks}")
    
    # Wait for all chunks or timeout
    max_wait = 10.0  # 10 seconds max wait
    start_time = time.time()
    
    while time.time() - start_time < max_wait:
        with lock:
            if len(received_chunks) >= expected_chunks:
                break
        time.sleep(0.1)
    
    if not metrics.end_time:
        metrics.end_time = time.time()
    
    with lock:
        # Check for missing chunks
        missing_chunks = []
        for i in range(expected_chunks):
            if i not in received_chunks:
                missing_chunks.append(i)
        
        if missing_chunks:
            print(f"Missing chunks: {missing_chunks}")
            print(f"Received {len(received_chunks)}/{expected_chunks} chunks")
        else:
            print(f"All {expected_chunks} chunks received successfully!")
        
        # Reconstruct even if some chunks are missing
        with open(OUTPUT_PATH, "wb") as out_file:
            for chunk_id in sorted(received_chunks.keys()):
                if chunk_id < expected_chunks:  # Don't write the end marker
                    out_file.write(received_chunks[chunk_id])
                
        print(f"Image reconstructed to {OUTPUT_PATH}")
        
        # Print detailed metrics
        print_transfer_metrics()
        
        if not missing_chunks:
            open_file(OUTPUT_PATH)

def print_transfer_metrics():
    stats = metrics.get_stats()
    
    print("\n" + "="*60)
    print("TRANSFER METRICS")
    print("="*60)
    print(f"Duration:              {stats['duration']:.2f} seconds")
    print(f"Total bytes sent:      {stats['total_bytes_sent']:,} bytes")
    print(f"Total bytes received:  {stats['total_bytes_received']:,} bytes")
    print(f"Packets sent:          {stats['packets_sent']:,}")
    print(f"Packets received:      {stats['packets_received']:,}")
    print(f"Packet loss rate:      {stats['packet_loss_rate']:.2f}%")
    print(f"Transfer efficiency:   {stats['efficiency']:.2f}%")
    print(f"Throughput:            {stats['throughput_mbps']:.2f} Mbps")
    print(f"Total retries:         {stats['total_retries']:,}")
    print(f"Failed chunks:         {stats['failed_chunks']:,}")
    print(f"Duplicate chunks:      {stats['duplicate_chunks']:,}")
    print(f"Out-of-order chunks:   {stats['out_of_order_chunks']:,}")
    
    if stats['avg_rtt_ms'] > 0:
        print(f"Average RTT:           {stats['avg_rtt_ms']:.2f} ms")
        print(f"Min RTT:               {stats['min_rtt_ms']:.2f} ms")
        print(f"Max RTT:               {stats['max_rtt_ms']:.2f} ms")
    
    print("="*60)

if __name__ == "__main__":
    if not os.path.exists(IMAGE_PATH):
        print(f"Image file not found: {IMAGE_PATH}")
        exit(1)

    print(f"Starting file transfer of {IMAGE_PATH}")
    print(f"File size: {os.path.getsize(IMAGE_PATH)} bytes")
    
    t_recv = threading.Thread(target=receive_response, daemon=True)
    t_recv.start()

    # Small delay to ensure receiver is ready
    time.sleep(0.1)
    
    send_image()

    # Wait for completion
    stop_event.wait(timeout=15.0)
    reconstruct_image()
    
    print("Transfer complete")