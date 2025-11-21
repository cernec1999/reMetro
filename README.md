# reMetro

Reverse engineering and emulating the retro Passenger Information Management System (PIMS) found in the Washington D.C. WMATA Metro stations.

## Project Overview

**reMetro** recreates the iconic LED display signs found in WMATA Metro stations - those distinctive amber/orange displays showing train arrivals, destinations, and car counts. This project provides a complete pipeline from font extraction to physical hardware reproduction.

## Architecture

The project consists of four main components:

### 1. metro-font-builder (Python)
Computer vision tool that extracts the custom pixel font from photographs of actual Metro displays.

**Features:**
- Perspective correction for accurate sampling
- Grid overlay for precise character extraction
- Exports to u8g2 embedded graphics format
- JSON output with pixel coordinates

**Usage:**
```bash
cd metro-font-builder
python3 -m venv venv
source venv/bin/activate
pip install -r requirements.txt
python src/metro-font-builder.py
```

### 2. metro-render (Rust)
Core rendering library (no_std) that draws Metro-style train prediction displays. This shared library works identically on desktop simulators and embedded hardware.

**Features:**
- Classic 4-row layout: header + 3 train predictions
- Columns: LN (line), CAR (cars), DEST (destination), MIN (minutes)
- Color-coded text (red headers, yellow/green data)
- Uses extracted Metro custom font
- Platform-agnostic (runs on embedded and desktop)

### 3. metro-simulator (Rust)
Desktop preview tool for rapid development and testing.

**Features:**
- SDL2-based window showing the display
- Uses same rendering code as hardware
- Instant feedback for font and layout changes

**Usage:**
```bash
cd metro-simulator
cargo run
```

### 4. metro-esp32 (Rust)
Embedded firmware for ESP32-S3 driving real HUB75 LED matrix panels.

**Hardware:**
- ESP32-S3 microcontroller
- HUB75 RGB LED matrix (32 rows × 128 columns, 4-bit color)
- 5V power supply

**Features:**
- WiFi and Bluetooth Low Energy ready
- Embassy async runtime
- DMA-accelerated display updates
- Flicker-free rendering

**Current Status:**
- ✅ Static train predictions rendering
- ⏳ Live WMATA API integration (planned)
- ⏳ Dynamic updates (planned)
- ⏳ BLE configuration (planned)

## Development Workflow

1. **Extract Font** - Photograph real Metro sign → Run metro-font-builder → Generate .u8g2font file
2. **Develop Rendering** - Write code in metro-render → Test in metro-simulator
3. **Deploy to Hardware** - Flash metro-esp32 → Display on physical LED matrix

## Technical Details

**Language:** Rust (embedded components) + Python (font extraction)

**Key Technologies:**
- Embassy async runtime for ESP32
- HUB75 LED matrix protocol
- u8g2 embedded graphics library
- embedded-graphics ecosystem

**Build System:** Cargo workspace with shared dependencies

**Code Reuse:** The `metro-render` library is shared between simulator and hardware via path dependencies, ensuring identical behavior across platforms.

## Building

### Prerequisites
- Rust toolchain (latest stable)
- Python 3.8+ (for font builder)
- SDL2 development libraries (for simulator)
- ESP32 toolchain (for hardware, see metro-esp32 for details)

### Build Simulator
```bash
cargo build --package metro-simulator
cargo run --package metro-simulator
```

### Build ESP32 Firmware
```bash
cd metro-esp32
cargo build --release
```

## Current Status

**Working:**
- ✅ Font extraction from photos
- ✅ Custom Metro font rendering
- ✅ Desktop simulator with accurate display
- ✅ ESP32 firmware driving HUB75 LED panels
- ✅ Static train prediction display

**In Progress:**
- ⏳ WMATA real-time API integration
- ⏳ WiFi connectivity for live data
- ⏳ Dynamic train arrival updates
- ⏳ BLE configuration interface

## License

See [LICENSE](LICENSE) file for details.

## Acknowledgments

This project is a fan recreation and is not affiliated with WMATA or any Metro transit authority.
