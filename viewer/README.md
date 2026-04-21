# rotation_viewer

Dual-display image sequence viewer driven by a UDP rotation sensor.

## How it works

1. Listens for UDP packets on port **5005** (from `192.168.2.1`)
2. Each packet is a plain-text angle in degrees, e.g. `"183.5"`
3. Maps `0–360°` → `0–TOTAL_FRAMES` and picks the matching frame
4. Displays the frame from **sequence1/** on Window 1 and **sequence2/** on Window 2

## Configuration (top of `src/main.rs`)

| Constant | Default | Description |
|---|---|---|
| `LISTEN_ADDR` | `0.0.0.0:5005` | UDP port to listen on |
| `SENDER_IP` | `192.168.2.1` | Only accept packets from this IP |
| `IMAGE_SEQUENCE_FOLDER_1` | `./sequence1` | Folder for display 1 |
| `IMAGE_SEQUENCE_FOLDER_2` | `./sequence2` | Folder for display 2 |
| `TOTAL_FRAMES` | `60` | Number of frames to map 0–360° onto |
| `WINDOW_WIDTH` | `1280` | Display width (px) |
| `WINDOW_HEIGHT` | `720` | Display height (px) |

## Folder structure

```
rotation_viewer/
├── Cargo.toml
├── src/main.rs
├── sequence1/
│   ├── frame_001.png
│   ├── frame_002.png
│   └── ...
└── sequence2/
    ├── frame_001.png
    └── ...
```

Images are loaded in **alphabetical/lexicographic order** by filename, so
zero-pad your frame numbers (`frame_001`, `frame_002`, … `frame_060`).

Supported formats: `.png`, `.jpg`, `.jpeg`

## Build & run

```bash
cargo build --release
./target/release/rotation_viewer
```

## Pi sender (Python example)

```python
import socket, time

sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
PC_IP = "192.168.2.x"   # your PC's IP
PORT  = 5005

while True:
    angle = read_sensor()          # returns 0.0 – 360.0
    sock.sendto(f"{angle:.2f}".encode(), (PC_IP, PORT))
    time.sleep(0.016)              # ~60 Hz
```

## Press ESC in either window to quit.
