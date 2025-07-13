# PC Communication Tool

This folder contains the **crawler-pc-file.py** script for communicating with the embedded device over USB serial.  
It lets you send simple on/off commands (or other messages you define) to your embedded project.

---

## 📦 Requirements

- Python 3.6 or higher
- Virtual environment (recommended)
- [pyserial](https://pyserial.readthedocs.io/en/latest/) (required)

---

## ⚡ Quick Start Guide

### 1️⃣ Clone the repository (if not done yet)

```bash
git clone <your-repo-url>
cd <your-repo-root>/pc-python-folder
```

---

## 2️⃣ Create a Virtual Environment

Go inside `/Communication/pc-python-folder/`:

#### ✅ macOS / Linux

```bash
python3 -m venv venv
source venv/bin/activate
```

#### ✅ Windows

```powershell
python -m venv venv
.\venv\Scripts\Activate
```

---

## 3️⃣ Install Dependencies

With the virtual environment active:

```bash
pip install -r requirements.txt
```

This installs **pyserial**, which is required for serial communication.

**Example requirements.txt:**

```
pyserial>=3.5,<4
```

---

## 4️⃣ Find the Correct Serial Port

In your `crawler-pc-file.py`:

```python
SERIAL_PORT = 'your-actual-port'
```

#### ✅ macOS

```bash
ls /dev/tty.*
```

Example:

```
/dev/tty.usbmodem123456781
```

#### ✅ Linux

```bash
ls /dev/ttyUSB*
```
or
```bash
ls /dev/ttyACM*
```

Example:

```
/dev/ttyUSB0
```

#### ✅ Windows

1. Open **Device Manager**
2. Expand **Ports (COM & LPT)**
3. Note the COM port (e.g. `COM3`)

Use in your script:

```python
SERIAL_PORT = 'COM3'
```

### 4️⃣ Running python file
1. Activate virtual environment
2. python3 pythonfile.py
