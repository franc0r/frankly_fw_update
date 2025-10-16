# CAN Device Simulator

This Python script simulates one or more CAN devices that respond to Frankly Bootloader protocol messages. It's useful for testing the `frankly-fw-update-cli` tool without requiring physical hardware.

## Features

- **Multi-device support**: Simulate multiple devices in a single process
- Responds to Ping/Search requests and device initialization
- Returns simulated device information (VID, PID, PRD, UID, bootloader version, flash layout)
- Supports custom node IDs with unique UIDs per device
- Works with both virtual CAN (vcan) and physical CAN interfaces
- Complete flash information support for device initialization

## Requirements

Install the python-can library:

```bash
pip install python-can
```

## Setup Virtual CAN Interface

For testing without physical CAN hardware, set up a virtual CAN interface:

```bash
# Create virtual CAN interface
sudo ip link add dev vcan0 type vcan

# Bring it up
sudo ip link set vcan0 up

# Verify it's running
ip link show vcan0
```

To make it persistent across reboots, add to `/etc/network/interfaces`:

```
auto vcan0
iface vcan0 inet manual
    pre-up ip link add dev $IFACE type vcan
    up ip link set up $IFACE
```

## Usage

### Basic Usage

Run the simulator with default settings (vcan0, node ID 1):

```bash
python3 can_device_simulator.py
```

### Single Device

Simulate a device with node ID 5:

```bash
python3 can_device_simulator.py --interface vcan0 --node-id 5
```

### Multiple Devices (Recommended)

Simulate multiple devices in a single process - this is the easiest way to test with multiple devices:

```bash
# Simulate 4 devices with node IDs 1, 3, 5, and 8 (matches default SIM_NODE_LST)
python3 can_device_simulator.py --interface vcan0 --node-ids 1 3 5 8

# Simulate 2 devices
python3 can_device_simulator.py --interface vcan0 --node-ids 10 20

# Simulate many devices
python3 can_device_simulator.py --interface vcan0 --node-ids 1 2 3 4 5 6 7 8 9 10
```

**Benefits of multi-device mode:**
- Single process, single terminal window
- Each device has a unique UID based on its node ID
- All devices respond to broadcast messages independently
- Easier to manage than multiple processes

### Alternative: Multiple Processes

You can also run multiple simulator instances in separate terminals (less convenient):

```bash
# Terminal 1
python3 can_device_simulator.py --interface vcan0 --node-id 1

# Terminal 2
python3 can_device_simulator.py --interface vcan0 --node-id 3
```

### Testing with frankly-fw-update-cli

With the simulator running, test the CLI tool:

```bash
# Search for devices
cargo run -- search --type can --interface vcan0

# Expected output with multiple devices (node IDs 1, 3, 5, 8):
# Device found[  1]: VID: 0x00000042 | PID: 0x00001337 | PRD: 0x20250101 | UID: 0x44444445333333342222222311111112
# Device found[  3]: VID: 0x00000042 | PID: 0x00001337 | PRD: 0x20250101 | UID: 0x44444447333333362222222511111114
# Device found[  5]: VID: 0x00000042 | PID: 0x00001337 | PRD: 0x20250101 | UID: 0x44444449333333382222222711111116
# Device found[  8]: VID: 0x00000042 | PID: 0x00001337 | PRD: 0x20250101 | UID: 0x4444444C3333333B2222222A11111119
```

## Simulator Output

When you start the simulator with multiple devices, you'll see:

