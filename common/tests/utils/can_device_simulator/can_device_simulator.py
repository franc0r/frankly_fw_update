#!/usr/bin/env python3
"""
CAN Device Simulator for Frankly Bootloader

This script simulates one or more CAN devices that respond to search/ping messages
from the frankly-fw-update-cli tool.

Usage:
    # Single device
    python3 can_device_simulator.py --interface vcan0 --node-id 5

    # Multiple devices
    python3 can_device_simulator.py --interface vcan0 --node-ids 1 3 5 8

Requirements:
    pip install python-can
"""

import argparse
import signal
import sys
import time
import can
from dataclasses import dataclass
from enum import IntEnum
from typing import List, Dict


# CAN Protocol Constants
CAN_BASE_ID = 0x781
CAN_BROADCAST_ID = 0x780


class RequestType(IntEnum):
    """Frankly Bootloader Request Types"""
    PING = 0x0001
    RESET_DEVICE = 0x0011
    START_APP = 0x0012
    DEV_INFO_BOOTLOADER_VERSION = 0x0101
    DEV_INFO_BOOTLOADER_CRC = 0x0102
    DEV_INFO_VID = 0x0103
    DEV_INFO_PID = 0x0104
    DEV_INFO_PRD = 0x0105
    DEV_INFO_UID1 = 0x0106
    DEV_INFO_UID2 = 0x0107
    DEV_INFO_UID3 = 0x0108
    DEV_INFO_UID4 = 0x0109
    FLASH_INFO_START_ADDR = 0x0201
    FLASH_INFO_PAGE_SIZE = 0x0202
    FLASH_INFO_NUM_PAGES = 0x0203
    APP_INFO_PAGE_IDX = 0x0301
    APP_INFO_CRC_CALC = 0x0302


class ResultType(IntEnum):
    """Frankly Bootloader Result Types"""
    NONE = 0x00
    OK = 0x01
    ERROR = 0xFE
    ERR_UNKNOWN_REQ = 0xFD
    ERR_NOT_SUPPORTED = 0xFC
    ERR_CRC_INVLD = 0xFB
    ERR_PAGE_FULL = 0xFA
    ERR_INVLD_ARG = 0xF9


@dataclass
class FranklyMessage:
    """Represents a Frankly Bootloader message (8 bytes)"""
    request: int  # 2 bytes
    result: int   # 1 byte
    packet_id: int  # 1 byte
    data: int  # 4 bytes (u32)

    def to_bytes(self) -> bytes:
        """Convert message to 8-byte array"""
        msg = bytearray(8)
        # Request type (little endian, 2 bytes)
        msg[0] = self.request & 0xFF
        msg[1] = (self.request >> 8) & 0xFF
        # Result type (1 byte)
        msg[2] = self.result & 0xFF
        # Packet ID (1 byte)
        msg[3] = self.packet_id & 0xFF
        # Data (little endian, 4 bytes)
        msg[4] = (self.data >> 0) & 0xFF
        msg[5] = (self.data >> 8) & 0xFF
        msg[6] = (self.data >> 16) & 0xFF
        msg[7] = (self.data >> 24) & 0xFF
        return bytes(msg)

    @classmethod
    def from_bytes(cls, data: bytes) -> 'FranklyMessage':
        """Parse message from 8-byte array"""
        if len(data) != 8:
            raise ValueError(f"Invalid message length: {len(data)}, expected 8")

        request = data[0] | (data[1] << 8)
        result = data[2]
        packet_id = data[3]
        msg_data = data[4] | (data[5] << 8) | (data[6] << 16) | (data[7] << 24)

        return cls(request=request, result=result, packet_id=packet_id, data=msg_data)


@dataclass
class SimulatedDevice:
    """Configuration for a single simulated device"""
    node_id: int
    tx_can_id: int
    device_info: Dict[str, int]

    @classmethod
    def create(cls, node_id: int) -> 'SimulatedDevice':
        """Create a device with default configuration"""
        tx_can_id = CAN_BASE_ID + (node_id * 2) + 1

        # Generate unique UIDs based on node ID
        device_info = {
            'bootloader_version': 0x00010203,  # v1.2.3
            'bootloader_crc': 0xDEADBEEF,
            'vid': 0x00000042,  # Vendor ID
            'pid': 0x00001337,  # Product ID
            'prd': 0x20250101,  # Production date (2025-01-01)
            'uid1': 0x11111111 + node_id,  # 128-bit UID split into 4x32-bit (unique per node)
            'uid2': 0x22222222 + node_id,
            'uid3': 0x33333333 + node_id,
            'uid4': 0x44444444 + node_id,
            'flash_start_addr': 0x08000000,  # STM32 typical flash start
            'flash_page_size': 0x00000400,   # 1KB pages
            'flash_num_pages': 0x00000040,   # 64 pages = 64KB total
            'app_page_idx': 0x00000008,      # App starts at page 8 (bootloader uses 8KB)
            'app_crc_calc': 0x00000000,      # Calculated CRC of app area
        }

        return cls(node_id=node_id, tx_can_id=tx_can_id, device_info=device_info)


