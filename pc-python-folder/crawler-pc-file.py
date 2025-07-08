import serial
import time

# Adjust the port to match your Mac's serial device
SERIAL_PORT = '/dev/tty.usbmodem123456781'  # Change this to your actual port
BAUDRATE = 115200  #

CMD_LED_ON = b'\x01'
CMD_LED_OFF = b'\x02'

def main():
    with serial.Serial(SERIAL_PORT, BAUDRATE, timeout=1) as ser:
        print("Connected to", SERIAL_PORT)
        
        while True:
            cmd = input("Enter 'on' or 'off' (or 'q' to quit): ").strip().lower()
            if cmd == 'on':
                ser.write(CMD_LED_ON)
                print("Sent LED ON")
            elif cmd == 'off':
                ser.write(CMD_LED_OFF)
                print("Sent LED OFF")
            elif cmd == 'q':
                print("Exiting.")
                break
            else:
                print("Unknown command.")

if __name__ == '__main__':
    main()