```
CAN Device Simulator
  Interface: vcan0
  Number of devices: 4
  RX CAN ID: 0x780 (listening to broadcast)

Device Node ID: 1
  TX CAN ID: 0x783
  Device Info:
    VID: 0x00000042
    PID: 0x00001337
    PRD: 0x20250101
    UID: 0x44444445333333342222222311111112
  Flash Layout:
    Total: 64 KB (64 pages × 1024 bytes)
    Bootloader: 8 KB | Application: 56 KB

Device Node ID: 3
  TX CAN ID: 0x787
  Device Info:
    VID: 0x00000042
    PID: 0x00001337
    PRD: 0x20250101
    UID: 0x44444447333333362222222511111114
  Flash Layout:
    Total: 64 KB (64 pages × 1024 bytes)
    Bootloader: 8 KB | Application: 56 KB

...

Listening for messages on CAN ID 0x780
Press Ctrl+C to stop

[RX] Node 1: PING (0x0001), Packet ID: 0
[TX] Node 1: Result=OK, Data=0x00010203
[RX] Node 3: PING (0x0001), Packet ID: 0
[TX] Node 3: Result=OK, Data=0x00010203
...
```

## Simulated Device Information

The simulator returns the following default values for each device:

| Field | Value | Description |
|-------|-------|-------------|
| Bootloader Version | `0x00010203` | Version 1.2.3 |
| Bootloader CRC | `0xDEADBEEF` | CRC checksum |
| VID (Vendor ID) | `0x00000042` | Device vendor identifier |
| PID (Product ID) | `0x00001337` | Device product identifier |
| PRD (Production Date) | `0x20250101` | Production date (2025-01-01) |
| UID (128-bit) | `0x44444444+N...` | Unique device ID (varies by node ID) |
| Flash Start Address | `0x08000000` | STM32-style flash start |
| Flash Page Size | `0x00000400` | 1024 bytes (1 KB) |
| Flash Total Pages | `0x00000040` | 64 pages (64 KB total) |
| App Start Page | `0x00000008` | Page 8 (bootloader uses 8 KB) |
| App CRC | `0x00000000` | Application CRC (0 = empty) |

**Note:** Each device's UID is automatically incremented based on its node ID, making devices easily distinguishable.

You can modify these values in the `SimulatedDevice.create()` method in the script.

## CAN Protocol Details

### CAN IDs

The protocol uses the following CAN ID scheme:

- **Broadcast ID**: `0x780` - Used by host to send commands to all devices
- **Device TX ID**: `0x781 + (node_id * 2) + 1` - Used by device to respond

For example:
- Node ID 1: Responds on `0x783`
- Node ID 3: Responds on `0x787`
- Node ID 5: Responds on `0x78C`
- Node ID 8: Responds on `0x791`

### Message Format

All messages are 8 bytes:

```
Byte 0-1: Request Type (u16, little endian)
Byte 2:   Result Type (u8)
Byte 3:   Packet ID (u8)
Byte 4-7: Data (u32, little endian)
```

### Supported Request Types

The simulator supports all device initialization requests:

**Device Information:**
- `0x0001` - Ping
- `0x0101` - DevInfoBootloaderVersion
- `0x0102` - DevInfoBootloaderCRC
- `0x0103` - DevInfoVID (Vendor ID)
- `0x0104` - DevInfoPID (Product ID)
- `0x0105` - DevInfoPRD (Production Date)
- `0x0106` - DevInfoUID1 (UID bits 0-31)
- `0x0107` - DevInfoUID2 (UID bits 32-63)
- `0x0108` - DevInfoUID3 (UID bits 64-95)
- `0x0109` - DevInfoUID4 (UID bits 96-127)

**Flash Information:**
- `0x0201` - FlashInfoStartAddr
- `0x0202` - FlashInfoPageSize
- `0x0203` - FlashInfoNumPages

**Application Information:**
- `0x0301` - AppInfoPageIdx
- `0x0302` - AppInfoCRCCalc

Other request types will return `ERR_NOT_SUPPORTED`.

## Monitoring CAN Traffic

You can monitor CAN traffic using `candump`:

```bash
# Install can-utils if not already installed
sudo apt-get install can-utils

# Monitor all traffic on vcan0
candump vcan0

# Monitor with timestamps and ASCII interpretation
candump -ta -c vcan0
```

