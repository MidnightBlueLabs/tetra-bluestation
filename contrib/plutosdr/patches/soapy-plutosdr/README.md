# SoapyPlutoSDR Patches

These patches are applied to
[pgreenland's SoapyPlutoSDR](https://github.com/pgreenland/SoapyPlutoSDR)
(branch: `sdr_gadget_timestamping_with_iio_support`).

## Patches

### 1. Tick-Base Overflow Prevention (`01-tick-base-overflow.patch`)

**Problem:** The FPGA sample counter is a free-running uint64 that starts at a
large value. When `SoapySDR::ticksToTimeNs()` multiplies this by 1e9, int64
overflows.

**Solution:** Subtract the first received timestamp as a "tick base" from all
subsequent timestamps. On the TX side the tick base is added back before writing
to the FPGA.

**Affected files:**
- `SoapyPlutoSDR.hpp` – `shared_tick_base` and `shared_tick_base_set` members
- `PlutoSDR_Settings.cpp` – Initialise tick base in constructor
- `PlutoSDR_Streaming.cpp` – Wire tick base into RX/TX and IIO streamers
- `PlutoSDR_RXStreamerUSBGadget.hpp/cpp` – Tick-base subtraction in recv
- `PlutoSDR_TXStreamerUSBGadget.hpp/cpp` – Tick-base addition in send
- `PlutoSDR_RXStreamerIPGadget.hpp/cpp` – Same pattern
- `PlutoSDR_TXStreamerIPGadget.hpp/cpp` – Same pattern

### 2. USB Gadget Device Discovery (`02-usb-gadget-network-iio.patch`)

**Problem:** `open_sdr_usb_gadget()` fails to find the device when the USB bus
and device numbers are not known in advance.

**Solution:** Modified `open_sdr_usb_gadget()` to find the PlutoSDR by USB
Vendor/Product ID (0456:b673) when the URI does not contain explicit USB
bus/device numbers.

**Affected files:**
- `PlutoSDR_Settings.cpp` – `handle_direct_args()` and `open_sdr_usb_gadget()`

## Applying the Patches

A build script is provided that fetches a known-good version of SoapyPlutoSDR
and applies the patches in one step:

```bash
cd contrib/plutosdr
bash build-soapy-plutosdr.sh
```

Alternatively, apply manually:

```bash
git clone https://github.com/pgreenland/SoapyPlutoSDR.git
cd SoapyPlutoSDR
git checkout sdr_gadget_timestamping_with_iio_support

# Apply all patches at once
git apply ../bluestation-plutosdr/patches/soapy-plutosdr/full-plutosdr-patches.patch

# Build
mkdir build && cd build
cmake ..
make -j$(nproc)
sudo make install
```

## Verifying the Installation

```bash
SoapySDRUtil --probe="driver=plutosdr"
```

The installed library should be at:

```bash
ls -la /usr/local/lib/SoapySDR/modules0.8/libPlutoSDRSupport.so
```