class CANDeviceSimulator:
    """Simulates one or more CAN devices for Frankly Bootloader"""

    def __init__(self, interface: str, node_ids: List[int]):
        """
        Initialize the simulator

        Args:
            interface: CAN interface name (e.g., 'vcan0', 'can0')
            node_ids: List of device node IDs (0-255)
        """
        self.interface = interface
        self.bus = None
        self.running = False

        # Create simulated devices
        self.devices: Dict[int, SimulatedDevice] = {}
        for node_id in node_ids:
            self.devices[node_id] = SimulatedDevice.create(node_id)

        # RX ID: CAN_BROADCAST_ID (all devices listen to broadcast)
        self.rx_can_id = CAN_BROADCAST_ID

        self._print_device_info()

    def _print_device_info(self):
        """Print information about all simulated devices"""
        print(f"CAN Device Simulator")
        print(f"  Interface: {self.interface}")
        print(f"  Number of devices: {len(self.devices)}")
        print(f"  RX CAN ID: 0x{self.rx_can_id:03X} (listening to broadcast)")
        print()

        for node_id, device in sorted(self.devices.items()):
            info = device.device_info
            flash_total_kb = (info['flash_page_size'] * info['flash_num_pages']) // 1024
            bootloader_kb = (info['flash_page_size'] * info['app_page_idx']) // 1024
            app_kb = flash_total_kb - bootloader_kb

            print(f"Device Node ID: {node_id}")
            print(f"  TX CAN ID: 0x{device.tx_can_id:03X}")
            print(f"  Device Info:")
            print(f"    VID: 0x{info['vid']:08X}")
            print(f"    PID: 0x{info['pid']:08X}")
            print(f"    PRD: 0x{info['prd']:08X}")
            print(f"    UID: 0x{info['uid4']:08X}{info['uid3']:08X}{info['uid2']:08X}{info['uid1']:08X}")
            print(f"  Flash Layout:")
            print(f"    Total: {flash_total_kb} KB ({info['flash_num_pages']} pages × {info['flash_page_size']} bytes)")
            print(f"    Bootloader: {bootloader_kb} KB | Application: {app_kb} KB")
            print()

    def start(self):
        """Start the simulator"""
        try:
            # Create CAN bus connection
            self.bus = can.interface.Bus(channel=self.interface, bustype='socketcan')
            print(f"Connected to {self.interface}")

            # Set up CAN filters to only receive broadcast messages
            filters = [{"can_id": self.rx_can_id, "can_mask": 0x7FF, "extended": False}]
            self.bus.set_filters(filters)
            print(f"Listening for messages on CAN ID 0x{self.rx_can_id:03X}")
            print("Press Ctrl+C to stop\n")

            self.running = True
            self.run_loop()

        except OSError as e:
            print(f"Error: Could not open CAN interface '{self.interface}'")
            print(f"       {e}")
            print(f"\nTip: Make sure the interface exists and you have permissions.")
            print(f"     For virtual CAN: sudo ip link add dev vcan0 type vcan && sudo ip link set vcan0 up")
            sys.exit(1)
        except Exception as e:
            print(f"Error: {e}")
            sys.exit(1)

    def stop(self):
        """Stop the simulator"""
        self.running = False
        if self.bus:
            self.bus.shutdown()
            print("\nSimulator stopped")

    def handle_message(self, msg: FranklyMessage, device: SimulatedDevice) -> FranklyMessage:
        """
        Process a received message and generate response for a specific device

        Args:
            msg: Received Frankly message
            device: The simulated device to respond

        Returns:
            Response message
        """
        request_name = self._get_request_name(msg.request)
        print(f"[RX] Node {device.node_id}: {request_name} (0x{msg.request:04X}), Packet ID: {msg.packet_id}")

        # Default response: echo request, set result to OK
        response = FranklyMessage(
            request=msg.request,
            result=ResultType.OK,
            packet_id=msg.packet_id,
            data=0
        )

        info = device.device_info

        # Handle different request types
        if msg.request == RequestType.PING:
            response.data = info['bootloader_version']
        elif msg.request == RequestType.DEV_INFO_BOOTLOADER_VERSION:
            response.data = info['bootloader_version']
        elif msg.request == RequestType.DEV_INFO_BOOTLOADER_CRC:
            response.data = info['bootloader_crc']
        elif msg.request == RequestType.DEV_INFO_VID:
            response.data = info['vid']
        elif msg.request == RequestType.DEV_INFO_PID:
            response.data = info['pid']
        elif msg.request == RequestType.DEV_INFO_PRD:
            response.data = info['prd']
        elif msg.request == RequestType.DEV_INFO_UID1:
            response.data = info['uid1']
        elif msg.request == RequestType.DEV_INFO_UID2:
            response.data = info['uid2']
        elif msg.request == RequestType.DEV_INFO_UID3:
            response.data = info['uid3']
        elif msg.request == RequestType.DEV_INFO_UID4:
            response.data = info['uid4']
        elif msg.request == RequestType.FLASH_INFO_START_ADDR:
            response.data = info['flash_start_addr']
        elif msg.request == RequestType.FLASH_INFO_PAGE_SIZE:
            response.data = info['flash_page_size']
        elif msg.request == RequestType.FLASH_INFO_NUM_PAGES:
            response.data = info['flash_num_pages']
        elif msg.request == RequestType.APP_INFO_PAGE_IDX:
            response.data = info['app_page_idx']
        elif msg.request == RequestType.APP_INFO_CRC_CALC:
            response.data = info['app_crc_calc']
        else:
            # Unsupported request
            response.result = ResultType.ERR_NOT_SUPPORTED
            print(f"[  ] Node {device.node_id}: Unsupported request type: 0x{msg.request:04X}")

        return response

    def run_loop(self):
        """Main message processing loop"""
        while self.running:
            try:
                # Wait for CAN message (1 second timeout)
                can_msg = self.bus.recv(timeout=1.0)

                if can_msg is None:
                    continue

                # Verify this is a broadcast message
                if can_msg.arbitration_id != self.rx_can_id:
                    continue

                # Parse Frankly message
                try:
                    frankly_msg = FranklyMessage.from_bytes(can_msg.data)
                except ValueError as e:
                    print(f"[!!] Invalid message: {e}")
                    continue

                # Each device responds to the broadcast message
                for node_id, device in self.devices.items():
                    # Generate response for this device
                    response = self.handle_message(frankly_msg, device)

                    # Send response on device's TX ID
                    response_can = can.Message(
                        arbitration_id=device.tx_can_id,
                        data=response.to_bytes(),
                        is_extended_id=False
                    )

                    self.bus.send(response_can)
                    print(f"[TX] Node {node_id}: Result={ResultType(response.result).name}, Data=0x{response.data:08X}")

                print()  # Empty line after all devices respond

            except KeyboardInterrupt:
                break
            except Exception as e:
                print(f"Error in message loop: {e}")
                time.sleep(0.1)

    @staticmethod
    def _get_request_name(request_type: int) -> str:
        """Get human-readable name for request type"""
        try:
            return RequestType(request_type).name
        except ValueError:
            return f"UNKNOWN(0x{request_type:04X})"


