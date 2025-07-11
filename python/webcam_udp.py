# demo that streams the webcam over udp, works with main_copy.rs
# have some patience, takes time to start this code
# Normally, when connected to a switch, this should be able to send from one laptop to the nucleo, with a target IP (other laptop) and the nucleo should be able to parse this to the other laptop
import cv2
import socket
import threading
import time
import struct
import numpy as np
from collections import defaultdict, deque
from datetime import datetime

class WebcamUDPStreamer:
    def __init__(self, stm32_ip="10.42.0.61", stm32_port=4321, 
                 local_ip="10.42.0.1", local_port=4321,
                 resolution=(640, 480), target_fps=15, jpeg_quality=80,
                 chunk_size=800, send_delay=0.001, max_retries=3, timeout=2.0):
        # Network configuration
        self.stm32_ip = stm32_ip
        self.stm32_port = stm32_port
        self.local_ip = local_ip
        self.local_port = local_port
        
        # Video settings
        self.resolution = resolution
        self.target_fps = target_fps
        self.jpeg_quality = jpeg_quality
        
        # Streaming parameters
        self.chunk_size = chunk_size
        self.send_delay = send_delay
        self.max_retries = max_retries
        self.timeout = timeout
        
        # State variables
        self.running = False
        self.cap = None
        self.udp_socket = None
        self.received_chunks = {}
        self.expected_chunks = 0
        self.stop_event = threading.Event()
        
        # Metrics
        self.metrics = {
            'start_time': None,
            'end_time': None,
            'frames_sent': 0,
            'frames_received': 0,
            'total_bytes_sent': 0,
            'total_bytes_received': 0,
            'packets_sent': 0,
            'packets_received': 0,
            'retries_per_chunk': defaultdict(int),
            'failed_chunks': set(),
            'duplicate_chunks': defaultdict(int),
            'out_of_order_chunks': 0,
            'last_received_chunk_id': -1,
            'send_times': {},
            'receive_times': {},
            'round_trip_times': [],
            'frame_timestamps': deque(maxlen=100),
            'frame_processing_times': deque(maxlen=100),
            'throughput_history': deque(maxlen=100)
        }

    def connect(self):
        """Initialize webcam and UDP socket"""
        try:
            # Initialize webcam
            self.cap = cv2.VideoCapture(0)
            if not self.cap.isOpened():
                raise Exception("Could not open webcam")
            
            self.cap.set(cv2.CAP_PROP_FRAME_WIDTH, self.resolution[0])
            self.cap.set(cv2.CAP_PROP_FRAME_HEIGHT, self.resolution[1])
            self.cap.set(cv2.CAP_PROP_FPS, self.target_fps)
            
            # Initialize UDP socket
            self.udp_socket = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            self.udp_socket.bind((self.local_ip, self.local_port))
            self.udp_socket.settimeout(self.timeout)
            
            print(f"Webcam initialized at {self.resolution[0]}x{self.resolution[1]} @ {self.target_fps}fps")
            print("press q to end stream and see final metrics")
            print(f"Streaming to {self.stm32_ip}:{self.stm32_port}")
            print(f"Listening on {self.local_ip}:{self.local_port}")
            
            self.metrics['start_time'] = time.time()
            return True
            
        except Exception as e:
            print(f"Initialization failed: {e}")
            return False

    def capture_and_stream(self):
        """Capture frames and stream them to STM32"""
        while self.running:
            start_time = time.time()
            
            # Capture frame
            ret, frame = self.cap.read()
            if not ret:
                continue
                
            # Encode as JPEG
            _, encoded_frame = cv2.imencode('.jpg', frame, 
                                          [cv2.IMWRITE_JPEG_QUALITY, self.jpeg_quality])
            frame_data = encoded_frame.tobytes()
            
            # Reset frame tracking
            self.received_chunks = {}
            self.expected_chunks = 0
            
            # Send frame in chunks
            chunk_id = 0
            total_size = len(frame_data)
            sent_bytes = 0
            
            while sent_bytes < total_size:
                chunk = frame_data[sent_bytes:sent_bytes + self.chunk_size - 10]
                if not chunk:
                    break

                # Packet structure: header(6) + chunk_id(4) + data
                ip_bytes = socket.inet_aton(self.local_ip)
                port_bytes = struct.pack("!H", self.local_port)
                id_bytes = struct.pack("!I", chunk_id)
                packet = ip_bytes + port_bytes + id_bytes + chunk
                
                # Send with retries
                send_success = False
                for retry in range(self.max_retries):
                    try:
                        send_time = time.time()
                        self.udp_socket.sendto(packet, (self.stm32_ip, self.stm32_port))
                        
                        # Update metrics
                        self.metrics['packets_sent'] += 1
                        self.metrics['total_bytes_sent'] += len(packet)
                        self.metrics['send_times'][chunk_id] = send_time
                        if retry > 0:
                            self.metrics['retries_per_chunk'][chunk_id] = retry
                        
                        sent_bytes += len(chunk)
                        send_success = True
                        break
                    except Exception as e:
                        print(f"Send error on chunk {chunk_id}, retry {retry + 1}: {e}")
                        if retry == self.max_retries - 1:
                            print(f"Failed to send chunk {chunk_id} after {self.max_retries} retries")
                            self.metrics['failed_chunks'].add(chunk_id)
                        time.sleep(0.1)
                
                chunk_id += 1
                time.sleep(self.send_delay)
            
            # Send end marker
            ip_bytes = socket.inet_aton(self.local_ip)
            port_bytes = struct.pack("!H", self.local_port)
            id_bytes = struct.pack("!I", chunk_id)
            end_packet = ip_bytes + port_bytes + id_bytes
            self.udp_socket.sendto(end_packet, (self.stm32_ip, self.stm32_port))
            self.metrics['packets_sent'] += 1
            self.metrics['total_bytes_sent'] += len(end_packet)
            self.metrics['send_times'][chunk_id] = time.time()
            
            self.expected_chunks = chunk_id
            self.metrics['frames_sent'] += 1
            self.metrics['frame_timestamps'].append(time.time())
            
            # Calculate processing time
            processing_time = time.time() - start_time
            self.metrics['frame_processing_times'].append(processing_time)
            
            # Adaptive delay to maintain target FPS
            target_frame_time = 1.0 / self.target_fps
            sleep_time = max(0, target_frame_time - processing_time)
            time.sleep(sleep_time)

    def receive_frames(self):
        """Receive and reconstruct frames from STM32"""
        while not self.stop_event.is_set():
            try:
                data, addr = self.udp_socket.recvfrom(2048)
                receive_time = time.time()
                
                if len(data) < 4:
                    continue  # Invalid packet

                chunk_id = struct.unpack("!I", data[:4])[0]
                payload = data[4:]

                # Check for duplicates
                if chunk_id in self.received_chunks:
                    self.metrics['duplicate_chunks'][chunk_id] += 1
                    continue
                
                # Check for out-of-order delivery
                if chunk_id < self.metrics['last_received_chunk_id']:
                    self.metrics['out_of_order_chunks'] += 1
                else:
                    self.metrics['last_received_chunk_id'] = chunk_id
                
                self.received_chunks[chunk_id] = payload
                self.metrics['packets_received'] += 1
                self.metrics['total_bytes_received'] += len(data)
                self.metrics['receive_times'][chunk_id] = receive_time
                
                # Calculate RTT if we have send time
                if chunk_id in self.metrics['send_times']:
                    rtt = receive_time - self.metrics['send_times'][chunk_id]
                    self.metrics['round_trip_times'].append(rtt)
                
                # Check if this is the end marker (empty payload)
                if len(payload) == 0:
                    self.reconstruct_and_display_frame()
                    
            except socket.timeout:
                continue
            except Exception as e:
                print(f"Receive error: {e}")

    def reconstruct_and_display_frame(self):
        """Reconstruct frame from chunks and display with metrics"""
        # Wait for all chunks or timeout
        max_wait = 1.0 / self.target_fps  # Don't wait longer than one frame period
        start_time = time.time()
        
        while time.time() - start_time < max_wait:
            if len(self.received_chunks) >= self.expected_chunks:
                break
            time.sleep(0.001)
        
        # Check for missing chunks
        missing_chunks = []
        for i in range(self.expected_chunks):
            if i not in self.received_chunks:
                missing_chunks.append(i)
        
        # Reconstruct frame even if some chunks are missing
        frame_data = bytearray()
        for chunk_id in sorted(self.received_chunks.keys()):
            if chunk_id < self.expected_chunks:  # Skip end marker
                frame_data.extend(self.received_chunks[chunk_id])
        
        # Decode and display frame
        if frame_data:
            try:
                frame = cv2.imdecode(np.frombuffer(frame_data, dtype=np.uint8), cv2.IMREAD_COLOR)
                if frame is not None:
                    self.metrics['frames_received'] += 1
                    self.display_frame_with_metrics(frame)
            except Exception as e:
                print(f"Frame decoding error: {e}")

    def display_frame_with_metrics(self, frame):
        """Display frame with overlay of streaming metrics"""
        # Calculate statistics
        current_time = time.time()
        elapsed = current_time - self.metrics['start_time']
        
        # Calculate FPS (overall and recent)
        overall_fps = self.metrics['frames_sent'] / elapsed if elapsed > 0 else 0
        recent_fps = 0
        if len(self.metrics['frame_timestamps']) > 1:
            recent_fps = len(self.metrics['frame_timestamps']) / (
                self.metrics['frame_timestamps'][-1] - self.metrics['frame_timestamps'][0])
        
        # Calculate packet loss
        packet_loss = 0
        if self.metrics['packets_sent'] > 0:
            packet_loss = (self.metrics['packets_sent'] - self.metrics['packets_received']) / self.metrics['packets_sent'] * 100
        
        # Calculate transfer rates (in Mbps)
        send_rate = (self.metrics['total_bytes_sent'] * 8) / (elapsed * 1_000_000) if elapsed > 0 else 0
        receive_rate = (self.metrics['total_bytes_received'] * 8) / (elapsed * 1_000_000) if elapsed > 0 else 0
        
        # Store current throughput for recent average
        self.metrics['throughput_history'].append(receive_rate)
        recent_throughput = sum(self.metrics['throughput_history']) / len(self.metrics['throughput_history']) if self.metrics['throughput_history'] else 0

        # Calculate average processing time
        avg_processing = 0
        if self.metrics['frame_processing_times']:
            avg_processing = sum(self.metrics['frame_processing_times']) / len(self.metrics['frame_processing_times'])
        
        # Calculate RTT stats
        avg_rtt = 0
        if self.metrics['round_trip_times']:
            avg_rtt = sum(self.metrics['round_trip_times']) / len(self.metrics['round_trip_times']) * 1000
        
        # Display statistics
        stats = [
            f"FPS: {recent_fps:.1f}/{overall_fps:.1f} (target: {self.target_fps})",
            f"Throughput: {recent_throughput:.2f}/{receive_rate:.2f} Mbps (send: {send_rate:.2f})",
            f"Packets: {self.metrics['packets_sent']} sent, {self.metrics['packets_received']} received",
            f"Packet loss: {packet_loss:.1f}%",
            f"Frames: {self.metrics['frames_sent']} sent, {self.metrics['frames_received']} received",
            f"Processing: {avg_processing*1000:.1f}ms avg",
            f"RTT: {avg_rtt:.1f}ms avg",
            f"Resolution: {self.resolution[0]}x{self.resolution[1]}",
            datetime.now().strftime("%H:%M:%S")
        ]
        
        for i, text in enumerate(stats):
            cv2.putText(frame, text, (10, 30 + i*25), 
                       cv2.FONT_HERSHEY_SIMPLEX, 0.6, (0, 255, 0), 2)
        
        cv2.imshow('Webcam Stream', frame)
        if cv2.waitKey(1) & 0xFF == ord('q'):
            self.running = False

    def print_final_metrics(self):
        """Print summary metrics when streaming stops"""
        elapsed = time.time() - self.metrics['start_time']
        # Calculate transfer rates (in Mbps)
        send_rate = (self.metrics['total_bytes_sent'] * 8) / (elapsed * 1_000_000) if elapsed > 0 else 0
        receive_rate = (self.metrics['total_bytes_received'] * 8) / (elapsed * 1_000_000) if elapsed > 0 else 0
        
        print("\n" + "="*60)
        print("FINAL STREAMING METRICS")
        print("="*60)
        print(f"Duration:             {elapsed:.2f} seconds")
        print(f"Frames sent:          {self.metrics['frames_sent']}")
        print(f"Frames received:      {self.metrics['frames_received']}")
        print(f"Frame rate:           {self.metrics['frames_sent']/elapsed:.1f} FPS")
        print(f"Send rate:            {send_rate:.2f} Mbps")
        print(f"Receive rate:         {receive_rate:.2f} Mbps")
        print(f"Bytes sent:           {self.metrics['total_bytes_sent']:,}")
        print(f"Bytes received:       {self.metrics['total_bytes_received']:,}")
        print(f"Packets sent:         {self.metrics['packets_sent']:,}")
        print(f"Packets received:     {self.metrics['packets_received']:,}")
        print(f"Packet loss rate:     {(self.metrics['packets_sent'] - self.metrics['packets_received'])/self.metrics['packets_sent']*100:.1f}%")
        print(f"Duplicate packets:    {sum(self.metrics['duplicate_chunks'].values())}")
        print(f"Out-of-order packets: {self.metrics['out_of_order_chunks']}")
        print(f"Failed chunks:        {len(self.metrics['failed_chunks'])}")
        
        if self.metrics['round_trip_times']:
            avg_rtt = sum(self.metrics['round_trip_times']) / len(self.metrics['round_trip_times']) * 1000
            min_rtt = min(self.metrics['round_trip_times']) * 1000
            max_rtt = max(self.metrics['round_trip_times']) * 1000
            print(f"Average RTT:          {avg_rtt:.1f} ms")
            print(f"Min RTT:              {min_rtt:.1f} ms")
            print(f"Max RTT:              {max_rtt:.1f} ms")
        
        print("="*60)

    def start(self):
        """Start the streaming application"""
        if not self.connect():
            return
            
        self.running = True
        self.stop_event.clear()
        
        # Start threads
        send_thread = threading.Thread(target=self.capture_and_stream)
        receive_thread = threading.Thread(target=self.receive_frames)
        
        send_thread.start()
        receive_thread.start()
        
        try:
            while self.running:
                time.sleep(0.1)
                if cv2.waitKey(1) & 0xFF == ord('q'):
                    self.running = False
        except KeyboardInterrupt:
            print("\nShutting down...")
        finally:
            self.running = False
            self.stop_event.set()
            send_thread.join()
            receive_thread.join()
            
            if self.udp_socket:
                self.udp_socket.close()
            if self.cap:
                self.cap.release()
            cv2.destroyAllWindows()
            
            self.print_final_metrics()

if __name__ == "__main__":
    streamer = WebcamUDPStreamer(
        stm32_ip="10.42.0.61",
        stm32_port=4321,
        local_ip="10.42.0.1",
        local_port=4321,
        resolution=(640, 480),
        target_fps=25,
        jpeg_quality=80,
        chunk_size=800,
        send_delay=0.001
    )
    streamer.start()