Example output during a search with multiple devices:
```
  vcan0  780   [8]  01 00 00 00 00 00 00 00    # Host sends Ping (broadcast)
  vcan0  783   [8]  01 00 01 00 03 02 01 00    # Device 1 responds with version 1.2.3
  vcan0  787   [8]  01 00 01 00 03 02 01 00    # Device 3 responds with version 1.2.3
  vcan0  78C   [8]  01 00 01 00 03 02 01 00    # Device 5 responds with version 1.2.3
  vcan0  791   [8]  01 00 01 00 03 02 01 00    # Device 8 responds with version 1.2.3
```

## Troubleshooting

### Permission Denied

If you get a permission error:

```bash
# Add your user to the relevant group (may need to log out/in)
sudo usermod -a -G dialout $USER

# Or run with sudo (not recommended for regular use)
sudo python3 can_device_simulator.py
```

### Interface Not Found

If the CAN interface doesn't exist:

```bash
# Check available interfaces
ip link show

# Recreate vcan0
sudo ip link del vcan0  # if it exists but isn't working
sudo ip link add dev vcan0 type vcan
sudo ip link set vcan0 up
```

### No Response from Simulator

1. Check that the simulator is running
2. Verify the interface name matches (e.g., `vcan0`)
3. Check the node IDs match what you're searching for
4. Monitor traffic with `candump vcan0` to see if messages are being sent
5. Check for error messages in the simulator output

### Duplicate Node IDs Error

If you see "Error: Duplicate node IDs are not allowed":
- Make sure each node ID is unique in the `--node-ids` list
- Example: `--node-ids 1 1 3` is invalid (1 appears twice)

## Command Line Options

```
usage: can_device_simulator.py [-h] [--interface INTERFACE]
                                [--node-id NODE_ID] [--node-ids NODE_IDS [NODE_IDS ...]]

Simulate one or more CAN devices for Frankly Bootloader

optional arguments:
  -h, --help            show this help message and exit
  --interface INTERFACE
                        CAN interface name (default: vcan0)
  --node-id NODE_ID     Single device node ID, 0-255 (deprecated: use --node-ids)
  --node-ids NODE_IDS [NODE_IDS ...]
                        Multiple device node IDs, 0-255 (e.g., --node-ids 1 3 5 8)
```

**Note:** `--node-ids` is the recommended way to specify devices. `--node-id` is kept for backward compatibility.

## Extending the Simulator

### Adding New Request Types

To add support for more request types:

1. Add the request type to the `RequestType` enum:
```python
class RequestType(IntEnum):
    # ... existing types ...
    NEW_REQUEST = 0x0ABC
```

2. Add handling in the `handle_message()` method:
```python
elif msg.request == RequestType.NEW_REQUEST:
    response.data = info['new_value']
```

3. Add the value to the device info in `SimulatedDevice.create()`:
```python
device_info = {
    # ... existing values ...
    'new_value': 0x12345678,
}
```

### Customizing Device Information

To change device information, edit the `SimulatedDevice.create()` method:

```python
device_info = {
    'bootloader_version': 0x00020004,  # Change to v2.0.4
    'vid': 0x00000100,                 # Change vendor ID
    'flash_num_pages': 0x00000080,     # Change to 128 pages (128 KB)
    # ... etc
}
```

## Related Files

- `can_device_simulator.py` - Main simulator script
- `CAN_SIMULATOR_README.md` - This documentation

## Tips

1. **Start with 4 devices** matching the default CLI config: `--node-ids 1 3 5 8`
2. **Use candump** in another terminal to monitor traffic while learning the protocol
3. **Unique UIDs** help distinguish devices - the simulator automatically generates these
4. **Test error cases** by temporarily removing request type handling to see how the CLI handles errors
5. **Multiple terminals?** Use `tmux` or `screen` to manage simulator + CLI + candump in one window