def main():
    parser = argparse.ArgumentParser(
        description='Simulate one or more CAN devices for Frankly Bootloader',
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Single device
  python3 can_device_simulator.py --interface vcan0 --node-id 5

  # Multiple devices (recommended - simpler)
  python3 can_device_simulator.py --interface vcan0 --node-ids 1 3 5 8

  # Setup virtual CAN interface first
  sudo ip link add dev vcan0 type vcan
  sudo ip link set vcan0 up

  # Test with the CLI tool
  cargo run -- search --type can --interface vcan0
        """
    )

    parser.add_argument(
        '--interface',
        type=str,
        default='vcan0',
        help='CAN interface name (default: vcan0)'
    )

    parser.add_argument(
        '--node-id',
        type=int,
        help='Single device node ID, 0-255 (deprecated: use --node-ids)'
    )

    parser.add_argument(
        '--node-ids',
        type=int,
        nargs='+',
        help='Multiple device node IDs, 0-255 (e.g., --node-ids 1 3 5 8)'
    )

    args = parser.parse_args()

    # Determine which node IDs to use
    node_ids = []
    if args.node_ids:
        node_ids = args.node_ids
    elif args.node_id is not None:
        node_ids = [args.node_id]
    else:
        # Default to node ID 1 if nothing specified
        node_ids = [1]

    # Validate node IDs
    for node_id in node_ids:
        if not 0 <= node_id <= 255:
            print(f"Error: node-id {node_id} must be between 0 and 255")
            sys.exit(1)

    # Check for duplicates
    if len(node_ids) != len(set(node_ids)):
        print("Error: Duplicate node IDs are not allowed")
        sys.exit(1)

    # Create and start simulator
    simulator = CANDeviceSimulator(args.interface, node_ids)

    # Set up signal handler for clean shutdown
    def signal_handler(sig, frame):
        simulator.stop()
        sys.exit(0)

    signal.signal(signal.SIGINT, signal_handler)
    signal.signal(signal.SIGTERM, signal_handler)

    # Start the simulator
    simulator.start()


if __name__ == '__main__':
    main()
