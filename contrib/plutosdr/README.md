# TETRA Bluestation with ADALM-Pluto SDR

A complete guide for operating a TETRA BTS (Base Transceiver Station) with
[tetra-bluestation](https://github.com/MidnightBlueLabs/tetra-bluestation) on an
[ADALM-Pluto SDR](https://www.analog.com/en/resources/evaluation-hardware-and-software/evaluation-boards-kits/adalm-pluto.html).

> **Note:** This setup is intended for amateur radio operation in the 70 cm band
> (430–440 MHz) with the appropriate licence. Transmitting on TETRA frequencies
> without authorisation is illegal.

## Overview

```
┌──────────────┐     USB      ┌──────────────────┐
│  ADALM-Pluto │◄────────────►│   Linux PC       │
│  (Custom FW) │  IIO Control │                  │
│              │  + USB Gadget │  tetra-bluestation│
│  FPGA with   │   Streaming  │  (Rust)          │
│  Timestamps  │              │       │          │
└──────────────┘              │  rust-soapysdr   │
                              │       │          │
                              │  SoapyPlutoSDR   │
                              │  (modified C++)  │
                              └──────────────────┘
```

The ADALM-Pluto runs a custom firmware with FPGA-based sample timestamping. The
host PC runs tetra-bluestation, which communicates via a modified SoapyPlutoSDR
driver.

## Tested Configuration

| Component | Version |
|-----------|---------|
| OS | Debian 12/13 (x86_64) |
| Rust | 1.94+ |
| ADALM-Pluto FW | pgreenland v0.38_with_timestamping |
| SoapySDR | 0.8.x |
| libiio | 0.24+ |
| libad9361 | 0.3+ |
| libusb | 1.0.x |

## Hardware Requirements

- **ADALM-Pluto SDR** (Rev. B or Rev. C)
- **Linux PC** with a USB port (min. 4 GB RAM for compilation)
- **Antenna** or dummy load on the TX port
- Optional: External 10/20 MHz reference oscillator (GPS-disciplined) for
  better frequency accuracy

## Software Dependencies

```bash
sudo apt install build-essential cmake git cargo rustc \
    libclang-dev libsoapysdr-dev libiio-dev libad9361-dev \
    libusb-1.0-0-dev soapysdr-tools sshpass libiio-utils
```

## Repository Structure

```
├── README.md                     # This guide
├── patches/
│   ├── soapy-plutosdr/           # Patches for the SoapyPlutoSDR driver
│   │   └── 01-tick-base-overflow.patch
│   │   └── 02-usb-gadget-network-iio.patch
│   └── tetra-bluestation/        # Patches for tetra-bluestation
│       └── 01-plutosdr-config.patch
│       └── 02-usb-direct-device-arg.patch
│       └── 03-tx-timestamp-every-write.patch
│       └── 04-disable-get-hardware-time.patch
│       └── 05-plutosdr-defaults.patch
├── config/
│   └── config.toml.example       # Example configuration
├── tools/
│   ├── measure_ppm.py            # PPM calibration (two-step)
│   ├── measure_pluto_tx.py       # Measure PlutoSDR TX offset
│   └── measure_pluto_rx.py       # Measure PlutoSDR RX offset (LeoBodnar)
├── firmware/
│   └── README.md                 # Firmware flashing instructions
└── scripts/
    └── restart_dashboard.sh      # Dashboard restart helper
```

## Quick Start

1. **Flash firmware** – see `firmware/README.md`
2. **Patch & build SoapyPlutoSDR** – see `patches/soapy-plutosdr/README.md`
3. **Patch & build tetra-bluestation** – see `patches/tetra-bluestation/README.md`
4. **Adjust configuration** – see `config/config.toml.example`
5. **Run** – `./target/release/tetra-bluestation config.toml`

## Frequency Calculation

```
DL_freq = Band × 100 MHz + Carrier × 25 kHz + Freq_Offset
        = 4 × 100 MHz + 1533 × 25 kHz + 0
        = 438.325 MHz

UL_freq = DL_freq − Duplex_Spacing
        = 438.325 MHz − 7.6 MHz
        = 430.725 MHz
```

## Known Limitations

### Frequency Accuracy

The internal 40 MHz TCXO of the ADALM-Pluto has ±25 ppm accuracy. TETRA
requires ±0.25 ppm. At 438 MHz this can result in up to ~11 kHz of frequency
error.

**Solutions:**
- Adjust `ppm_err` in config.toml after measurement
- Calibrate the Pluto XO correction:
  `ssh root@192.168.2.1 "echo 40000000 > /sys/bus/iio/devices/iio:device0/xo_correction"`
- Use an external GPS-disciplined reference oscillator (recommended)
- Use the PPM measurement tools (see `tools/`)

### Normal Startup Warnings

The following warnings at startup are normal:
- `Lost -1152 samples` – First buffer alignment
- `Too late to produce TX block 0, skipping 5 TX blocks` – Initial sync delay
- `Failed to set RX/TX thread priority` – Not running as root; no functional impact

## Troubleshooting

| Problem | Solution |
|---------|---------|
| "Bad URI" / "no device context found" | `ping 192.168.2.1`, `iio_info -s` |
| "libusb failed to open device (-3)" | Create udev rule (see below) |
| "USB direct mode failed" | `ssh root@192.168.2.1 "ps \| grep sdr"`, `lsusb \| grep Analog` |
| Terminal cannot find the BTS | Check TX frequency, increase TX gain, verify duplex spacing |
| Timestamp overflow / crash | Is the SoapyPlutoSDR patch correctly installed? |

### udev Rule for PlutoSDR

```bash
sudo tee /etc/udev/rules.d/90-plutosdr.rules << 'EOF'
SUBSYSTEM=="usb", ATTR{idVendor}=="0456", ATTR{idProduct}=="b673", MODE="0666"
EOF
sudo udevadm control --reload-rules
sudo udevadm trigger
```

## References

- [tetra-bluestation](https://github.com/MidnightBlueLabs/tetra-bluestation) – TETRA Base Station Stack (Rust)
- [tetra-bluestation Docs Wiki](https://github.com/MidnightBlueLabs/tetra-bluestation-docs/wiki)
- [pgreenland/plutosdr-fw](https://github.com/pgreenland/plutosdr-fw) – Custom PlutoSDR firmware with FPGA timestamps
- [pgreenland/SoapyPlutoSDR](https://github.com/pgreenland/SoapyPlutoSDR) – SoapySDR driver with timestamp support
- [ETSI TS 100 392-15](https://www.etsi.org/deliver/etsi_ts/100300_100399/10039215/) – TETRA frequency bands and duplex spacing

## Licence

MIT – This guide and the associated patches are provided as-is for educational
and amateur radio purposes.
