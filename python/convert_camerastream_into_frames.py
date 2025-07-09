import cv2
import time

# Open the default camera (usually index 0)
cap = cv2.VideoCapture(0)

# Check if camera is opened successfully
if not cap.isOpened():
    print("Error: Could not open camera.")
    exit()

print("Camera opened successfully.")

# Target frame rate (e.g., 4 FPS)
fps = 4
interval = 1 / fps  # Time between frames

frame_count = 0
last_capture_time = time.time()

try:
    while True:
        ret, frame = cap.read()

        if not ret:
            print("Warning: Failed to capture frame.")
            continue

        # Show the frame in a window
        cv2.imshow("Camera Feed", frame)

        # Check if it's time to save a frame
        current_time = time.time()
        if current_time - last_capture_time >= interval:
            timestamp = int(current_time * 1000)
            filename = f"frame_{timestamp}.jpg"
            cv2.imwrite(filename, frame)
            frame_count += 1
            print(f"Frame {frame_count} captured and saved as {filename}")
            last_capture_time = current_time

        # Exit when ESC is pressed
        if cv2.waitKey(1) & 0xFF == 27:
            print("\n ESC pressed. Exiting...")
            break

except KeyboardInterrupt:
    print("\n Interrupted by user. Exiting...")

finally:
    cap.release()
    cv2.destroyAllWindows()
    print("Camera released. All windows closed.")
